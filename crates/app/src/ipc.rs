//! `#[tauri::command]` の包み。
//!
//! **中身はありません。** `commands` の関数をそのまま呼び、`AppState` を
//! `tauri::State` から取り出すだけです。判断がここに入りはじめたら、それは
//! `commands` に置き場所がなかったということなので、向こうに移してください。
//! `docs/DESIGN.md`「テスト」の開発用ハーネスは `commands` の側を HTTP に出すので、ここに書いたものは
//! ブラウザからは通りません。

use std::path::{Path, PathBuf};

use ekanban_core::db::{FilterState, WindowBoundsState};
use ekanban_core::model::{BoardId, ColumnId};
use tauri::{AppHandle, Emitter as _, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::capture::Registration;
use crate::commands;
use crate::error::AppError;
use crate::events;
use crate::shortcut::KeyPress;
use crate::snapshot::QuickCaptureStatus;
use crate::snapshot::{CaptureTarget, StartupState, ThemePreference};
use crate::state::AppState;

type Reply<T> = Result<T, AppError>;

// ---------------------------------------------------------------- ボード

#[tauri::command]
pub fn startup_state(state: State<'_, AppState>) -> Reply<StartupState> {
    commands::startup_state(&state)
}

#[tauri::command]
pub fn load_documents(state: State<'_, AppState>) -> Reply<Vec<commands::BoardDocument>> {
    commands::load_documents(&state)
}

/// 盤面を書く。**書いたことをほかの窓へ知らせるのはここ**です。
///
/// 書いた本人には戻り値で届くので、送るのはボードの窓が書いていないときだけ。
/// クイックキャプチャの窓が書いたら、ボードの窓は読み直します
/// （`docs/DESIGN.md`「コマンドとイベント」）。
#[tauri::command]
pub fn save_document(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    document: commands::BoardDocument,
    events: Vec<ekanban_core::model::CardEvent>,
) -> Reply<commands::SavedBoard> {
    let saved = commands::save_document(&state, document, events)?;
    if window.label() != crate::run::BOARD_WINDOW {
        if let Err(error) = app.emit_to(crate::run::BOARD_WINDOW, events::BOARD_CHANGED, ()) {
            ekanban_core::diagnostics::log(&format!(
                "failed to tell the board about a change: {error}"
            ));
        }
    }
    Ok(saved)
}

#[tauri::command]
pub fn create_board(state: State<'_, AppState>, name: String) -> Reply<commands::BoardDocument> {
    commands::create_board(&state, &name)
}

#[tauri::command]
pub fn delete_board(state: State<'_, AppState>, board_id: BoardId) -> Reply<()> {
    commands::delete_board(&state, board_id)
}

#[tauri::command]
pub fn set_open_board(state: State<'_, AppState>, board_id: BoardId) -> Reply<()> {
    commands::set_open_board(&state, board_id)
}

// ---------------------------------------------------------------- 絞り込み

// ---------------------------------------------------------------- 表示の状態

#[tauri::command]
pub fn set_filter_state(state: State<'_, AppState>, filter: FilterState) -> Reply<()> {
    commands::set_filter_state(&state, &filter)
}

#[tauri::command]
pub fn set_theme_preference(state: State<'_, AppState>, preference: ThemePreference) -> Reply<()> {
    commands::set_theme_preference(&state, preference)
}

#[tauri::command]
pub fn set_sidebar_collapsed(state: State<'_, AppState>, collapsed: bool) -> Reply<()> {
    commands::set_sidebar_collapsed(&state, collapsed)
}

/// ウィンドウのタイトルを差し替える。
///
/// 文言は webview が組みます（`web/src/state/board.ts`）。ボード名をどう見せるか
/// は表示の判断なので、盤面と同じところに置きました（[ADR 0039]）。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
#[tauri::command]
pub fn set_window_title(window: WebviewWindow, title: String) {
    // 失敗しても盤面は動く。使う人に打てる手も無いので、記録だけ残す。
    if let Err(error) = window.set_title(&title) {
        ekanban_core::diagnostics::log(&format!("failed to set the window title: {error}"));
    }
}

#[tauri::command]
pub fn set_window_bounds(state: State<'_, AppState>, bounds: WindowBoundsState) -> Reply<()> {
    commands::set_window_bounds(&state, bounds)
}

// ---------------------------------------------------------------- ファイル

#[tauri::command]
pub fn export_board_json(
    state: State<'_, AppState>,
    board_id: BoardId,
    destination: PathBuf,
) -> Reply<PathBuf> {
    commands::export_board_json(&state, board_id, &destination)
}

#[tauri::command]
pub fn write_text_file(
    destination: PathBuf,
    extension: String,
    contents: String,
) -> Reply<PathBuf> {
    commands::write_text_file(&destination, &extension, &contents)
}

#[tauri::command]
pub fn backup_database(state: State<'_, AppState>, destination: PathBuf) -> Reply<PathBuf> {
    commands::backup_database(&state, &destination)
}

#[tauri::command]
pub fn database_location(state: State<'_, AppState>) -> PathBuf {
    commands::database_location(&state)
}

/// 保存先を選ばせる。閉じられたら `None`。
///
/// OS のネイティブな保存ダイアログです（`docs/DESIGN.md`「アプリが伝えること」）。**非同期のコマンドにしてあります**
/// ——同期のコマンドは main スレッドで動き、そこでダイアログの返事を待つと
/// ウィンドウごと固まります。
#[tauri::command]
pub async fn choose_save_path(
    app: AppHandle,
    state: State<'_, AppState>,
    file_name: String,
) -> Reply<Option<PathBuf>> {
    // 最初に見せる場所はデータベースの隣。書き出したものを、いちばん近い
    // 「自分のファイルがあるところ」に置けるようにする。
    let directory = commands::database_location(&state)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let (sender, receiver) = tauri::async_runtime::channel(1);
    app.dialog()
        .file()
        .set_directory(directory)
        .set_file_name(file_name)
        .save_file(move |chosen| {
            // 受け取り手が居なくなっていても、こちらから言うことは無い。
            let _ = sender.blocking_send(chosen);
        });

    let mut receiver = receiver;
    Ok(receiver
        .recv()
        .await
        .flatten()
        .and_then(|path| path.into_path().ok()))
}

/// 選んだパスの場所を開く。書き出しの知らせの「場所を開く」がこれ。
#[tauri::command]
pub fn reveal_path(app: AppHandle, path: PathBuf) {
    reveal(&app, &path);
}

/// データベースの場所を、OS のファイル管理で開く。
#[tauri::command]
pub fn reveal_database(app: AppHandle, state: State<'_, AppState>) {
    reveal(&app, &commands::reveal_database(&state));
}

/// 自動バックアップの置き場所を開く。
///
/// まだ 1 つも取れていなければ、開く先がありません。拒否は何も言いません
/// （`docs/DESIGN.md`）。
#[tauri::command]
pub fn reveal_backups(app: AppHandle, state: State<'_, AppState>) {
    if let Some(directory) = commands::reveal_backups(&state) {
        reveal(&app, &directory);
    }
}

fn reveal(app: &AppHandle, path: &Path) {
    if let Err(error) = app.opener().reveal_item_in_dir(path) {
        ekanban_core::diagnostics::log(&format!("failed to reveal {}: {error}", path.display()));
    }
}

// ---------------------------------------------------------------- 期限の下読み

// ---------------------------------------------------------------- 説明のリンク

/// 説明の中のリンクをブラウザで開く（[ADR 0002]）。
///
/// 開いてよい形かどうかは `commands` が決めます。説明はユーザーが打った文字列
/// なので、`file://` や `javascript:` を混ぜられる場所です。
///
/// [ADR 0002]: ../../../docs/adr/0002-links-inside-the-description-field.md
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) {
    let Some(url) = commands::openable_url(&url) else {
        ekanban_core::diagnostics::log(&format!("refused to open {url}"));
        return;
    };
    if let Err(error) = app.opener().open_url(url, None::<&str>) {
        ekanban_core::diagnostics::log(&format!("failed to open {url}: {error}"));
    }
}

// ---------------------------------------------------------------- キャプチャ

/// クイックキャプチャの入れ先。**名前は付きません**——引くのは画面です
/// （[ADR 0039]）。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
#[tauri::command]
pub fn capture_target(state: State<'_, AppState>) -> Reply<Option<CaptureTarget>> {
    commands::capture_target(&state)
}

/// 割り当てのダイアログが開くときに読むもの。使えない環境の理由と、保存されて
/// いるのに登録できていない理由。
#[tauri::command]
pub fn quick_capture_status(registration: State<'_, Registration>) -> QuickCaptureStatus {
    crate::capture::status(&registration)
}

/// 割り当てを差し替える。`None` で解除。保存された形が返る。
#[tauri::command]
pub fn set_quick_capture_shortcut_from_key(
    app: AppHandle,
    state: State<'_, AppState>,
    registration: State<'_, Registration>,
    press: Option<KeyPress>,
) -> Reply<Option<String>> {
    crate::capture::set(&app, &state, &registration, press)
}

/// メニューに付いているキーの割り当てを、付け外しする。
///
/// **割り当てを捕まえている間だけ外します**（`docs/DESIGN.md`「クイックキャプチャ」）。
/// 付いたままだと、メニューのアクセラレータが webview より先に押されたキーを
/// 取ってしまい、`keydown` がダイアログまで届きません。
#[tauri::command]
pub fn set_menu_accelerators_active(app: AppHandle, active: bool) {
    if let Err(error) = crate::menu::set_accelerators_active(&app, active) {
        ekanban_core::diagnostics::log(&format!("failed to change the menu accelerators: {error}"));
    }
}

/// キャプチャの窓を閉じる。
///
/// 盤面には触らないので `commands` に置き場所がありません。窓の操作だけです。
#[tauri::command]
pub fn close_capture_window(app: AppHandle, focus_board: bool) {
    crate::capture::close(&app, focus_board);
}

#[tauri::command]
pub fn set_capture_target(
    state: State<'_, AppState>,
    board_id: Option<BoardId>,
    column_id: Option<ColumnId>,
) -> Reply<()> {
    commands::set_capture_target(&state, board_id.zip(column_id))
}

#[tauri::command]
pub fn set_quick_capture_shortcut(
    state: State<'_, AppState>,
    shortcut: Option<String>,
) -> Reply<()> {
    commands::set_quick_capture_shortcut(&state, shortcut.as_deref())
}

// ---------------------------------------------------------------- 記録

#[tauri::command]
pub fn log_frontend_error(message: String) {
    commands::log_frontend_error(&message);
}
