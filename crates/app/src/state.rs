//! 開いている盤面を持つところ（`docs/DESIGN.md`「状態の持ち主」、[ADR 0018]）。
//!
//! [ADR 0018]: ../../../docs/adr/0018-rust-owns-the-board-state.md

#[cfg(feature = "shell")]
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError, TryLockError};

use chrono::Local;
use ekanban_core::model::{Board, BoardError, BoardSummary, ColumnId};
use ekanban_core::store::{JsonStore, Store, StoreError};

use crate::error::{AppError, ErrorKind};
use crate::snapshot::{due_statuses_of, window_title, Snapshot};

/// 開いているボードと、その裏のデータベース。
///
/// `Board` は Rust が持ち、webview はその投影だけを描きます。Undo / Redo の
/// スタックもこの中です。
///
/// `docs/DESIGN.md`「状態の持ち主」に沿った形です。保存を直列化する `save: Mutex<()>` を置いて
/// いません。**コマンドは盤面のロックを持ったまま適用と保存を続けて行う**ので、
/// 盤面のロックが保存の順番もそのまま決めます。2 つ目のロックは、同じことを
/// 2 か所で守る形になります。
pub struct AppState {
    source: Source,
    board: Mutex<Board>,
}

/// 盤面をどこに置くか（[ADR 0036]）。
///
/// **開くたびに開き直します。** クイックキャプチャの窓が閉じている間にも書ける
/// ので、メモリ上の写しを抱え続けると、そちらの変更が見えません。JSON の側は
/// ページに 1 つしかないので、同じものを借り直します。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
#[derive(Clone)]
pub enum Source {
    #[cfg(feature = "shell")]
    Sqlite(std::path::PathBuf),
    Json {
        store: Arc<Mutex<JsonStore>>,
        /// 「どこにあるか」を人が読む形で。**パスではありません**——ブラウザに
        /// ファイルシステム上の居場所は無いので、置いた側が名乗ります。
        place: String,
    },
}

impl Source {
    /// 置き場所を開く。
    pub fn open(&self) -> Result<Store<'_>, StoreError> {
        match self {
            #[cfg(feature = "shell")]
            Self::Sqlite(path) => Ok(Store::Sqlite(ekanban_core::db::Database::open(path)?)),
            // **待ちません。** 取れないのは、同じ置き場所を開いたまま
            // もう 1 つ開いたときだけです（`AppState::store` の注意書き）。
            // 待つと、相手が自分なので二度と空きません。
            Self::Json { store, .. } => match store.try_lock() {
                Ok(guard) => Ok(Store::Json(guard)),
                Err(TryLockError::Poisoned(poisoned)) => Ok(Store::Json(poisoned.into_inner())),
                Err(TryLockError::WouldBlock) => Err(StoreError::AlreadyOpen),
            },
        }
    }

    /// 盤面がどこにあるか。画面（「ekanban について」）に出す文言。
    pub fn place(&self) -> String {
        match self {
            #[cfg(feature = "shell")]
            Self::Sqlite(path) => path.to_string_lossy().into_owned(),
            Self::Json { place, .. } => place.clone(),
        }
    }

    /// SQLite のファイルの場所。JSON の置き場所には無い。
    #[cfg(feature = "shell")]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Sqlite(path) => Some(path),
            Self::Json { .. } => None,
        }
    }
}

impl AppState {
    /// 置き場所を決め、最後に開いていたボードを載せる。
    pub fn open(source: Source, board: Board) -> Self {
        Self {
            source,
            board: Mutex::new(board),
        }
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    /// SQLite のファイルの場所。**殻を持っているときだけ**あります。
    #[cfg(feature = "shell")]
    pub fn database_path(&self) -> &Path {
        self.source
            .path()
            .expect("殻の側は必ず SQLite のファイルを持つ")
    }

    /// 盤面のロックを取る。
    ///
    /// ロックが毒されているのは、前のコマンドがパニックしたときだけです。そこで
    /// 全部のコマンドを落とすと、記録を読むことすらできなくなるので、中身を
    /// 取り出して続けます。パニックそのものは `diagnostics` のフックが記録します。
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Board> {
        self.board.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 置き場所を開く。
    ///
    /// **開いたまま、もう 1 つ開かないでください。** SQLite は接続を 2 つ持てる
    /// ので気づけませんが、ブラウザ版の置き場所はページに 1 つで、同じ
    /// `Mutex` を 2 度取ると落ちます（[ADR 0036]）。持っている置き場所から
    /// 組み立てるか、先に `drop` してください。
    ///
    /// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
    pub(crate) fn store(&self) -> Result<Store<'_>, StoreError> {
        self.source.open()
    }

    /// 盤面を変えて保存し、変更後のスナップショットを返す。
    ///
    /// **両方成功してから返します。** 途中で失敗したら盤面への変更も捨てて `Err`
    /// を返すので、画面には何も届かず、巻き戻すものもありません（`docs/DESIGN.md`「状態の持ち主」）。
    ///
    /// `apply` が `false` を返したら「変更なし」です。保存はせず、スナップショット
    /// だけ返します。何も言わないのは今までどおりです（`docs/DESIGN.md`「コマンドとイベント」）。
    pub(crate) fn mutate<T>(
        &self,
        title: &'static str,
        apply: impl FnOnce(&mut Board) -> Result<T, BoardError>,
    ) -> Result<(T, Snapshot), AppError>
    where
        T: Changed,
    {
        let mut board = self.lock();
        // 失敗したときに戻す先。数百枚のカードを 1 回複製するだけなので、
        // 操作ごとに差分を組み立てるより素直で速い。
        let before = board.clone();

        let value = match apply(&mut board) {
            Ok(value) => value,
            Err(error) => {
                *board = before;
                return Err(AppError::from_board(title, &error));
            }
        };

        let mut store = self.store().map_err(|error| {
            *board = before.clone();
            AppError::from_save(&error)
        })?;

        if value.changed() {
            if let Err(error) = store.save_board(&mut board) {
                *board = before;
                return Err(AppError::from_save(&error));
            }
        }

        let snapshot = snapshot_of(&board, &store).map_err(|error| {
            AppError::from_db(ErrorKind::BoardIo, "ボード一覧を読めませんでした", &error)
        })?;
        Ok((value, snapshot))
    }

    /// いま開いている盤面のスナップショット。何も変えない。
    pub fn snapshot(&self) -> Result<Snapshot, AppError> {
        let board = self.lock();
        let store = self.store().map_err(|error| {
            AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
        })?;
        snapshot_of(&board, &store).map_err(|error| {
            AppError::from_db(ErrorKind::BoardIo, "ボード一覧を読めませんでした", &error)
        })
    }

    /// 開いているボードを丸ごと差し替える。ボードの切り替えと作成・削除で通る。
    pub(crate) fn replace(&self, next: Board) {
        *self.lock() = next;
    }
}

/// クイックキャプチャの入れ先が、このボードのどのカラムか。
///
/// 設定が指しているカラムがこのボードにあればそれ、別のボードなら `None`。
/// 設定が無ければ既定で、それは**先頭のボードの先頭カラム**です（#117、[ADR 0028]）
/// ——開いているボードから決めていたころは、設定していない状態でどのボードを
/// 開いても「⚡ クイックキャプチャ先」が出ていました。入れ先はアプリ全体で
/// 1 つなので、印も 1 か所にしか出ません。
///
/// 先頭のボードは `boards`（`load_boards_as_of` の順、`ORDER BY boards.id`）の
/// 1 つめで、サイドバーの一番上と同じです。**スナップショットが既に読んで
/// いる一覧をそのまま受けます**——ここでもう 1 回引くと、盤面を変えるたびに
/// 同じクエリが 2 回走ります。落とし方は `commands::capture_target` と揃えてあります。
///
/// [ADR 0028]: ../../../docs/adr/0028-a-single-default-quick-capture-target.md
fn capture_column_of(
    board: &Board,
    store: &Store<'_>,
    boards: &[BoardSummary],
) -> Option<ColumnId> {
    let first_column = || board.columns.first().map(|column| column.id);
    match store.load_capture_target().unwrap_or(None) {
        Some((board_id, _)) if board_id != board.id => None,
        Some((_, column_id)) if board.columns.iter().any(|column| column.id == column_id) => {
            Some(column_id)
        }
        // このボードを指しているのにカラムが無い。選ばれているのはこのボード
        // なので、その先頭カラムに落とす。
        Some(_) => first_column(),
        // 設定が無い。既定の先頭ボードでなければ、ここには印を出さない。
        None => match boards.first() {
            Some(first) if first.id == board.id => first_column(),
            _ => None,
        },
    }
}

pub(crate) fn snapshot_of(board: &Board, store: &Store<'_>) -> Result<Snapshot, StoreError> {
    let today = Local::now().date_naive();
    let boards = store.load_boards_as_of(today)?;
    Ok(Snapshot {
        board: board.clone(),
        can_undo: board.can_undo(),
        can_redo: board.can_redo(),
        due_statuses: due_statuses_of(board, today),
        today,
        capture_column: capture_column_of(board, store, &boards),
        window_title: window_title(&board.name),
        boards,
    })
}

/// 「この操作は盤面を変えたか」を、モデルの戻り値から読み取るための橋。
///
/// `model.rs` の関数は、変えたかどうかを `bool` で返すもの（`rename_column`）、
/// 作った ID を返すもの（`add_card`）、`()` を返すもの（`delete_card`）が
/// 混ざっています。変えていないなら保存しない、という判断をコマンドごとに
/// 書き写さずに済ませます。
pub(crate) trait Changed {
    fn changed(&self) -> bool;
}

impl Changed for bool {
    fn changed(&self) -> bool {
        *self
    }
}

impl Changed for () {
    fn changed(&self) -> bool {
        true
    }
}

impl Changed for i64 {
    fn changed(&self) -> bool {
        true
    }
}

impl Changed for usize {
    fn changed(&self) -> bool {
        // `archive_column` は「何枚アーカイブしたか」を返す。0 枚なら盤面は
        // 変わっていない。
        *self > 0
    }
}
