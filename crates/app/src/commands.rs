//! `docs/DESIGN.md`「コマンドとイベント」のコマンド。
//!
//! **盤面の操作はここにありません**（[ADR 0039]）。このアプリをウェブアプリと
//! して書いたとして、サーバ側に置くだろうものだけが残っています——置き場所の
//! 読み書き、覚えておく設定、OS に頼むこと。カードを足す・動かす・戻すは
//! webview の `web/src/model/board.ts` にあり、結果は `save_document` として
//! 1 本の口から届きます。
//!
//! ここに `tauri` は出てきません。`#[tauri::command]` の包みは `ipc.rs` にあり、
//! こちらは窓を開けずに丸ごと試せます（`crates/app/tests/commands.rs`）。この層が
//! Tauri を知らないことは、都合ではなく設計です。

use std::path::{Path, PathBuf};

use chrono::Local;
use ekanban_core::diagnostics;
use ekanban_core::model::{Board, BoardId, CardEvent, ColumnId, TagId};
use ekanban_core::store::{
    FilterState, Store, StoreError, StoredDocument, WindowBoundsState, DEFAULT_DAY_BOUNDARY_HOUR,
};

use ekanban_core::backup;

use crate::error::{AppError, ErrorKind};
use crate::snapshot::{CaptureTarget, Platform, StartupState, ThemePreference};
use crate::state::{AppState, Source};

// ---------------------------------------------------------------- 起動

/// 開くボードと、その付随状態をデータベースから読む。
///
/// 最後に開いていたボードが消えていれば先頭のボードに黙って戻します。起動を
/// 妨げません。**盤面そのものは読みません**——webview が `load_documents` で
/// 全部読みます（[ADR 0039]）。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
pub fn load_startup_state(source: Source) -> Result<(AppState, StartupState), AppError> {
    let state = AppState::open(source);
    let startup = startup_state(&state)?;
    Ok((state, startup))
}

/// 最初に開くボードと、付随する表示の状態。
///
/// webview が起動のときに 1 回呼びます。**ウィンドウを開き直すときも同じ経路を
/// 通ります**。閉じている間もクイックキャプチャはカードを足せるので、メモリ上の
/// 値を抱えて使い回さず、そのつどデータベースから読みます（`docs/DESIGN.md`）。
pub fn startup_state(state: &AppState) -> Result<StartupState, AppError> {
    let mut store = state.store().map_err(open_failed)?;
    // 何も入っていなければ最初の盤面を蒔く。SQLite は開いた時点で蒔いてある。
    store.seed_if_empty().map_err(open_failed)?;
    let boards = store.load_boards().map_err(open_failed)?;
    let open_board_id = store
        .load_last_board_id()
        .map_err(open_failed)?
        .filter(|board_id| boards.iter().any(|board| board.id == *board_id))
        .or_else(|| boards.first().map(|board| board.id))
        .ok_or_else(|| open_failed(StoreError::NoBoard))?;
    store
        .set_last_board_id(open_board_id)
        .map_err(open_failed)?;

    Ok(StartupState {
        open_board_id,
        platform: Platform::current(),
        filter: store.load_filter_state().unwrap_or_default(),
        window_bounds: store.load_window_bounds().ok().flatten(),
        theme: ThemePreference::parse(store.load_theme_preference().ok().flatten().as_deref()),
        sidebar_collapsed: store.load_sidebar_collapsed().unwrap_or(false),
        day_boundary_hour: store
            .load_day_boundary_hour()
            .unwrap_or(DEFAULT_DAY_BOUNDARY_HOUR),
        capture_target: capture_target_of(&mut store),
        quick_capture_shortcut: store.load_quick_capture_shortcut().unwrap_or(None),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database_path: state.source().place(),
    })
}

fn open_failed(error: StoreError) -> AppError {
    AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
}

// ---------------------------------------------------------------- ボード
//
// **盤面の中身は触りません。** ボードを作る・消す・どれを開いていたか覚える
// ——採番の名前空間を切るのと、行を消すのと、1 行の設定を書くのだけが残ります
// （[ADR 0039]）。名前を変えるのは盤面の操作なので `save_document` を通ります。

/// ボードを 1 つ作る。**採番するのは置き場所**なので、webview からは頼むだけ。
///
/// 返すのは作ったばかりの文書です。読み直しに行かせず、そのまま手元の一覧へ
/// 足せます。
pub fn create_board(state: &AppState, name: &str) -> Result<BoardDocument, AppError> {
    let fail = |error: &StoreError| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", error)
    };
    let mut store = state.store().map_err(|e| fail(&e))?;
    let board = store.create_board(name).map_err(|e| fail(&e))?;
    let rev = store.load_board_rev(board.id).map_err(|e| fail(&e))?;
    store.set_last_board_id(board.id).map_err(|e| fail(&e))?;
    Ok(BoardDocument::of(StoredDocument { board, rev }))
}

/// ボードを消す。**最後の 1 つは消せません**（`Database::delete_board` が拒否します）。
///
/// 次にどれを開くかは webview が決めます——残っている盤面はそちらが持って
/// いるので、選ぶのに読み直しが要りません。
pub fn delete_board(state: &AppState, board_id: BoardId) -> Result<(), AppError> {
    let mut store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを削除できませんでした", &error)
    })?;
    store.delete_board(board_id).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを削除できませんでした", &error)
    })
}

/// 開いているボードを覚える。次の起動はここから始まります。
pub fn set_open_board(state: &AppState, board_id: BoardId) -> Result<(), AppError> {
    store(
        state,
        "開いているボードを覚えられませんでした",
        |store| store.set_last_board_id(board_id),
    )
}

// ---------------------------------------------------------------- 盤面の読み書き

/// 置き場所から読んだ盤面 1 つぶん。**webview が持つ形**（[ADR 0039]）。
///
/// `Board` に、境界を越えない値（採番の続き）と版を添えたものです。採番の
/// 続きは画面が手元で番号を採るのに要り、版は保存の競合を見るのに要ります
/// （[ADR 0040]）。どちらも盤面の中身ではないので、`Board` の中には入れません。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
/// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BoardDocument {
    pub board: Board,
    pub next_card_id: i64,
    pub next_column_id: ColumnId,
    pub next_tag_id: TagId,
    pub next_checklist_item_id: i64,
    /// 保存のたびに 1 つ進む。手元のものと合わなければ、置き場所が断ります。
    pub rev: i64,
}

impl BoardDocument {
    fn of(stored: StoredDocument) -> Self {
        Self {
            next_card_id: stored.board.next_card_id,
            next_column_id: stored.board.next_column_id,
            next_tag_id: stored.board.next_tag_id,
            next_checklist_item_id: stored.board.next_checklist_item_id,
            rev: stored.rev,
            board: stored.board,
        }
    }

    /// 置き場所へ渡す形に戻す。**採番の続きを盤面へ書き戻します**——`Board` の
    /// 側では `#[serde(skip)]` なので、受け取った JSON には入っていません。
    fn into_board(self, events: Vec<CardEvent>) -> Board {
        let mut board = self.board;
        board.next_card_id = self.next_card_id;
        board.next_column_id = self.next_column_id;
        board.next_tag_id = self.next_tag_id;
        board.next_checklist_item_id = self.next_checklist_item_id;
        board.adopt_pending_events(events);
        board
    }
}

/// 保存できたときに返すもの。**次に書くときの版**です。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SavedBoard {
    pub rev: i64,
}

/// 全部のボードを、webview が持つ形で読む（[ADR 0039]）。
///
/// **一覧のためだけに開くのではありません。** 盤面を持つのが webview になると、
/// ボードの切り替えも期限の件数も手元で済みます。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
pub fn load_documents(state: &AppState) -> Result<Vec<BoardDocument>, AppError> {
    let store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
    })?;
    let documents = store.load_documents().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
    })?;
    Ok(documents.into_iter().map(BoardDocument::of).collect())
}

/// 盤面を書く。**検めてから書きます**（[ADR 0040]）。
///
/// 見るのは 2 つ。**版**が手元のものと合っているか（合わなければ、ほかの窓が
/// 書いたということなので断ります）と、渡された盤面が**行として成り立って
/// いるか**（`Board::validate`）です。盤面の判断はやり直しません。
///
/// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
pub fn save_document(
    state: &AppState,
    document: BoardDocument,
    events: Vec<CardEvent>,
) -> Result<SavedBoard, AppError> {
    let expected = document.rev;
    let mut board = document.into_board(events);
    board
        .validate()
        .map_err(|error| AppError::from_board("保存できませんでした", &error))?;

    let mut store = state.store().map_err(|error| AppError::from_save(&error))?;
    let rev = store
        .save_board_at(&mut board, expected)
        .map_err(|error| AppError::from_save(&error))?;
    Ok(SavedBoard { rev })
}

// ---------------------------------------------------------------- 表示の状態

fn store(
    state: &AppState,
    title: &'static str,
    write: impl FnOnce(&mut Store) -> Result<(), StoreError>,
) -> Result<(), AppError> {
    let mut store = state
        .store()
        .map_err(|error| AppError::from_db(ErrorKind::Save, title, &error))?;
    write(&mut store).map_err(|error| AppError::from_db(ErrorKind::Save, title, &error))
}

pub fn set_filter_state(state: &AppState, filter: &FilterState) -> Result<(), AppError> {
    store(
        state,
        "絞り込みを覚えられませんでした",
        |store| store.set_filter_state(filter),
    )
}

pub fn set_theme_preference(state: &AppState, preference: ThemePreference) -> Result<(), AppError> {
    store(
        state,
        "テーマを覚えられませんでした",
        |store| store.set_theme_preference(preference.as_str()),
    )
}

/// 日付が変わる時刻を覚える（[ADR 0048]）。
///
/// 0〜23 の外は置き場所が断ります。**基準日を作るのは画面**で、ここは覚えて
/// おくだけです。
///
/// [ADR 0048]: ../../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md
pub fn set_day_boundary_hour(state: &AppState, hour: u8) -> Result<(), AppError> {
    store(
        state,
        "日付の切り替わりを覚えられませんでした",
        |store| store.set_day_boundary_hour(hour),
    )
}

pub fn set_sidebar_collapsed(state: &AppState, collapsed: bool) -> Result<(), AppError> {
    store(
        state,
        "サイドバーの状態を覚えられませんでした",
        |store| store.set_sidebar_collapsed(collapsed),
    )
}

pub fn set_window_bounds(state: &AppState, bounds: WindowBoundsState) -> Result<(), AppError> {
    store(
        state,
        "ウィンドウの位置を覚えられませんでした",
        |store| store.set_window_bounds(bounds),
    )
}

// ---------------------------------------------------------------- ファイル

/// 選ばれたパスに拡張子を補う。
///
/// 保存ダイアログで名前を打ち替えると、拡張子ごと消えることがあります。
/// 拡張子の無いファイルを書くと、次に開くときに何のファイルか分かりません。
/// **すでに何か付いているものは触りません**——`board.json.txt` を選んだ人の
/// 意図を、こちらで書き換えないためです。
fn with_extension(destination: &Path, extension: &str) -> PathBuf {
    if destination.extension().is_none() {
        destination.with_extension(extension)
    } else {
        destination.to_path_buf()
    }
}

/// 開いているボードを JSON にする。**まだ書きません。**
///
/// 書く先が無い環境があるので分けてあります（ブラウザ、[ADR 0042]）。そこでは
/// この文字列がそのままページへ渡り、ダウンロードになります。
///
/// **組み立てるのがここなのは、置いてある形の写しだから**です（[ADR 0045]）。
/// カードの履歴（`card_events`）まで入り、それは置き場所にしかありません。
/// 人が読む Markdown のほうは webview が組み立てます。
///
/// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
/// [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
pub fn export_board_json_contents(state: &AppState, board_id: BoardId) -> Result<String, AppError> {
    let fail =
        |error: &StoreError| AppError::from_db(ErrorKind::Export, "書き出せませんでした", error);
    let store = state.store().map_err(|e| fail(&e))?;
    let board = store.load_board_by_id(board_id).map_err(|e| fail(&e))?;
    store.export_board_json(&board).map_err(|e| fail(&e))
}

/// 開いているボードを JSON のファイルに書き出す。書けたパスを返す。
///
/// **書く先があるのは配るアプリだけ**です。ブラウザには書き込めるファイル
/// システムが無いので、そちらは置き場所ごと TypeScript にあります
/// （`web/src/store/export.ts`、[ADR 0042]）。
pub fn export_board_json(
    state: &AppState,
    board_id: BoardId,
    destination: &Path,
) -> Result<PathBuf, AppError> {
    write_text_file(
        destination,
        "json",
        &export_board_json_contents(state, board_id)?,
    )
}

/// 組み立てられた中身を、選ばれた場所に書く。書けたパスを返す。
///
/// **中身は受け取るだけ**です（[ADR 0045]）。行き先を選ぶのは OS の保存
/// ダイアログ、報せるのはアプリの中のダイアログで（`docs/DESIGN.md`「アプリが
/// 伝えること」）、ここはその間の「書く」だけを引き受けます。
///
/// [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
pub fn write_text_file(
    destination: &Path,
    extension: &str,
    contents: &str,
) -> Result<PathBuf, AppError> {
    let destination = &with_extension(destination, extension);
    std::fs::write(destination, contents).map_err(|error| {
        AppError::new(
            ErrorKind::Export,
            "書き出せませんでした",
            format!("{} に書けませんでした: {error}", destination.display()),
        )
    })?;
    Ok(destination.to_path_buf())
}

/// データベースの控えを、選んだ場所に取る。書けたパスを返す。
///
/// **いま使っているファイルそのものは断ります。** `backup_to` は上書きで開くので、
/// 同じパスを渡すと控えを取ったつもりで元のファイルを触ることになります。
///
/// 控えは SQLite のファイルを写すものです。**ブラウザ版に SQLite のファイルは
/// ありません**ので、そちらでは「データベースをコピー…」を灰色にしてあります
/// （[ADR 0042]）。
pub fn backup_database(state: &AppState, destination: &Path) -> Result<PathBuf, AppError> {
    let destination = &with_extension(destination, "sqlite3");
    if destination == state.database_path() {
        return Err(AppError::new(
            ErrorKind::Export,
            "控えを保存できませんでした",
            "控えの保存先には、いま使っているデータベースとは別のファイルを指定してください",
        ));
    }
    let database = ekanban_core::db::Database::open(state.database_path()).map_err(|error| {
        AppError::from_db(ErrorKind::Export, "控えを保存できませんでした", &error)
    })?;
    database.backup_to(destination).map_err(|error| {
        AppError::from_db(ErrorKind::Export, "控えを保存できませんでした", &error)
    })?;
    Ok(destination.to_path_buf())
}

/// データベースそのものの場所。
pub fn database_location(state: &AppState) -> PathBuf {
    state.database_path().to_path_buf()
}

/// 「場所を開く」で開く先。実際に開くのは呼ぶ側（`tauri-plugin-opener`、`docs/DESIGN.md`「アプリが伝えること」）。
pub fn reveal_database(state: &AppState) -> PathBuf {
    state.database_path().to_path_buf()
}

/// 日ごとの控えが溜まるディレクトリ。
///
/// まだ 1 つも取れていないうちに押されることがあります。開く先が無いだけなので
/// `None` を返し、呼ぶ側は黙って何もしません（拒否は何も言わない、`docs/DESIGN.md`）。
pub fn reveal_backups(state: &AppState) -> Option<PathBuf> {
    let directory = backup::directory(state.database_path());
    directory.is_dir().then_some(directory)
}

// ---------------------------------------------------------------- 期限の下読み

// ---------------------------------------------------------------- 説明のリンク

/// 開いてよい URL か。開けるなら、そのまま返す。
///
/// 拾うのは `http(s)://` だけという [ADR 0002] の決めごとを、**開く側でも
/// 確かめます**。説明はユーザーが打った文字列なので、`file://` や
/// `javascript:` を混ぜられる場所です。
///
/// [ADR 0002]: ../../../docs/adr/0002-links-inside-the-description-field.md
pub fn openable_url(url: &str) -> Option<&str> {
    (url.starts_with("https://") || url.starts_with("http://")).then_some(url)
}

// ---------------------------------------------------------------- キャプチャ

/// 覚えてあるキャプチャ先。**そのまま返します。**
///
/// 指している先が消えていても、ここでは何もしません。既定（先頭のボードの
/// 先頭カラム、[ADR 0028]）に落とすのも、名前を引くのも webview の仕事です
/// ——盤面を全部持っているのはそちらなので（[ADR 0039]）、そこで済みます。
///
/// [ADR 0028]: ../../../docs/adr/0028-a-single-default-quick-capture-target.md
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
fn capture_target_of(store: &mut Store) -> Option<CaptureTarget> {
    store
        .load_capture_target()
        .unwrap_or(None)
        .map(|(board_id, column_id)| CaptureTarget {
            board_id,
            column_id,
        })
}

/// いまのキャプチャ先。設定が無ければ `None`。
pub fn capture_target(state: &AppState) -> Result<Option<CaptureTarget>, AppError> {
    let mut store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "キャプチャ先を読めませんでした", &error)
    })?;
    Ok(capture_target_of(&mut store))
}

/// キャプチャ先を覚える。`None` で既定（先頭のボードの先頭カラム）に戻す。
pub fn set_capture_target(
    state: &AppState,
    target: Option<(BoardId, ColumnId)>,
) -> Result<(), AppError> {
    store(
        state,
        "カードの追加先を覚えられませんでした",
        |store| store.set_capture_target(target),
    )
}

/// 保存されている割り当てを読む。無ければ `None`。
pub fn quick_capture_shortcut(state: &AppState) -> Result<Option<String>, AppError> {
    let store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::Save, "割り当てを読めませんでした", &error)
    })?;
    store
        .load_quick_capture_shortcut()
        .map_err(|error| AppError::from_db(ErrorKind::Save, "割り当てを読めませんでした", &error))
}

/// 割り当てを覚える。**登録できなかった割り当ては保存しません**（`docs/DESIGN.md`「クイックキャプチャ」）ので、
/// 呼ぶ側が登録に成功してからここを呼びます。読めない文字列はここで断ります。
pub fn set_quick_capture_shortcut(
    state: &AppState,
    shortcut: Option<&str>,
) -> Result<(), AppError> {
    store(
        state,
        "割り当てを覚えられませんでした",
        |store| store.set_quick_capture_shortcut(shortcut),
    )
}

// ---------------------------------------------------------------- 記録

/// webview の未捕捉例外を、Rust 側と同じログに落とす（`docs/DESIGN.md`「アプリが伝えること」）。
///
/// webview の失敗が黙って消えると、原因を追う手段がなくなります。
pub fn log_frontend_error(message: &str) {
    diagnostics::log(&format!("webview: {message}"));
}

/// その日ぶんの控えを 1 つ残す。起動のときに別スレッドから呼びます。
///
/// 失敗しても起動は止めません（`docs/DESIGN.md`）。取るのは起動時で、終了時では
/// ない——終了時に取ると、壊した状態のほうを保存することになります。
pub fn run_daily_backup(database_path: &Path) {
    if let Err(error) = backup::run_daily(database_path, Local::now().date_naive()) {
        diagnostics::log(&format!(
            "failed to back up {}: {error}",
            database_path.display()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拡張子を落として保存されたファイルは、次に開くときに何か分からない。
    #[test]
    fn adds_the_extension_when_the_chosen_name_has_none() {
        assert_eq!(
            with_extension(Path::new("/tmp/board"), "json"),
            PathBuf::from("/tmp/board.json")
        );
    }

    /// すでに付いているものは触らない。`board.json.txt` を選んだ意図を書き換えない。
    #[test]
    fn keeps_the_extension_the_person_chose() {
        assert_eq!(
            with_extension(Path::new("/tmp/board.txt"), "json"),
            PathBuf::from("/tmp/board.txt")
        );
    }

    /// 説明はユーザーが打った文字列なので、開く前に確かめる（ADR 0002）。
    #[test]
    fn opens_only_http_and_https() {
        assert_eq!(
            openable_url("https://example.com"),
            Some("https://example.com")
        );
        assert_eq!(
            openable_url("http://example.com"),
            Some("http://example.com")
        );
        assert_eq!(openable_url("file:///etc/passwd"), None);
        assert_eq!(openable_url("javascript:alert(1)"), None);
        assert_eq!(openable_url("example.com"), None);
    }
}
