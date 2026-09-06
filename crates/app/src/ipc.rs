//! `#[tauri::command]` の包み。
//!
//! **中身はありません。** `commands` の関数をそのまま呼び、`AppState` を
//! `tauri::State` から取り出すだけです。判断がここに入りはじめたら、それは
//! `commands` に置き場所がなかったということなので、向こうに移してください。
//! `docs/DESIGN.md`「テスト」の開発用ハーネスは `commands` の側を HTTP に出すので、ここに書いたものは
//! ブラウザからは通りません。

use std::path::{Path, PathBuf};

use ekanban_core::db::{FilterState, WindowBoundsState};
use ekanban_core::model::{BoardId, CardId, ChecklistItemDraft, ColumnId, TagId};
use tauri::{AppHandle, Emitter as _, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::commands::{self, ExportFormat};
use crate::error::AppError;
use crate::events;
use crate::shortcut::KeyPress;
use crate::snapshot::{CaptureTarget, Snapshot, StartupState, ThemePreference};
use crate::state::AppState;

type Reply<T> = Result<T, AppError>;

// ---------------------------------------------------------------- ボード

#[tauri::command]
pub fn startup_state(state: State<'_, AppState>) -> Reply<StartupState> {
    commands::startup_state(&state)
}

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> Reply<Snapshot> {
    state.snapshot()
}

#[tauri::command]
pub fn create_board(state: State<'_, AppState>, name: String) -> Reply<Snapshot> {
    commands::create_board(&state, &name)
}

#[tauri::command]
pub fn rename_board(state: State<'_, AppState>, name: String) -> Reply<Snapshot> {
    commands::rename_board(&state, &name)
}

#[tauri::command]
pub fn delete_board(state: State<'_, AppState>, board_id: BoardId) -> Reply<Snapshot> {
    commands::delete_board(&state, board_id)
}

#[tauri::command]
pub fn switch_board(state: State<'_, AppState>, board_id: BoardId) -> Reply<Snapshot> {
    commands::switch_board(&state, board_id)
}

// ---------------------------------------------------------------- カード

#[tauri::command]
pub fn add_card(
    state: State<'_, AppState>,
    column_id: ColumnId,
    title: String,
    description: String,
) -> Reply<Snapshot> {
    commands::add_card(&state, column_id, &title, &description)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_card(
    state: State<'_, AppState>,
    card_id: CardId,
    title: String,
    description: String,
    due_date: String,
    tag_ids: Vec<TagId>,
    checklist: Vec<ChecklistItemDraft>,
) -> Reply<Snapshot> {
    commands::update_card(
        &state,
        card_id,
        &title,
        &description,
        &due_date,
        tag_ids,
        checklist,
    )
}

#[tauri::command]
pub fn move_card(
    state: State<'_, AppState>,
    card_id: CardId,
    to_column_id: ColumnId,
    to_index: usize,
) -> Reply<Snapshot> {
    commands::move_card(&state, card_id, to_column_id, to_index)
}

#[tauri::command]
pub fn copy_card(state: State<'_, AppState>, card_id: CardId) -> Reply<Snapshot> {
    commands::copy_card(&state, card_id)
}

#[tauri::command]
pub fn delete_card(state: State<'_, AppState>, card_id: CardId) -> Reply<Snapshot> {
    commands::delete_card(&state, card_id)
}

#[tauri::command]
pub fn archive_card(state: State<'_, AppState>, card_id: CardId) -> Reply<Snapshot> {
    commands::archive_card(&state, card_id)
}

#[tauri::command]
pub fn restore_card(state: State<'_, AppState>, card_id: CardId) -> Reply<Snapshot> {
    commands::restore_card(&state, card_id)
}

#[tauri::command]
pub fn set_card_due_date(
    state: State<'_, AppState>,
    card_id: CardId,
    due_date: String,
) -> Reply<Snapshot> {
    commands::set_card_due_date(&state, card_id, &due_date)
}

#[tauri::command]
pub fn set_card_tags(
    state: State<'_, AppState>,
    card_id: CardId,
    tag_ids: Vec<TagId>,
) -> Reply<Snapshot> {
    commands::set_card_tags(&state, card_id, tag_ids)
}

// ---------------------------------------------------------------- カラム

#[tauri::command]
pub fn add_column(state: State<'_, AppState>, name: String) -> Reply<Snapshot> {
    commands::add_column(&state, &name)
}

#[tauri::command]
pub fn rename_column(
    state: State<'_, AppState>,
    column_id: ColumnId,
    name: String,
) -> Reply<Snapshot> {
    commands::rename_column(&state, column_id, &name)
}

#[tauri::command]
pub fn remove_column(state: State<'_, AppState>, column_id: ColumnId) -> Reply<Snapshot> {
    commands::remove_column(&state, column_id)
}

#[tauri::command]
pub fn move_column(
    state: State<'_, AppState>,
    column_id: ColumnId,
    to_index: usize,
) -> Reply<Snapshot> {
    commands::move_column(&state, column_id, to_index)
}

#[tauri::command]
pub fn set_column_wip_limit(
    state: State<'_, AppState>,
    column_id: ColumnId,
    wip_limit: String,
) -> Reply<Snapshot> {
    commands::set_column_wip_limit(&state, column_id, &wip_limit)
}

#[tauri::command]
pub fn archive_column(state: State<'_, AppState>, column_id: ColumnId) -> Reply<Snapshot> {
    commands::archive_column(&state, column_id)
}

// ---------------------------------------------------------------- タグ

#[tauri::command]
pub fn add_tag(state: State<'_, AppState>, name: String, color: String) -> Reply<Snapshot> {
    commands::add_tag(&state, &name, &color)
}

#[tauri::command]
pub fn rename_tag(state: State<'_, AppState>, tag_id: TagId, name: String) -> Reply<Snapshot> {
    commands::rename_tag(&state, tag_id, &name)
}

#[tauri::command]
pub fn set_tag_color(state: State<'_, AppState>, tag_id: TagId, color: String) -> Reply<Snapshot> {
    commands::set_tag_color(&state, tag_id, &color)
}

#[tauri::command]
pub fn remove_tag(state: State<'_, AppState>, tag_id: TagId) -> Reply<Snapshot> {
    commands::remove_tag(&state, tag_id)
}

// ---------------------------------------------------------------- 取り消し

#[tauri::command]
pub fn undo(state: State<'_, AppState>) -> Reply<Snapshot> {
    commands::undo(&state)
}

#[tauri::command]
pub fn redo(state: State<'_, AppState>) -> Reply<Snapshot> {
    commands::redo(&state)
}

// ---------------------------------------------------------------- 絞り込み

#[tauri::command]
pub fn filter_cards(
    state: State<'_, AppState>,
    query: String,
    tag_id: Option<TagId>,
) -> Vec<CardId> {
    commands::filter_cards(&state, &query, tag_id)
}

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
/// 文言は `Snapshot::window_title` が組んだものをそのまま受けます。ここに
/// 組み立てを書くと、同じ規則が Rust と TypeScript の 2 か所に散ります。
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
pub fn suggested_export_name(state: State<'_, AppState>, format: ExportFormat) -> String {
    commands::suggested_export_name(&state, format)
}

#[tauri::command]
pub fn export_board(
    state: State<'_, AppState>,
    format: ExportFormat,
    destination: PathBuf,
) -> Reply<PathBuf> {
    commands::export_board(&state, format, &destination)
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

// ---------------------------------------------------------------- 説明のリンク

#[tauri::command]
pub fn description_links(text: String) -> Vec<commands::UrlSpan> {
    commands::description_links(&text)
}

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

/// クイックキャプチャの入れ先。窓の見出しに出す「〇〇ボード / △△カラム」。
#[tauri::command]
pub fn capture_target(state: State<'_, AppState>) -> Reply<Option<CaptureTarget>> {
    commands::capture_target(&state)
}

/// 開いているボードのカラムをキャプチャ先にする。`None` で既定に戻す。
#[tauri::command]
pub fn set_capture_column(
    state: State<'_, AppState>,
    column_id: Option<ColumnId>,
) -> Reply<Snapshot> {
    commands::set_capture_column(&state, column_id)
}

/// この環境でグローバルホットキーを使えるか。使えないなら理由。
#[tauri::command]
pub fn quick_capture_support() -> Option<String> {
    crate::shortcut::platform_support().err()
}

/// 割り当てを差し替える。`None` で解除。保存された形が返る。
#[tauri::command]
pub fn set_quick_capture_shortcut_from_key(
    app: AppHandle,
    state: State<'_, AppState>,
    press: Option<KeyPress>,
) -> Reply<Option<String>> {
    crate::capture::set(&app, &state, press)
}

/// キャプチャの窓を閉じる。
///
/// 盤面には触らないので `commands` に置き場所がありません。窓の操作だけです。
#[tauri::command]
pub fn close_capture_window(app: AppHandle, focus_board: bool) {
    crate::capture::close(&app, focus_board);
}

#[tauri::command]
pub fn capture_card(app: AppHandle, state: State<'_, AppState>, title: String) -> Reply<Snapshot> {
    let snapshot = commands::capture_card(&state, &title)?;
    // ボードの窓は、自分が呼んでいないこの変更を知らない（`docs/DESIGN.md`「コマンドとイベント」）。
    if let Err(error) = app.emit_to(crate::run::BOARD_WINDOW, events::BOARD_CHANGED, &snapshot) {
        ekanban_core::diagnostics::log(&format!(
            "failed to tell the board about a capture: {error}"
        ));
    }
    Ok(snapshot)
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
