//! 開いている盤面を持つところ（`docs/TAURI-MIGRATION.md` §2、[ADR 0018]）。
//!
//! [ADR 0018]: ../../../docs/adr/0018-rust-owns-the-board-state.md

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use chrono::Local;
use ekanban_core::db::{Database, DbError};
use ekanban_core::model::{Board, BoardError};

use crate::error::{AppError, ErrorKind};
use crate::snapshot::Snapshot;

/// 開いているボードと、その裏のデータベース。
///
/// `Board` は Rust が持ち、webview はその投影だけを描きます。Undo / Redo の
/// スタックもこの中です。
///
/// §2 の素描には保存を直列化する `save: Mutex<()>` も並んでいますが、置いて
/// いません。**コマンドは盤面のロックを持ったまま適用と保存を続けて行う**ので、
/// 盤面のロックが保存の順番もそのまま決めます。2 つ目のロックは、同じことを
/// 2 か所で守る形になります。
pub struct AppState {
    database_path: PathBuf,
    board: Mutex<Board>,
}

impl AppState {
    /// データベースを開き、最後に開いていたボードを載せる。
    pub fn open(database_path: impl Into<PathBuf>, board: Board) -> Self {
        Self {
            database_path: database_path.into(),
            board: Mutex::new(board),
        }
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// 盤面のロックを取る。
    ///
    /// ロックが毒されているのは、前のコマンドがパニックしたときだけです。そこで
    /// 全部のコマンドを落とすと、記録を読むことすらできなくなるので、中身を
    /// 取り出して続けます。パニックそのものは `diagnostics` のフックが記録します。
    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Board> {
        self.board.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn database(&self) -> Result<Database, DbError> {
        Database::open(&self.database_path)
    }

    /// 盤面を変えて保存し、変更後のスナップショットを返す。
    ///
    /// **両方成功してから返します。** 途中で失敗したら盤面への変更も捨てて `Err`
    /// を返すので、画面には何も届かず、巻き戻すものもありません（§2）。
    ///
    /// `apply` が `false` を返したら「変更なし」です。保存はせず、スナップショット
    /// だけ返します。何も言わないのは今までどおりです（§3）。
    pub(crate) fn mutate<T>(
        &self,
        title: &'static str,
        apply: impl FnOnce(&mut Board) -> Result<T, BoardError>,
    ) -> Result<(T, Snapshot), AppError>
    where
        T: Changed,
    {
        let mut board = self.lock();
        // 失敗したときに戻す先。gpui 版と同じで、数百枚のカードを 1 回複製する。
        let before = board.clone();

        let value = match apply(&mut board) {
            Ok(value) => value,
            Err(error) => {
                *board = before;
                return Err(AppError::from_board(title, &error));
            }
        };

        let mut database = self.database().map_err(|error| {
            *board = before.clone();
            AppError::from_save(&error)
        })?;

        if value.changed() {
            if let Err(error) = database.save_board(&mut board) {
                *board = before;
                return Err(AppError::from_save(&error));
            }
        }

        let snapshot = snapshot_of(&board, &database).map_err(|error| {
            AppError::from_db(ErrorKind::BoardIo, "ボード一覧を読めませんでした", &error)
        })?;
        Ok((value, snapshot))
    }

    /// いま開いている盤面のスナップショット。何も変えない。
    pub fn snapshot(&self) -> Result<Snapshot, AppError> {
        let board = self.lock();
        let database = self.database().map_err(|error| {
            AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
        })?;
        snapshot_of(&board, &database).map_err(|error| {
            AppError::from_db(ErrorKind::BoardIo, "ボード一覧を読めませんでした", &error)
        })
    }

    /// 開いているボードを丸ごと差し替える。ボードの切り替えと作成・削除で通る。
    pub(crate) fn replace(&self, next: Board) {
        *self.lock() = next;
    }
}

pub(crate) fn snapshot_of(board: &Board, database: &Database) -> Result<Snapshot, DbError> {
    Ok(Snapshot {
        board: board.clone(),
        boards: database.load_boards_as_of(Local::now().date_naive())?,
        can_undo: board.can_undo(),
        can_redo: board.can_redo(),
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
