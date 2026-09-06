//! `#[tauri::command]` の包み。
//!
//! **中身はありません。** `commands` の関数をそのまま呼び、`AppState` を
//! `tauri::State` から取り出すだけです。判断がここに入りはじめたら、それは
//! `commands` に置き場所がなかったということなので、向こうに移してください。
//! §10 の開発用ハーネスは `commands` の側を HTTP に出すので、ここに書いたものは
//! ブラウザからは通りません。

use std::path::PathBuf;

use ekanban_core::db::{FilterState, WindowBoundsState};
use ekanban_core::model::{BoardId, CardId, ChecklistItemDraft, ColumnId, TagId};
use tauri::{State, WebviewWindow};

use crate::commands::{self, ExportFormat};
use crate::error::AppError;
use crate::snapshot::{Snapshot, StartupState, ThemePreference};
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
pub fn sort_column_by_due_date(state: State<'_, AppState>, column_id: ColumnId) -> Reply<Snapshot> {
    commands::sort_column_by_due_date(&state, column_id)
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

#[tauri::command]
pub fn reveal_database(state: State<'_, AppState>) -> PathBuf {
    commands::reveal_database(&state)
}

#[tauri::command]
pub fn reveal_backups(state: State<'_, AppState>) -> PathBuf {
    commands::reveal_backups(&state)
}

// ---------------------------------------------------------------- キャプチャ

#[tauri::command]
pub fn capture_card(state: State<'_, AppState>, title: String) -> Reply<Snapshot> {
    commands::capture_card(&state, &title)
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
