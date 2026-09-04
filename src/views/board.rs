use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use gpui_kit::{
    component::dialog::DialogButtonProps,
    component::input::{Input, InputState, Textarea, TextareaState},
    component::scroll::ScrollableElement as _,
    component::Disableable as _,
    component::Sizable,
    component::WindowExt as _,
    component::{
        button::{Button, ButtonVariant, ButtonVariants as _},
        ActiveTheme, Theme, ThemeMode,
    },
    div, point,
    prelude::*,
    px, rgb, App, Bounds, Context, DragMoveEvent, Entity, FocusHandle, Focusable as _, Half,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, Render, ScrollHandle,
    SharedString, Subscription, Window,
};

use crate::{
    actions::{
        About, AddBoard, AddCard, AddColumn, AddTag, BackupDatabase, CancelEdit, ClearSearch,
        CloseWindow, DeleteBoard, ExportBoardJson, ExportBoardMarkdown, FocusSearch, Redo,
        RenameBoard, RevealDatabase, SaveEdit, ShowAllCards, ShowOverdueCards, ShowThisWeekCards,
        ToggleArchiveView, ToggleFullscreen, Undo, UseDarkTheme, UseLightTheme, UseSystemTheme,
    },
    db::{save_board_snapshot, Database, DbError, FilterState, WindowBoundsState},
    model::{
        card_matches_search, due_status, parse_due_date, parse_wip_limit, Board, BoardError,
        BoardId, BoardSummary, Card, CardId, ChecklistItem, ChecklistItemDraft, ChecklistItemId,
        Column, ColumnId, DueStatus, Tag, TagId,
    },
};

#[derive(Clone, Copy)]
struct CardDrag {
    card_id: CardId,
}

#[derive(Clone, Copy)]
struct ColumnDrag {
    column_id: ColumnId,
}

struct CardDragPreview {
    title: SharedString,
    position: Point<Pixels>,
}

impl Render for CardDragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let preview_width = px(250.);
        let preview_height = px(64.);
        div()
            .pl(self.position.x - preview_width.half())
            .pt(self.position.y - preview_height.half())
            .child(
                div()
                    .w(preview_width)
                    .h(preview_height)
                    .p_3()
                    .flex()
                    .items_center()
                    .bg(theme_color(cx, UiColor::SurfaceHover))
                    .border_1()
                    .border_color(theme_color(cx, UiColor::Accent))
                    .rounded_lg()
                    .shadow_lg()
                    .text_color(theme_color(cx, UiColor::Foreground))
                    .child(self.title.clone()),
            )
    }
}

struct ColumnDragPreview {
    name: SharedString,
    position: Point<Pixels>,
}

struct CardEditor {
    card_id: CardId,
    title: Entity<InputState>,
    description: Entity<TextareaState>,
    due_date: Entity<InputState>,
    tag_ids: Vec<TagId>,
    checklist_items: Vec<ChecklistEditorItem>,
    error: Option<FieldError>,
}

struct ChecklistEditorItem {
    id: Option<ChecklistItemId>,
    text: Entity<InputState>,
    checked: bool,
}

struct ColumnEditor {
    column_id: Option<ColumnId>,
    name: Entity<InputState>,
    wip_limit: Entity<InputState>,
    error: Option<FieldError>,
}

struct TagEditor {
    tag_id: Option<TagId>,
    name: Entity<InputState>,
    color: Entity<InputState>,
    error: Option<FieldError>,
}

struct BoardEditor {
    board_id: Option<BoardId>,
    name: Entity<InputState>,
    error: Option<FieldError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StatusMessage {
    level: StatusLevel,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorContext {
    MoveCard,
    MoveColumn,
    Card,
    Column,
    Tag,
    Board,
    Undo,
    Redo,
}

impl ErrorContext {
    fn label(self) -> &'static str {
        match self {
            Self::MoveCard => "カードを移動できませんでした",
            Self::MoveColumn => "カラムを移動できませんでした",
            Self::Card => "カードを操作できませんでした",
            Self::Column => "カラムを操作できませんでした",
            Self::Tag => "タグを操作できませんでした",
            Self::Board => "ボードを操作できませんでした",
            Self::Undo => "操作を元に戻せませんでした",
            Self::Redo => "操作をやり直せませんでした",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorField {
    CardTitle,
    DueDate,
    ChecklistItem,
    ColumnName,
    WipLimit,
    TagName,
    BoardName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldError {
    field: EditorField,
    message: String,
    value: Option<String>,
}

enum SaveFailure {
    None,
    ClearCardEditor,
    RestoreCardEditor(CardEditor),
    RestoreColumnEditor(ColumnEditor),
    RestoreTagEditor(TagEditor),
    RestoreBoardEditor(BoardEditor),
    RestoreTagState {
        tag_id: TagId,
        editor: Option<TagEditor>,
        filter_was_selected: bool,
    },
}

struct PendingSave {
    id: u64,
    snapshot: Board,
    before: Board,
    success_message: String,
    on_failure: SaveFailure,
}

struct ActiveSave {
    id: u64,
    before: Board,
    success_message: String,
    on_failure: SaveFailure,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DueFilter {
    None,
    Overdue,
    ThroughThisWeek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ThemePreference {
    System,
    Light,
    Dark,
}

pub(crate) fn parse_theme_preference(value: Option<&str>) -> ThemePreference {
    match value {
        Some("light") => ThemePreference::Light,
        Some("dark") => ThemePreference::Dark,
        _ => ThemePreference::System,
    }
}

fn apply_theme_preference(preference: ThemePreference, window: Option<&mut Window>, cx: &mut App) {
    match preference {
        ThemePreference::System => Theme::sync_system_appearance(window, cx),
        ThemePreference::Light => Theme::change(ThemeMode::Light, window, cx),
        ThemePreference::Dark => Theme::change(ThemeMode::Dark, window, cx),
    }
    Theme::sync_scrollbar_appearance(cx);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CardDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Json,
    Markdown,
}

impl Render for ColumnDragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let preview_width = px(220.);
        let preview_height = px(48.);
        div()
            .pl(self.position.x - preview_width.half())
            .pt(self.position.y - preview_height.half())
            .child(
                div()
                    .w(preview_width)
                    .h(preview_height)
                    .p_3()
                    .flex()
                    .items_center()
                    .bg(theme_color(cx, UiColor::Accent))
                    .border_1()
                    .border_color(theme_color(cx, UiColor::Accent))
                    .rounded_lg()
                    .shadow_lg()
                    .text_color(theme_color(cx, UiColor::Foreground))
                    .child(self.name.clone()),
            )
    }
}

pub struct BoardView {
    board: Board,
    boards: Vec<BoardSummary>,
    database_path: PathBuf,
    focus_handle: FocusHandle,
    save_lock: Arc<Mutex<()>>,
    next_save_id: u64,
    pending_saves: VecDeque<PendingSave>,
    active_save: Option<ActiveSave>,
    status: Option<StatusMessage>,
    editing_card: Option<CardEditor>,
    editing_column: Option<ColumnEditor>,
    editing_tag: Option<TagEditor>,
    editing_board: Option<BoardEditor>,
    due_filter: DueFilter,
    tag_filter: Option<TagId>,
    show_archived: bool,
    selected_card: Option<CardId>,
    context_menu_card: Option<CardId>,
    context_menu_column: Option<ColumnId>,
    card_panel_menu_open: bool,
    board_scroll_handle: ScrollHandle,
    column_scroll_handles: HashMap<ColumnId, ScrollHandle>,
    window_bounds: WindowBoundsState,
    _window_bounds_subscription: Subscription,
    _appearance_subscription: Subscription,
    _app_quit_subscription: Subscription,
    theme_preference: ThemePreference,
    search: Entity<InputState>,
    search_query: String,
}

impl BoardView {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        board: Board,
        boards: Vec<BoardSummary>,
        database_path: PathBuf,
        filter_state: FilterState,
        window_bounds: WindowBoundsState,
        theme_preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("タイトル・説明を検索"));
        let due_filter = match filter_state.due_filter {
            1 => DueFilter::Overdue,
            2 => DueFilter::ThroughThisWeek,
            _ => DueFilter::None,
        };
        let search_query = filter_state.search;
        let search_query_for_input = search_query.clone();
        search.update(cx, |state, cx| {
            state.set_value(search_query_for_input, window, cx)
        });

        apply_theme_preference(theme_preference, Some(window), cx);
        Theme::sync_scrollbar_appearance(cx);
        let bounds_path = database_path.clone();
        let window_bounds_subscription =
            cx.observe_window_bounds(window, move |this, window, cx| {
                if !window.is_fullscreen() {
                    let bounds = window.bounds();
                    this.window_bounds = WindowBoundsState {
                        x: bounds.origin.x.as_f32(),
                        y: bounds.origin.y.as_f32(),
                        width: bounds.size.width.as_f32(),
                        height: bounds.size.height.as_f32(),
                    };
                    let path = bounds_path.clone();
                    let saved_bounds = this.window_bounds;
                    cx.background_spawn(async move {
                        let _ = Database::open(path)
                            .and_then(|database| database.set_window_bounds(saved_bounds));
                    })
                    .detach();
                }
            });
        let appearance_subscription = cx.observe_window_appearance(window, |this, window, cx| {
            if this.theme_preference == ThemePreference::System {
                apply_theme_preference(ThemePreference::System, Some(window), cx);
            }
        });
        let quit_path = database_path.clone();
        let app_quit_subscription = cx.on_app_quit(move |this, _| {
            let path = quit_path.clone();
            let bounds = this.window_bounds;
            async move {
                let _ =
                    Database::open(path).and_then(|database| database.set_window_bounds(bounds));
            }
        });

        Self {
            board,
            boards,
            database_path,
            focus_handle: cx.focus_handle(),
            save_lock: Arc::new(Mutex::new(())),
            next_save_id: 0,
            pending_saves: VecDeque::new(),
            active_save: None,
            status: None,
            editing_card: None,
            editing_column: None,
            editing_tag: None,
            editing_board: None,
            due_filter,
            tag_filter: filter_state.tag_id,
            show_archived: false,
            selected_card: None,
            context_menu_card: None,
            context_menu_column: None,
            card_panel_menu_open: false,
            board_scroll_handle: ScrollHandle::new(),
            column_scroll_handles: HashMap::new(),
            window_bounds,
            _window_bounds_subscription: window_bounds_subscription,
            _appearance_subscription: appearance_subscription,
            _app_quit_subscription: app_quit_subscription,
            theme_preference,
            search,
            search_query,
        }
    }

    fn set_status(&mut self, level: StatusLevel, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            level,
            text: text.into(),
        });
    }

    fn set_info(&mut self, text: impl Into<String>) {
        self.set_status(StatusLevel::Info, text);
    }

    fn set_success(&mut self, text: impl Into<String>) {
        self.set_status(StatusLevel::Success, text);
    }

    fn set_error(&mut self, text: impl Into<String>) {
        self.set_status(StatusLevel::Error, text);
    }

    fn persist_filter_state(&self, cx: &mut Context<Self>) {
        let state = FilterState {
            search: self.search_query.clone(),
            tag_id: self.tag_filter,
            due_filter: match self.due_filter {
                DueFilter::None => 0,
                DueFilter::Overdue => 1,
                DueFilter::ThroughThisWeek => 2,
            },
        };
        let path = self.database_path.clone();
        cx.background_spawn(async move {
            let _ = Database::open(path).and_then(|database| database.set_filter_state(&state));
        })
        .detach();
    }

    fn set_theme_preference(
        &mut self,
        preference: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.theme_preference == preference {
            return;
        }
        self.theme_preference = preference;
        apply_theme_preference(preference, Some(window), cx);
        let value = match preference {
            ThemePreference::System => "system",
            ThemePreference::Light => "light",
            ThemePreference::Dark => "dark",
        };
        let path = self.database_path.clone();
        cx.background_spawn(async move {
            let _ = Database::open(path).and_then(|database| database.set_theme_preference(value));
        })
        .detach();
        self.set_info(match preference {
            ThemePreference::System => "システムの外観に合わせます",
            ThemePreference::Light => "ライトモードに変更しました",
            ThemePreference::Dark => "ダークモードに変更しました",
        });
        cx.notify();
    }

    fn present_board_error(&mut self, context: ErrorContext, error: BoardError) {
        self.set_error(format!(
            "{}: {}",
            context.label(),
            board_error_detail(&error)
        ));
    }

    fn present_db_error(&mut self, context: &str, error: DbError) {
        self.set_error(format!("{context}: {}", db_error_detail(&error)));
    }

    fn rollback_board(&mut self, before: Board) {
        self.board = before;
        self.board.discard_pending_events();
        self.sync_current_board_summary();
    }

    fn sync_current_board_summary(&mut self) {
        if let Some(summary) = self
            .boards
            .iter_mut()
            .find(|summary| summary.id == self.board.id)
        {
            summary.name = self.board.name.clone();
            summary.created_at = self.board.created_at;
            summary.updated_at = self.board.updated_at;
        }
    }

    fn has_pending_save(&self) -> bool {
        self.active_save.is_some() || !self.pending_saves.is_empty()
    }

    fn reject_while_saving(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.has_pending_save() {
            return false;
        }
        self.set_info("保存が完了するまでボードを変更できません");
        cx.notify();
        true
    }

    fn reset_board_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_card = None;
        self.editing_column = None;
        self.editing_tag = None;
        self.editing_board = None;
        self.selected_card = None;
        self.context_menu_card = None;
        self.context_menu_column = None;
        self.due_filter = DueFilter::None;
        self.tag_filter = None;
        self.show_archived = false;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.search_query.clear();
        self.persist_filter_state(cx);
    }

    fn switch_board(&mut self, board_id: BoardId, window: &mut Window, cx: &mut Context<Self>) {
        if board_id == self.board.id {
            return;
        }
        if self.reject_while_saving(cx) {
            return;
        }
        if self.editing_card.is_some()
            || self.editing_column.is_some()
            || self.editing_tag.is_some()
            || self.editing_board.is_some()
        {
            self.set_info("編集中はボードを切り替えられません");
            cx.notify();
            return;
        }

        let result = Database::open(&self.database_path)
            .and_then(|database| database.load_board_by_id(board_id));
        match result {
            Ok(board) => {
                let name = board.name.clone();
                self.board = board;
                self.reset_board_view(window, cx);
                match Database::open(&self.database_path)
                    .and_then(|database| database.set_last_board_id(board_id))
                {
                    Ok(()) => self.set_success(format!("「{name}」に切り替えました")),
                    Err(error) => self.set_error(format!(
                        "ボードは切り替えましたが、選択状態の保存に失敗しました: {}",
                        db_error_detail(&error)
                    )),
                }
            }
            Err(error) => self.present_db_error("ボードを切り替えられませんでした", error),
        }
        cx.notify();
    }

    fn begin_add_board(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.reject_while_saving(cx) {
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("ボード名"));
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_board = Some(BoardEditor {
            board_id: None,
            name,
            error: None,
        });
        cx.notify();
    }

    fn begin_board_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.reject_while_saving(cx) {
            return;
        }
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("ボード名")
                .default_value(self.board.name.clone())
        });
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_board = Some(BoardEditor {
            board_id: Some(self.board.id),
            name,
            error: None,
        });
        cx.notify();
    }

    fn cancel_board_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_board.take().is_some() {
            self.set_info("ボード名の編集をキャンセルしました");
            cx.notify();
        }
    }

    fn save_board_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_board.take() else {
            return;
        };
        let name = editor.name.read(cx).value().to_string();
        match editor.board_id {
            None => {
                let result = Database::open(&self.database_path).and_then(|mut database| {
                    let board = database.create_board(name)?;
                    database.set_last_board_id(board.id)?;
                    Ok(board)
                });
                match result {
                    Ok(board) => {
                        let summary = BoardSummary {
                            id: board.id,
                            name: board.name.clone(),
                            created_at: board.created_at,
                            updated_at: board.updated_at,
                        };
                        self.boards.push(summary);
                        self.board = board;
                        self.reset_board_view(window, cx);
                        self.set_success("ボードを追加しました");
                    }
                    Err(error) => {
                        self.editing_board = Some(editor);
                        if let Some(editor) = self.editing_board.as_mut() {
                            editor.error = field_error_for_db(&error);
                        }
                        self.present_db_error("ボードを追加できませんでした", error);
                    }
                }
            }
            Some(board_id) => {
                let before = self.board.clone();
                match self.board.rename(name) {
                    Ok(false) => self.set_info("ボード名に変更はありません"),
                    Ok(true) => {
                        self.sync_current_board_summary();
                        self.enqueue_save(
                            before,
                            "ボード名を変更しました",
                            SaveFailure::RestoreBoardEditor(editor),
                            cx,
                        );
                    }
                    Err(error) => {
                        let field_error = field_error_for(&error);
                        self.editing_board = Some(editor);
                        if let Some(editor) = self.editing_board.as_mut() {
                            editor.error = field_error;
                        }
                        self.present_board_error(ErrorContext::Board, error);
                    }
                }
                debug_assert_eq!(board_id, self.board.id);
            }
        }
        cx.notify();
    }

    fn request_delete_board(
        &mut self,
        board_id: BoardId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.reject_while_saving(cx) {
            return;
        }
        let Some(summary) = self.boards.iter().find(|summary| summary.id == board_id) else {
            self.set_error("ボードが見つかりません。画面を更新してください。");
            cx.notify();
            return;
        };
        let board_name = summary.name.clone();
        let board_view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let board_view = board_view.clone();
            alert
                .confirm()
                .title("ボードを削除しますか？")
                .description(format!("「{board_name}」と、その中のカードを削除します。"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(ButtonVariant::Danger)
                        .ok_text("削除")
                        .cancel_text("キャンセル")
                        .show_cancel(true),
                )
                .on_ok(move |_, window, cx| {
                    board_view.update(cx, |this, cx| this.delete_board(board_id, window, cx));
                    true
                })
        });
    }

    fn delete_board(&mut self, board_id: BoardId, window: &mut Window, cx: &mut Context<Self>) {
        if self.reject_while_saving(cx) {
            return;
        }
        let result = Database::open(&self.database_path).and_then(|mut database| {
            database.delete_board(board_id)?;
            let boards = database.load_boards()?;
            Ok((boards, database))
        });
        match result {
            Ok((boards, database)) => {
                let deleting_current = self.board.id == board_id;
                self.boards = boards;
                if deleting_current {
                    let next_id = self
                        .boards
                        .first()
                        .map(|summary| summary.id)
                        .expect("deleting the last board is rejected");
                    match database.load_board_by_id(next_id) {
                        Ok(board) => {
                            self.board = board;
                            self.reset_board_view(window, cx);
                            if let Err(error) = database.set_last_board_id(next_id) {
                                self.set_error(format!(
                                    "ボードを削除して切り替えましたが、選択状態の保存に失敗しました: {}",
                                    db_error_detail(&error)
                                ));
                            } else {
                                self.set_success("ボードを削除しました");
                            }
                        }
                        Err(error) => {
                            self.present_db_error("切り替え先のボードを読み込めませんでした", error)
                        }
                    }
                } else {
                    self.set_success("ボードを削除しました");
                }
            }
            Err(error) => self.present_db_error("ボードを削除できませんでした", error),
        }
        cx.notify();
    }

    fn enqueue_save(
        &mut self,
        before: Board,
        success_message: impl Into<String>,
        on_failure: SaveFailure,
        cx: &mut Context<Self>,
    ) {
        self.next_save_id += 1;
        self.pending_saves.push_back(PendingSave {
            id: self.next_save_id,
            snapshot: self.board.clone(),
            before,
            success_message: success_message.into(),
            on_failure,
        });
        // The snapshot owns the events produced by this operation. New
        // operations append to the live board while this snapshot is being
        // written, so they can be saved independently by the next request.
        self.board.pending_events.clear();
        self.set_info("保存中…");
        self.start_next_save(cx);
    }

    fn start_next_save(&mut self, cx: &mut Context<Self>) {
        if self.active_save.is_some() {
            return;
        }
        let Some(pending) = self.pending_saves.pop_front() else {
            return;
        };

        let id = pending.id;
        let path = self.database_path.clone();
        let save_lock = self.save_lock.clone();
        let snapshot = pending.snapshot;
        self.active_save = Some(ActiveSave {
            id,
            before: pending.before,
            success_message: pending.success_message,
            on_failure: pending.on_failure,
        });
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = save_lock.lock().expect("save worker mutex was poisoned");
                    save_board_snapshot(path, snapshot)
                })
                .await;
            let _ = this.update(cx, |view, cx| view.finish_save(id, result, cx));
        })
        .detach();
    }

    fn finish_save(&mut self, id: u64, result: Result<(), DbError>, cx: &mut Context<Self>) {
        let Some(active) = self.active_save.take() else {
            return;
        };
        if active.id != id {
            self.active_save = Some(active);
            return;
        }
        match result {
            Ok(()) => {
                if self.pending_saves.is_empty() {
                    self.set_success(active.success_message);
                } else {
                    self.set_info("保存中…");
                }
                self.start_next_save(cx);
            }
            Err(error) => {
                // Requests are started only after the preceding request has
                // succeeded. Therefore all queued snapshots include the
                // failed state and must be discarded together with it.
                self.pending_saves.clear();
                self.rollback_board(active.before);
                match active.on_failure {
                    SaveFailure::None => {}
                    SaveFailure::ClearCardEditor => self.editing_card = None,
                    SaveFailure::RestoreCardEditor(editor) => self.editing_card = Some(editor),
                    SaveFailure::RestoreColumnEditor(editor) => self.editing_column = Some(editor),
                    SaveFailure::RestoreTagEditor(editor) => self.editing_tag = Some(editor),
                    SaveFailure::RestoreBoardEditor(editor) => self.editing_board = Some(editor),
                    SaveFailure::RestoreTagState {
                        tag_id,
                        editor,
                        filter_was_selected,
                    } => {
                        if filter_was_selected {
                            self.tag_filter = Some(tag_id);
                        }
                        self.editing_tag = editor;
                    }
                }
                self.sync_current_board_summary();
                self.present_db_error("保存に失敗しました", error);
            }
        }
        cx.notify();
    }

    fn select_card(&mut self, card_id: CardId, window: &mut Window, cx: &mut Context<Self>) {
        if !self
            .board
            .columns
            .iter()
            .any(|column| column.cards.iter().any(|card| card.id == card_id))
        {
            return;
        }
        self.selected_card = Some(card_id);
        self.context_menu_card = None;
        self.context_menu_column = None;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// カードをクリックしたときに詳細パネルを開く。
    ///
    /// 単一クリックで開くようになったぶん、編集中に隣のカードを押す事故が起きやすい。
    /// 未保存の入力を黙って捨てないよう、切り替える前に保存する。保存できない状態
    /// （タイトルが空、期限の書式が不正、チェックリストの項目名が空）なら切り替えない。
    fn open_card_panel(&mut self, card_id: CardId, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .editing_card
            .as_ref()
            .is_some_and(|editor| editor.card_id == card_id)
        {
            return;
        }
        if self.editing_card.is_some() {
            self.commit_card_edit(false, cx);
            if self.editing_card.is_some() {
                return;
            }
        }
        self.select_card(card_id, window, cx);
        if self.selected_card == Some(card_id) {
            self.begin_card_edit(card_id, window, cx);
        }
    }

    fn toggle_card_panel_menu(&mut self, cx: &mut Context<Self>) {
        self.card_panel_menu_open = !self.card_panel_menu_open;
        self.context_menu_card = None;
        self.context_menu_column = None;
        cx.notify();
    }

    /// パネルの ⋮ からのコピー。編集内容を保存してからコピーし、パネルは閉じる。
    fn copy_card_from_panel(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        self.card_panel_menu_open = false;
        self.commit_card_edit(false, cx);
        if self.editing_card.is_some() {
            return;
        }
        self.copy_card(card_id, cx);
    }

    fn card_edit_is_savable(&self, editor: &CardEditor, cx: &Context<Self>) -> bool {
        !editor.title.read(cx).value().trim().is_empty()
            && parse_due_date(editor.due_date.read(cx).value().as_ref()).is_ok()
            && !editor
                .checklist_items
                .iter()
                .any(|item| item.text.read(cx).value().trim().is_empty())
    }

    fn navigate_selection(
        &mut self,
        direction: CardDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next_card = next_card_id(&self.board.columns, self.selected_card, direction);
        if let Some(card_id) = next_card {
            self.selected_card = Some(card_id);
            self.reveal_selected_card();
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn reveal_selected_card(&self) {
        let Some((column_index, card_index)) = self.selected_card_location() else {
            return;
        };
        if let Some(column) = self.board.columns.get(column_index) {
            if let Some(handle) = self.column_scroll_handles.get(&column.id) {
                handle.scroll_to_item(card_index);
            }
        }
        self.board_scroll_handle.scroll_to_item(column_index);
    }

    fn selected_card_location(&self) -> Option<(usize, usize)> {
        self.board
            .columns
            .iter()
            .enumerate()
            .find_map(|(column_index, column)| {
                column
                    .cards
                    .iter()
                    .position(|card| Some(card.id) == self.selected_card)
                    .map(|card_index| (column_index, card_index))
            })
    }

    fn move_selected_card_between_columns(
        &mut self,
        direction: CardDirection,
        cx: &mut Context<Self>,
    ) {
        let Some(card_id) = self.selected_card else {
            return;
        };
        let Some((source_column_index, source_card_index)) = self.selected_card_location() else {
            self.selected_card = None;
            return;
        };

        let target_column_index = match direction {
            CardDirection::Left => source_column_index.checked_sub(1),
            CardDirection::Right => {
                let next = source_column_index + 1;
                (next < self.board.columns.len()).then_some(next)
            }
            CardDirection::Up | CardDirection::Down => Some(source_column_index),
        };
        let Some(target_column_index) = target_column_index else {
            return;
        };
        let target_column_id = self.board.columns[target_column_index].id;
        let target_index = match direction {
            CardDirection::Up => source_card_index.saturating_sub(1),
            CardDirection::Down => source_card_index + 2,
            CardDirection::Left | CardDirection::Right => {
                source_card_index.min(self.board.columns[target_column_index].cards.len())
            }
        };
        self.move_card(card_id, target_column_id, target_index, cx);
    }

    fn selection_after_removing(&self, card_id: CardId) -> Option<CardId> {
        let (column_index, card_index) =
            self.board
                .columns
                .iter()
                .enumerate()
                .find_map(|(column_index, column)| {
                    column
                        .cards
                        .iter()
                        .position(|card| card.id == card_id)
                        .map(|card_index| (column_index, card_index))
                })?;

        let column = &self.board.columns[column_index];
        column
            .cards
            .get(card_index + 1)
            .or_else(|| {
                card_index
                    .checked_sub(1)
                    .and_then(|index| column.cards.get(index))
            })
            .map(|card| card.id)
            .or_else(|| {
                self.board
                    .columns
                    .iter()
                    .flat_map(|column| column.cards.iter())
                    .find(|card| card.id != card_id)
                    .map(|card| card.id)
            })
    }

    fn keyboard_shortcuts_disabled(&self, window: &Window, cx: &Context<Self>) -> bool {
        self.editing_card.is_some()
            || self.editing_column.is_some()
            || self.editing_tag.is_some()
            || self.editing_board.is_some()
            || self.search.read(cx).focus_handle(cx).is_focused(window)
    }

    fn handle_board_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Text inputs own the key event while editing. This also keeps Enter,
        // Escape, and arrow keys from escaping during IME composition.
        if self.keyboard_shortcuts_disabled(window, cx) || self.show_archived {
            return;
        }

        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        if modifiers.platform && modifiers.alt && !modifiers.shift && !modifiers.control {
            match key {
                "left" => {
                    cx.stop_propagation();
                    self.move_selected_card_between_columns(CardDirection::Left, cx);
                }
                "right" => {
                    cx.stop_propagation();
                    self.move_selected_card_between_columns(CardDirection::Right, cx);
                }
                "up" => {
                    cx.stop_propagation();
                    self.move_selected_card_between_columns(CardDirection::Up, cx);
                }
                "down" => {
                    cx.stop_propagation();
                    self.move_selected_card_between_columns(CardDirection::Down, cx);
                }
                _ => {}
            }
            return;
        }

        if modifiers.modified() {
            return;
        }

        match key {
            "up" => {
                cx.stop_propagation();
                self.navigate_selection(CardDirection::Up, window, cx);
            }
            "down" => {
                cx.stop_propagation();
                self.navigate_selection(CardDirection::Down, window, cx);
            }
            "left" => {
                cx.stop_propagation();
                self.navigate_selection(CardDirection::Left, window, cx);
            }
            "right" => {
                cx.stop_propagation();
                self.navigate_selection(CardDirection::Right, window, cx);
            }
            "enter" => {
                if let Some(card_id) = self.selected_card {
                    cx.stop_propagation();
                    self.begin_card_edit(card_id, window, cx);
                }
            }
            "delete" | "backspace" => {
                if let Some(card_id) = self.selected_card {
                    cx.stop_propagation();
                    self.delete_card(card_id, cx);
                }
            }
            _ => {}
        }
    }

    fn move_card(
        &mut self,
        card_id: CardId,
        target_column_id: ColumnId,
        target_index: usize,
        cx: &mut Context<Self>,
    ) {
        let before = self.board.clone();
        match self
            .board
            .move_card(card_id, target_column_id, target_index)
        {
            Ok(false) => return,
            Ok(true) => {
                self.reveal_selected_card();
                self.enqueue_save(before, "保存しました", SaveFailure::None, cx)
            }
            Err(error) => self.present_board_error(ErrorContext::MoveCard, error),
        }
        cx.notify();
    }

    fn move_column(&mut self, column_id: ColumnId, target_index: usize, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.move_column(column_id, target_index) {
            Ok(false) => return,
            Ok(true) => self.enqueue_save(before, "カラムを並べ替えました", SaveFailure::None, cx),
            Err(error) => self.present_board_error(ErrorContext::MoveColumn, error),
        }
        cx.notify();
    }

    fn add_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let column_id = self
            .selected_card_location()
            .and_then(|(column_index, _)| self.board.columns.get(column_index))
            .map(|column| column.id)
            .or_else(|| self.board.columns.first().map(|column| column.id));
        let Some(column_id) = column_id else {
            return;
        };
        self.add_card_to_column(column_id, window, cx);
    }

    fn add_card_to_column(
        &mut self,
        column_id: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.show_archived {
            self.set_info("アーカイブ表示中はカードを追加できません");
            cx.notify();
            return;
        }
        let before = self.board.clone();
        let result = self
            .board
            .add_card(column_id, "新しいカード", "説明を追加してください");
        match result {
            Ok(card_id) => {
                self.begin_card_edit(card_id, window, cx);
                self.enqueue_save(
                    before,
                    "カードを追加しました",
                    SaveFailure::ClearCardEditor,
                    cx,
                );
            }
            Err(error) => self.present_board_error(ErrorContext::Card, error),
        }
        cx.notify();
    }

    fn begin_card_edit(&mut self, card_id: CardId, window: &mut Window, cx: &mut Context<Self>) {
        let Some((title, description, due_date, tag_ids, checklist_items)) = self
            .board
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .map(|card| {
                (
                    card.title.clone(),
                    card.description.clone(),
                    card.due_date
                        .map(|date| date.format("%Y-%m-%d").to_string())
                        .unwrap_or_default(),
                    card.tag_ids.clone(),
                    card.checklist_items.clone(),
                )
            })
        else {
            self.present_board_error(ErrorContext::Card, BoardError::CardNotFound(card_id));
            cx.notify();
            return;
        };

        self.selected_card = Some(card_id);
        self.card_panel_menu_open = false;
        let title_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("カードのタイトル")
                .default_value(title)
        });
        let description_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("カードの説明")
                .default_value(description)
        });
        let due_date_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("YYYY-MM-DD（任意）")
                .default_value(due_date)
        });
        let checklist_items = checklist_items
            .into_iter()
            .map(|item| {
                let text =
                    cx.new(|cx| InputState::new(window, cx).default_value(item.text.clone()));
                ChecklistEditorItem {
                    id: Some(item.id),
                    text,
                    checked: item.checked,
                }
            })
            .collect();
        title_input.update(cx, |state, cx| state.focus(window, cx));
        self.editing_card = Some(CardEditor {
            card_id,
            title: title_input,
            description: description_input,
            due_date: due_date_input,
            tag_ids,
            checklist_items,
            error: None,
        });
        cx.notify();
    }

    fn add_checklist_item_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_card.as_mut() else {
            return;
        };
        let text = cx.new(|cx| InputState::new(window, cx).placeholder("チェック項目"));
        let item = ChecklistEditorItem {
            id: None,
            text,
            checked: false,
        };
        editor.checklist_items.push(item);
        if let Some(item) = editor.checklist_items.last() {
            item.text.update(cx, |state, cx| state.focus(window, cx));
        }
        cx.notify();
    }

    fn toggle_checklist_item(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(item) = self
            .editing_card
            .as_mut()
            .and_then(|editor| editor.checklist_items.get_mut(index))
        {
            item.checked = !item.checked;
            cx.notify();
        }
    }

    fn delete_checklist_item_editor(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = self.editing_card.as_mut() {
            if index < editor.checklist_items.len() {
                editor.checklist_items.remove(index);
                cx.notify();
            }
        }
    }

    fn move_checklist_item_editor(
        &mut self,
        index: usize,
        direction: CardDirection,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self.editing_card.as_mut() else {
            return;
        };
        let target = match direction {
            CardDirection::Up => index.checked_sub(1),
            CardDirection::Down => (index + 1 < editor.checklist_items.len()).then_some(index + 1),
            CardDirection::Left | CardDirection::Right => None,
        };
        if let Some(target) = target {
            editor.checklist_items.swap(index, target);
            cx.notify();
        }
    }

    fn cancel_card_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_card.take().is_some() {
            self.set_info("カードの編集をキャンセルしました");
            cx.notify();
        }
    }

    fn save_card_edit(&mut self, cx: &mut Context<Self>) {
        self.commit_card_edit(true, cx);
    }

    /// カードの編集を確定する。`announce_unchanged` が false のときは
    /// 「変更はありません」を出さない。パネルを切り替えるたびに出るとうるさいため。
    fn commit_card_edit(&mut self, announce_unchanged: bool, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_card.take() else {
            return;
        };
        let title = editor.title.read(cx).value().to_string();
        let description = editor.description.read(cx).value().to_string();
        let due_date_text = editor.due_date.read(cx).value().to_string();
        let tag_ids = editor.tag_ids.clone();
        let checklist_drafts = editor
            .checklist_items
            .iter()
            .map(|item| ChecklistItemDraft {
                id: item.id,
                text: item.text.read(cx).value().to_string(),
                checked: item.checked,
            })
            .collect::<Vec<_>>();
        if checklist_drafts
            .iter()
            .any(|item| item.text.trim().is_empty())
        {
            self.editing_card = Some(editor);
            if let Some(editor) = self.editing_card.as_mut() {
                editor.error = field_error_for(&BoardError::EmptyChecklistItemText);
            }
            self.present_board_error(ErrorContext::Card, BoardError::EmptyChecklistItemText);
            cx.notify();
            return;
        }
        let due_date = match parse_due_date(&due_date_text) {
            Ok(due_date) => due_date,
            Err(error) => {
                self.editing_card = Some(editor);
                if let Some(editor) = self.editing_card.as_mut() {
                    editor.error = field_error_for(&error);
                }
                self.present_board_error(ErrorContext::Card, error);
                cx.notify();
                return;
            }
        };
        let before = self.board.clone();

        let changed = match self.board.update_card_details_with_checklist(
            editor.card_id,
            title,
            description,
            due_date,
            tag_ids,
            checklist_drafts,
        ) {
            Ok(changed) => changed,
            Err(error) => {
                self.editing_card = Some(editor);
                if let Some(editor) = self.editing_card.as_mut() {
                    editor.error = field_error_for(&error);
                }
                self.present_board_error(ErrorContext::Card, error);
                cx.notify();
                return;
            }
        };

        if changed {
            self.enqueue_save(
                before,
                "カードを更新しました",
                SaveFailure::RestoreCardEditor(editor),
                cx,
            );
        } else if announce_unchanged {
            self.set_info("カードに変更はありません");
        }
        cx.notify();
    }

    fn delete_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        let next_selection = if self.selected_card == Some(card_id) {
            self.selection_after_removing(card_id)
        } else {
            self.selected_card
        };
        let before = self.board.clone();
        match self.board.delete_card(card_id) {
            Ok(()) => {
                self.selected_card = next_selection;
                self.context_menu_card = None;
                let on_failure = if self
                    .editing_card
                    .as_ref()
                    .is_some_and(|editor| editor.card_id == card_id)
                {
                    self.editing_card
                        .take()
                        .map(SaveFailure::RestoreCardEditor)
                        .unwrap_or(SaveFailure::None)
                } else {
                    SaveFailure::None
                };
                self.enqueue_save(before, "カードを削除しました", on_failure, cx);
            }
            Err(error) => self.present_board_error(ErrorContext::Card, error),
        }
        cx.notify();
    }

    fn copy_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        if self.show_archived {
            return;
        }
        let before = self.board.clone();
        match self.board.copy_card(card_id) {
            Ok(new_card_id) => {
                self.selected_card = Some(new_card_id);
                self.context_menu_card = None;
                self.enqueue_save(before, "カードをコピーしました", SaveFailure::None, cx);
            }
            Err(error) => self.present_board_error(ErrorContext::Card, error),
        }
        cx.notify();
    }

    fn toggle_card_tag_from_menu(
        &mut self,
        card_id: CardId,
        tag_id: TagId,
        cx: &mut Context<Self>,
    ) {
        let Some(card) = self
            .board
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
        else {
            self.present_board_error(ErrorContext::Card, BoardError::CardNotFound(card_id));
            cx.notify();
            return;
        };
        let mut tag_ids = card.tag_ids.clone();
        if let Some(index) = tag_ids.iter().position(|id| *id == tag_id) {
            tag_ids.remove(index);
        } else {
            tag_ids.push(tag_id);
        }
        let before = self.board.clone();
        match self.board.set_card_tags(card_id, tag_ids) {
            Ok(false) => {}
            Ok(true) => {
                self.context_menu_card = None;
                self.enqueue_save(before, "タグを更新しました", SaveFailure::None, cx);
            }
            Err(error) => self.present_board_error(ErrorContext::Card, error),
        }
        cx.notify();
    }

    fn open_card_context_menu(
        &mut self,
        card_id: CardId,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Right {
            return;
        }
        cx.stop_propagation();
        self.selected_card = Some(card_id);
        self.context_menu_column = None;
        self.focus_handle.focus(window, cx);
        self.context_menu_card = Some(card_id);
        cx.notify();
    }

    fn toggle_column_context_menu(&mut self, column_id: ColumnId, cx: &mut Context<Self>) {
        self.context_menu_card = None;
        self.context_menu_column = if self.context_menu_column == Some(column_id) {
            None
        } else {
            Some(column_id)
        };
        cx.notify();
    }

    fn archive_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        let next_selection = if self.selected_card == Some(card_id) {
            self.selection_after_removing(card_id)
        } else {
            self.selected_card
        };
        let before = self.board.clone();
        match self.board.archive_card(card_id) {
            Ok(true) => {
                self.selected_card = next_selection;
                self.context_menu_card = None;
                let on_failure = if self
                    .editing_card
                    .as_ref()
                    .is_some_and(|editor| editor.card_id == card_id)
                {
                    self.editing_card
                        .take()
                        .map(SaveFailure::RestoreCardEditor)
                        .unwrap_or(SaveFailure::None)
                } else {
                    SaveFailure::None
                };
                self.enqueue_save(before, "カードをアーカイブしました", on_failure, cx);
            }
            Ok(false) => {}
            Err(error) => self.present_board_error(ErrorContext::Card, error),
        }
        cx.notify();
    }

    fn archive_column(&mut self, column_id: ColumnId, cx: &mut Context<Self>) {
        self.context_menu_column = None;
        let next_selection = if self.selected_card.is_some_and(|card_id| {
            self.board.columns.iter().any(|column| {
                column.id == column_id && column.cards.iter().any(|card| card.id == card_id)
            })
        }) {
            self.board
                .columns
                .iter()
                .filter(|column| column.id != column_id)
                .flat_map(|column| column.cards.iter())
                .map(|card| card.id)
                .next()
        } else {
            self.selected_card
        };
        let before = self.board.clone();
        match self.board.archive_column(column_id) {
            Ok(0) => self.set_info("アーカイブするカードがありません"),
            Ok(count) => {
                self.selected_card = next_selection;
                self.context_menu_card = None;
                let on_failure = self
                    .editing_card
                    .take()
                    .map(SaveFailure::RestoreCardEditor)
                    .unwrap_or(SaveFailure::None);
                self.enqueue_save(
                    before,
                    format!("{count} 枚をアーカイブしました"),
                    on_failure,
                    cx,
                );
            }
            Err(error) => self.present_board_error(ErrorContext::Column, error),
        }
        cx.notify();
    }

    fn request_archive_column(
        &mut self,
        column_id: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(column) = self
            .board
            .columns
            .iter()
            .find(|column| column.id == column_id)
        else {
            self.present_board_error(ErrorContext::Column, BoardError::ColumnNotFound(column_id));
            cx.notify();
            return;
        };
        let card_count = column.cards.len();
        if card_count == 0 {
            self.archive_column(column_id, cx);
            return;
        }

        self.context_menu_column = None;
        let board_view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let board_view = board_view.clone();
            alert
                .confirm()
                .title("カラムをアーカイブしますか？")
                .description(format!(
                    "このカラムの {card_count} 枚のカードをアーカイブします。"
                ))
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(ButtonVariant::Danger)
                        .ok_text("アーカイブ")
                        .cancel_text("キャンセル")
                        .show_cancel(true),
                )
                .on_ok(move |_, _, cx| {
                    board_view.update(cx, |this, cx| this.archive_column(column_id, cx));
                    true
                })
        });
    }

    fn restore_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.restore_card(card_id) {
            Ok(true) => self.enqueue_save(before, "カードを復元しました", SaveFailure::None, cx),
            Ok(false) => {}
            Err(error) => self.present_board_error(ErrorContext::Card, error),
        }
        cx.notify();
    }

    fn toggle_archive_view(&mut self, cx: &mut Context<Self>) {
        self.show_archived = !self.show_archived;
        self.editing_card = None;
        self.editing_column = None;
        self.selected_card = None;
        self.context_menu_card = None;
        self.context_menu_column = None;
        self.set_info(if self.show_archived {
            "アーカイブを表示しています"
        } else {
            "ボードを表示しています"
        });
        cx.notify();
    }

    fn set_due_date_input(
        &mut self,
        due_date: Option<NaiveDate>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = self.editing_card.as_ref() {
            let value = due_date
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            editor
                .due_date
                .update(cx, |state, cx| state.set_value(value, window, cx));
            cx.notify();
        }
    }

    fn toggle_card_tag(&mut self, tag_id: TagId, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_card.as_mut() else {
            return;
        };
        if let Some(index) = editor.tag_ids.iter().position(|id| *id == tag_id) {
            editor.tag_ids.remove(index);
        } else {
            editor.tag_ids.push(tag_id);
            editor.tag_ids.sort_unstable();
        }
        cx.notify();
    }

    fn begin_add_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("タグ名"));
        let color = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("#60a5fa")
                .default_value("#60a5fa")
        });
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_tag = Some(TagEditor {
            tag_id: None,
            name,
            color,
            error: None,
        });
        cx.notify();
    }

    fn begin_tag_edit(&mut self, tag_id: TagId, window: &mut Window, cx: &mut Context<Self>) {
        let Some((tag_name, tag_color)) = self
            .board
            .tags
            .iter()
            .find(|tag| tag.id == tag_id)
            .map(|tag| (tag.name.clone(), tag.color.clone()))
        else {
            self.present_board_error(ErrorContext::Tag, BoardError::TagNotFound(tag_id));
            cx.notify();
            return;
        };
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("タグ名")
                .default_value(tag_name)
        });
        let color = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("#60a5fa")
                .default_value(tag_color)
        });
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_tag = Some(TagEditor {
            tag_id: Some(tag_id),
            name,
            color,
            error: None,
        });
        cx.notify();
    }

    fn cancel_tag_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_tag.take().is_some() {
            self.set_info("タグの編集をキャンセルしました");
            cx.notify();
        }
    }

    fn save_tag_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_tag.take() else {
            return;
        };
        let name = editor.name.read(cx).value().to_string();
        let color = editor.color.read(cx).value().to_string();
        let before = self.board.clone();
        let result = (|| -> Result<bool, BoardError> {
            match editor.tag_id {
                Some(tag_id) => {
                    let name_changed = self.board.rename_tag(tag_id, name)?;
                    let color_changed = self.board.set_tag_color(tag_id, color)?;
                    Ok(name_changed || color_changed)
                }
                None => {
                    self.board.add_tag(name, color)?;
                    Ok(true)
                }
            }
        })();

        match result {
            Ok(false) => self.set_info("タグに変更はありません"),
            Ok(true) => self.enqueue_save(
                before,
                if editor.tag_id.is_some() {
                    "タグを更新しました"
                } else {
                    "タグを追加しました"
                },
                SaveFailure::RestoreTagEditor(editor),
                cx,
            ),
            Err(error) => {
                self.rollback_board(before);
                self.editing_tag = Some(editor);
                if let Some(editor) = self.editing_tag.as_mut() {
                    editor.error = field_error_for(&error);
                }
                self.present_board_error(ErrorContext::Tag, error);
            }
        }
        cx.notify();
    }

    fn delete_tag(&mut self, tag_id: TagId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.remove_tag(tag_id) {
            Ok(()) => {
                let filter_was_selected = self.tag_filter == Some(tag_id);
                let editor = if self
                    .editing_tag
                    .as_ref()
                    .is_some_and(|editor| editor.tag_id == Some(tag_id))
                {
                    self.editing_tag.take()
                } else {
                    None
                };
                if filter_was_selected {
                    self.tag_filter = None;
                    self.persist_filter_state(cx);
                }
                self.enqueue_save(
                    before,
                    "タグを削除しました",
                    SaveFailure::RestoreTagState {
                        tag_id,
                        editor,
                        filter_was_selected,
                    },
                    cx,
                );
            }
            Err(error) => self.present_board_error(ErrorContext::Tag, error),
        }
        cx.notify();
    }

    fn set_tag_filter(&mut self, tag_id: TagId, cx: &mut Context<Self>) {
        self.tag_filter = if self.tag_filter == Some(tag_id) {
            None
        } else {
            Some(tag_id)
        };
        self.persist_filter_state(cx);
        self.set_info("タグフィルターを変更しました");
        cx.notify();
    }

    fn begin_add_column(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_archived {
            self.set_info("アーカイブ表示中はカラムを追加できません");
            cx.notify();
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("カラム名"));
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_column = Some(ColumnEditor {
            column_id: None,
            name,
            wip_limit: cx.new(|cx| InputState::new(window, cx).placeholder("WIP 上限")),
            error: None,
        });
        cx.notify();
    }

    fn begin_column_edit(
        &mut self,
        column_id: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu_column = None;
        let Some((column_name, wip_limit)) = self
            .board
            .columns
            .iter()
            .find(|column| column.id == column_id)
            .map(|column| {
                (
                    column.name.clone(),
                    column
                        .wip_limit
                        .map(|limit| limit.to_string())
                        .unwrap_or_default(),
                )
            })
        else {
            self.present_board_error(ErrorContext::Column, BoardError::ColumnNotFound(column_id));
            cx.notify();
            return;
        };

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("カラム名")
                .default_value(column_name)
        });
        let wip_limit = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("WIP 上限")
                .default_value(wip_limit)
        });
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_column = Some(ColumnEditor {
            column_id: Some(column_id),
            name,
            wip_limit,
            error: None,
        });
        cx.notify();
    }

    fn cancel_column_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_column.take().is_some() {
            self.set_info("カラムの編集をキャンセルしました");
            cx.notify();
        }
    }

    fn save_column_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_column.take() else {
            return;
        };
        let name = editor.name.read(cx).value().to_string();
        let wip_limit_text = editor.wip_limit.read(cx).value().to_string();
        let wip_limit = match parse_wip_limit(&wip_limit_text) {
            Ok(wip_limit) => wip_limit,
            Err(error) => {
                self.editing_column = Some(editor);
                if let Some(editor) = self.editing_column.as_mut() {
                    editor.error = field_error_for(&error);
                }
                self.present_board_error(ErrorContext::Column, error);
                cx.notify();
                return;
            }
        };
        let before = self.board.clone();
        let result = (|| -> Result<bool, BoardError> {
            match editor.column_id {
                Some(column_id) => {
                    let name_changed = self.board.rename_column(column_id, name)?;
                    let wip_changed = self.board.set_column_wip_limit(column_id, wip_limit)?;
                    Ok(name_changed || wip_changed)
                }
                None => {
                    let column_id = self.board.add_column(name)?;
                    self.board.set_column_wip_limit(column_id, wip_limit)?;
                    Ok(true)
                }
            }
        })();

        match result {
            Ok(false) => {
                self.set_info("カラムに変更はありません");
            }
            Ok(true) => self.enqueue_save(
                before,
                if editor.column_id.is_some() {
                    "カラムを更新しました"
                } else {
                    "カラムを追加しました"
                },
                SaveFailure::RestoreColumnEditor(editor),
                cx,
            ),
            Err(error) => {
                self.editing_column = Some(editor);
                if let Some(editor) = self.editing_column.as_mut() {
                    editor.error = field_error_for(&error);
                }
                self.present_board_error(ErrorContext::Column, error);
            }
        }
        cx.notify();
    }

    fn request_delete_column(
        &mut self,
        column_id: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu_column = None;
        let Some(column) = self
            .board
            .columns
            .iter()
            .find(|column| column.id == column_id)
        else {
            self.present_board_error(ErrorContext::Column, BoardError::ColumnNotFound(column_id));
            cx.notify();
            return;
        };
        if column.cards.is_empty() {
            self.delete_column(column_id, cx);
            return;
        }

        let card_count = column.cards.len();
        let board_view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let board_view = board_view.clone();
            alert
                .confirm()
                .title("カラムを削除しますか？")
                .description(format!(
                    "このカラムには {card_count} 枚のカードがあります。削除するとカードも削除されます。"
                ))
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(ButtonVariant::Danger)
                        .ok_text("削除")
                        .cancel_text("キャンセル")
                        .show_cancel(true),
                )
                .on_ok(move |_, _, cx| {
                    board_view.update(cx, |this, cx| this.delete_column(column_id, cx));
                    true
                })
        });
    }

    fn delete_column(&mut self, column_id: ColumnId, cx: &mut Context<Self>) {
        self.context_menu_column = None;
        let next_selection = if self.selected_card.is_some_and(|card_id| {
            self.board.columns.iter().any(|column| {
                column.id == column_id && column.cards.iter().any(|card| card.id == card_id)
            })
        }) {
            self.board
                .columns
                .iter()
                .filter(|column| column.id != column_id)
                .flat_map(|column| column.cards.iter())
                .map(|card| card.id)
                .next()
        } else {
            self.selected_card
        };
        let before = self.board.clone();
        match self.board.remove_column(column_id) {
            Ok(()) => {
                self.selected_card = next_selection;
                let on_failure = if self
                    .editing_column
                    .as_ref()
                    .is_some_and(|editor| editor.column_id == Some(column_id))
                {
                    self.editing_column
                        .take()
                        .map(SaveFailure::RestoreColumnEditor)
                        .unwrap_or(SaveFailure::None)
                } else {
                    SaveFailure::None
                };
                self.enqueue_save(before, "カラムを削除しました", on_failure, cx);
            }
            Err(error) => self.present_board_error(ErrorContext::Column, error),
        }
        cx.notify();
    }

    fn sort_column_by_due_date(&mut self, column_id: ColumnId, cx: &mut Context<Self>) {
        self.context_menu_column = None;
        let before = self.board.clone();
        match self.board.sort_column_by_due_date(column_id) {
            Ok(false) => {
                self.set_info("期限順に変更はありません");
            }
            Ok(true) => self.enqueue_save(before, "期限順に並べ替えました", SaveFailure::None, cx),
            Err(error) => self.present_board_error(ErrorContext::Column, error),
        }
        cx.notify();
    }

    fn set_due_filter(&mut self, filter: DueFilter, cx: &mut Context<Self>) {
        self.due_filter = if self.due_filter == filter {
            DueFilter::None
        } else {
            filter
        };
        self.persist_filter_state(cx);
        self.set_info("表示フィルターを変更しました");
        cx.notify();
    }

    fn commit_search(&mut self, cx: &mut Context<Self>) {
        self.search_query = self.search.read(cx).value().to_string();
        if self.search_query.trim().is_empty() {
            self.set_info("検索をクリアしました");
        } else {
            self.set_info(format!("「{}」で検索中", self.search_query));
        }
        self.persist_filter_state(cx);
        cx.notify();
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.search_query.clear();
        self.persist_filter_state(cx);
        self.set_info("検索をクリアしました");
        cx.notify();
    }

    fn save_active_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_card.is_some() {
            self.save_card_edit(cx);
        } else if self.editing_column.is_some() {
            self.save_column_edit(cx);
        } else if self.editing_tag.is_some() {
            self.save_tag_edit(cx);
        } else if self.editing_board.is_some() {
            self.save_board_edit(window, cx);
        }
    }

    fn cancel_active_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_card.is_some() {
            self.cancel_card_edit(cx);
        } else if self.editing_column.is_some() {
            self.cancel_column_edit(cx);
        } else if self.editing_tag.is_some() {
            self.cancel_tag_edit(cx);
        } else if self.editing_board.is_some() {
            self.cancel_board_edit(cx);
        }
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_card.is_some()
            || self.editing_column.is_some()
            || self.editing_tag.is_some()
            || self.editing_board.is_some()
            || self.search.read(cx).focus_handle(cx).is_focused(window)
        {
            self.set_info("編集中は元に戻せません");
            cx.notify();
            return;
        }

        let before = self.board.clone();
        match self.board.undo() {
            Ok(false) => self.set_info("元に戻す操作がありません"),
            Ok(true) => self.enqueue_save(before, "元に戻しました", SaveFailure::None, cx),
            Err(error) => self.present_board_error(ErrorContext::Undo, error),
        }
        cx.notify();
    }

    fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing_card.is_some()
            || self.editing_column.is_some()
            || self.editing_tag.is_some()
            || self.editing_board.is_some()
            || self.search.read(cx).focus_handle(cx).is_focused(window)
        {
            self.set_info("編集中はやり直せません");
            cx.notify();
            return;
        }

        let before = self.board.clone();
        match self.board.redo() {
            Ok(false) => self.set_info("やり直す操作がありません"),
            Ok(true) => self.enqueue_save(before, "やり直しました", SaveFailure::None, cx),
            Err(error) => self.present_board_error(ErrorContext::Redo, error),
        }
        cx.notify();
    }

    fn choose_export_path(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        let directory = self
            .database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let suggested_name = match format {
            ExportFormat::Json => suggested_export_name(&self.board.name, "json"),
            ExportFormat::Markdown => suggested_export_name(&self.board.name, "md"),
        };
        let receiver = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let board = self.board.clone();
        let database_path = self.database_path.clone();
        let save_lock = self.save_lock.clone();

        cx.spawn(async move |this, cx| {
            let mut destination = match receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.set_error(format!("保存先を選択できませんでした: {error}"));
                        cx.notify();
                    });
                    return;
                }
                Err(_) => {
                    let _ = this.update(cx, |view, cx| {
                        view.set_error("保存先の選択が中断されました");
                        cx.notify();
                    });
                    return;
                }
            };
            if destination.extension().is_none() {
                destination.set_extension(match format {
                    ExportFormat::Json => "json",
                    ExportFormat::Markdown => "md",
                });
            }

            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = save_lock.lock().expect("save worker mutex was poisoned");
                    let content = match format {
                        ExportFormat::Json => Database::open(&database_path)
                            .and_then(|database| database.export_board_json(&board))
                            .map_err(|error| error.to_string()),
                        ExportFormat::Markdown => Ok(render_board_markdown(&board)),
                    }?;
                    std::fs::write(&destination, content).map_err(|error| error.to_string())
                })
                .await;

            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(()) => view.set_success("ボードを書き出しました"),
                    Err(error) => view.set_error(format!("ボードを書き出せませんでした: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn backup_database(&mut self, cx: &mut Context<Self>) {
        let directory = self
            .database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let receiver = cx.prompt_for_new_path(&directory, Some("ekanban-backup.sqlite3"));
        let source = self.database_path.clone();
        let save_lock = self.save_lock.clone();

        cx.spawn(async move |this, cx| {
            let mut destination = match receiver.await {
                Ok(Ok(Some(path))) => path,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    let _ = this.update(cx, |view, cx| {
                        view.set_error(format!("保存先を選択できませんでした: {error}"));
                        cx.notify();
                    });
                    return;
                }
                Err(_) => {
                    let _ = this.update(cx, |view, cx| {
                        view.set_error("保存先の選択が中断されました");
                        cx.notify();
                    });
                    return;
                }
            };
            if destination.extension().is_none() {
                destination.set_extension("sqlite3");
            }
            if destination == source {
                let _ = this.update(cx, |view, cx| {
                    view.set_error("バックアップ先には別のファイルを指定してください");
                    cx.notify();
                });
                return;
            }

            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = save_lock.lock().expect("save worker mutex was poisoned");
                    Database::open(&source)
                        .and_then(|database| database.backup_to(&destination))
                        .map_err(|error| error.to_string())
                })
                .await;

            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(()) => view.set_success("データベースをバックアップしました"),
                    Err(error) => view.set_error(format!(
                        "データベースをバックアップできませんでした: {error}"
                    )),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reveal_database(&mut self, cx: &mut Context<Self>) {
        cx.reveal_path(&self.database_path);
        self.set_info("データベースの場所を開きました");
        cx.notify();
    }

    fn show_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, |alert, _, _| {
            alert
                .title("ekanbanについて")
                .description("ローカル SQLite で動作する Kanban アプリです。")
                .button_props(DialogButtonProps::default().ok_text("OK"))
        });
    }

    fn render_board_editor(
        &self,
        editor: &BoardEditor,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name_value = editor.name.read(cx).value().to_string();
        let name_error = if name_value.trim().is_empty() {
            Some("ボード名を入力してください".to_string())
        } else {
            field_error_message(editor.error.as_ref(), EditorField::BoardName, &name_value)
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        this.save_board_edit(window, cx);
                    }
                    "escape" => {
                        cx.stop_propagation();
                        this.cancel_board_edit(cx);
                    }
                    _ => {}
                }
            }))
            .child(themed_input(Input::new(&editor.name).small(), cx))
            .when_some(name_error, |this, message| {
                this.child(field_error_note(
                    message,
                    theme_color(cx, UiColor::DangerForeground),
                ))
            })
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("save-board-edit")
                            .primary()
                            .disabled(name_value.trim().is_empty())
                            .label("保存")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_board_edit(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("cancel-board-edit")
                            .secondary()
                            .label("取消")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_board_edit(cx))),
                    ),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editing_board = self.editing_board.as_ref();
        div()
            .w(px(220.))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .border_r_1()
            .border_color(theme_color(cx, UiColor::Border))
            .bg(theme_color(cx, UiColor::Sidebar))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .child("ボード"),
                    )
                    .child(Button::new("add-board").secondary().label("＋").on_click(
                        cx.listener(|this, _, window, cx| this.begin_add_board(window, cx)),
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .overflow_y_scrollbar()
                    .children(self.boards.iter().map(|summary| {
                        let board_id = summary.id;
                        let selected = board_id == self.board.id;
                        div()
                            .id(("board-item", board_id as u64))
                            .w_full()
                            .p_2()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(if selected {
                                theme_color(cx, UiColor::SidebarAccent)
                            } else {
                                theme_color(cx, UiColor::Surface)
                            })
                            .hover(|this| {
                                this.bg(if selected {
                                    theme_color(cx, UiColor::Accent)
                                } else {
                                    theme_color(cx, UiColor::SurfaceHover)
                                })
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.switch_board(board_id, window, cx)
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme_color(cx, UiColor::Foreground))
                                    .child(summary.name.clone()),
                            )
                    }))
                    .when(self.boards.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme_color(cx, UiColor::MutedForeground))
                                .child("ボードがありません"),
                        )
                    }),
            )
            .child(if let Some(editor) = editing_board {
                self.render_board_editor(editor, cx).into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        Button::new("rename-board")
                            .secondary()
                            .label("名前を変更")
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.begin_board_edit(window, cx)
                                }),
                            ),
                    )
                    .child(
                        Button::new("delete-board")
                            .secondary()
                            .disabled(self.boards.len() <= 1)
                            .label("ボードを削除")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.request_delete_board(this.board.id, window, cx)
                            })),
                    )
                    .into_any_element()
            })
    }

    fn render_search(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        this.commit_search(cx);
                    }
                    "escape" => {
                        cx.stop_propagation();
                        this.clear_search(window, cx);
                    }
                    _ => {}
                }
            }))
            .child(themed_input(Input::new(&self.search).small(), cx))
            .when(!self.search_query.is_empty(), |this| {
                this.child(
                    Button::new("clear-search")
                        .ghost()
                        .label("クリア")
                        .on_click(cx.listener(|this, _, window, cx| this.clear_search(window, cx))),
                )
            })
    }

    fn render_column_editor(
        &self,
        editor: &ColumnEditor,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let editor_kind = editor.column_id;
        let name_value = editor.name.read(cx).value().to_string();
        let name_error = if name_value.trim().is_empty() {
            Some("カラム名を入力してください".to_string())
        } else {
            field_error_message(editor.error.as_ref(), EditorField::ColumnName, &name_value)
        };
        let wip_limit_value = editor.wip_limit.read(cx).value().to_string();
        let wip_limit_invalid = parse_wip_limit(&wip_limit_value).is_err();
        let wip_limit_error = if wip_limit_invalid {
            Some("WIP は正の整数、または空欄で入力してください".to_string())
        } else {
            field_error_message(
                editor.error.as_ref(),
                EditorField::WipLimit,
                &wip_limit_value,
            )
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        this.save_column_edit(cx);
                    }
                    "escape" => {
                        cx.stop_propagation();
                        this.cancel_column_edit(cx);
                    }
                    _ => {}
                }
            }))
            .child(themed_input(Input::new(&editor.name).small(), cx))
            .when_some(name_error, |this, message| {
                this.child(field_error_note(
                    message,
                    theme_color(cx, UiColor::DangerForeground),
                ))
            })
            .child(themed_input(Input::new(&editor.wip_limit).small(), cx))
            .when_some(wip_limit_error, |this, message| {
                this.child(field_error_note(
                    message,
                    theme_color(cx, UiColor::DangerForeground),
                ))
            })
            .child(
                Button::new(("save-column", editor_kind.unwrap_or(0) as u64))
                    .primary()
                    .disabled(wip_limit_invalid || name_value.trim().is_empty())
                    .label("保存")
                    .on_click(cx.listener(|this, _, _, cx| this.save_column_edit(cx))),
            )
            .child(
                Button::new(("cancel-column", editor_kind.unwrap_or(0) as u64))
                    .secondary()
                    .label("取消")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_column_edit(cx))),
            )
    }

    fn render_tag_editor(&self, editor: &TagEditor, cx: &mut Context<Self>) -> impl IntoElement {
        let editor_kind = editor.tag_id;
        let name_value = editor.name.read(cx).value().to_string();
        let name_error = if name_value.trim().is_empty() {
            Some("タグ名を入力してください".to_string())
        } else {
            field_error_message(editor.error.as_ref(), EditorField::TagName, &name_value)
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        cx.stop_propagation();
                        this.save_tag_edit(cx);
                    }
                    "escape" => {
                        cx.stop_propagation();
                        this.cancel_tag_edit(cx);
                    }
                    _ => {}
                }
            }))
            .child(themed_input(Input::new(&editor.name).small(), cx))
            .when_some(name_error, |this, message| {
                this.child(field_error_note(
                    message,
                    theme_color(cx, UiColor::DangerForeground),
                ))
            })
            .child(themed_input(Input::new(&editor.color).small(), cx))
            .child(
                Button::new(("save-tag", editor_kind.unwrap_or(0) as u64))
                    .primary()
                    .disabled(name_value.trim().is_empty())
                    .label("保存")
                    .on_click(cx.listener(|this, _, _, cx| this.save_tag_edit(cx))),
            )
            .child(
                Button::new(("cancel-tag", editor_kind.unwrap_or(0) as u64))
                    .secondary()
                    .label("取消")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_tag_edit(cx))),
            )
    }

    fn render_tag_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editing_tag = self.editing_tag.as_ref();
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("タグ"),
            )
            .children(self.board.tags.iter().map(|tag| {
                let tag_id = tag.id;
                let selected = self.tag_filter == Some(tag_id);
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("filter-tag", tag_id as u64))
                            .secondary()
                            .label(format!("{}{}", if selected { "✓ " } else { "" }, tag.name))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.set_tag_filter(tag_id, cx)),
                            ),
                    )
                    .child(
                        Button::new(("edit-tag", tag_id as u64))
                            .ghost()
                            .label("編集")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.begin_tag_edit(tag_id, window, cx)
                            })),
                    )
                    .child(
                        Button::new(("delete-tag", tag_id as u64))
                            .secondary()
                            .label("削除")
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.delete_tag(tag_id, cx)),
                            ),
                    )
            }))
            .child(if let Some(editor) = editing_tag {
                self.render_tag_editor(editor, cx).into_any_element()
            } else {
                Button::new("add-tag")
                    .secondary()
                    .label("＋ タグ")
                    .on_click(cx.listener(|this, _, window, cx| this.begin_add_tag(window, cx)))
                    .into_any_element()
            })
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (status_icon, status_color, status_background, status_text) = match self.status.as_ref()
        {
            Some(status) => match status.level {
                StatusLevel::Info => (
                    "ⓘ",
                    theme_color(cx, UiColor::InfoForeground),
                    theme_color(cx, UiColor::Info),
                    status.text.clone(),
                ),
                StatusLevel::Success => (
                    "✓",
                    theme_color(cx, UiColor::SuccessForeground),
                    theme_color(cx, UiColor::Success),
                    status.text.clone(),
                ),
                StatusLevel::Error => (
                    "⚠",
                    theme_color(cx, UiColor::DangerForeground),
                    theme_color(cx, UiColor::Danger),
                    status.text.clone(),
                ),
            },
            None => (
                "●",
                theme_color(cx, UiColor::MutedForeground),
                theme_color(cx, UiColor::Surface),
                "ローカル SQLite".to_string(),
            ),
        };
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .border_b_1()
            .border_color(theme_color(cx, UiColor::Border))
            .bg(theme_color(cx, UiColor::Background))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .child(self.board.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(status_background)
                            .text_xs()
                            .text_color(status_color)
                            .child(format!("{status_icon} {status_text}")),
                    )
                    .child(self.render_search(cx))
                    .child(self.render_tag_bar(cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("filter-none")
                            .secondary()
                            .label(if self.due_filter == DueFilter::None {
                                "✓ すべて"
                            } else {
                                "すべて"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_due_filter(DueFilter::None, cx)
                            })),
                    )
                    .child(
                        Button::new("filter-overdue")
                            .secondary()
                            .label(if self.due_filter == DueFilter::Overdue {
                                "✓ 期限切れ"
                            } else {
                                "期限切れ"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_due_filter(DueFilter::Overdue, cx)
                            })),
                    )
                    .child(
                        Button::new("filter-week")
                            .secondary()
                            .label(if self.due_filter == DueFilter::ThroughThisWeek {
                                "✓ 今週まで"
                            } else {
                                "今週まで"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_due_filter(DueFilter::ThroughThisWeek, cx)
                            })),
                    )
                    .child(
                        Button::new("archive-view")
                            .secondary()
                            .label(format!("アーカイブ ({})", self.board.archived_cards.len()))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_archive_view(cx))),
                    )
                    .child(if self.show_archived {
                        Button::new("back-to-board")
                            .primary()
                            .label("ボードへ戻る")
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_archive_view(cx)))
                            .into_any_element()
                    } else {
                        Button::new("add-card")
                            .primary()
                            .label("＋ カードを追加")
                            .on_click(cx.listener(|this, _, window, cx| this.add_card(window, cx)))
                            .into_any_element()
                    }),
            )
    }

    fn render_column(
        &self,
        column_index: usize,
        column: &Column,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let column_id = column.id;
        let end_index = column.cards.len();
        let scroll_handle = self
            .column_scroll_handles
            .get(&column_id)
            .cloned()
            .expect("column scroll handle initialized before rendering");
        let column_name = SharedString::from(column.name.clone());
        let is_editing = self
            .editing_column
            .as_ref()
            .is_some_and(|editor| editor.column_id == Some(column_id));
        let last_column = self.board.columns.len() == 1;
        let wip_over = column
            .wip_limit
            .is_some_and(|limit| column.cards.len() as i64 > limit);
        let card_count_label = column
            .wip_limit
            .map(|limit| format!("{} / {limit}", column.cards.len()))
            .unwrap_or_else(|| format!("{} cards", column.cards.len()));
        let header_content = if is_editing {
            self.render_column_editor(
                self.editing_column.as_ref().expect("editing column exists"),
                cx,
            )
            .into_any_element()
        } else {
            div()
                .id(("column-drag-handle", column_id as u64))
                .flex_1()
                .min_w_0()
                .cursor_move()
                .on_drag(ColumnDrag { column_id }, move |_, position, _, cx| {
                    cx.new(|_| ColumnDragPreview {
                        name: column_name.clone(),
                        position,
                    })
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(theme_color(cx, UiColor::Foreground))
                        .child(column.name.clone()),
                )
                .into_any_element()
        };
        div()
            .id(("column", column_id as u64))
            .w(px(280.))
            .flex_none()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded_lg()
            .bg(theme_color(cx, UiColor::Surface))
            .border_1()
            .border_color(theme_color(cx, UiColor::Border))
            .on_drop(cx.listener(move |this, drag: &CardDrag, _, cx| {
                this.move_card(drag.card_id, column_id, end_index, cx);
            }))
            .on_drop(cx.listener(move |this, drag: &ColumnDrag, _, cx| {
                this.move_column(drag.column_id, column_index, cx);
            }))
            .drag_over::<CardDrag>({
                let color = theme_color(cx, UiColor::Accent);
                move |style, _, _, _| style.border_color(color)
            })
            .drag_over::<ColumnDrag>({
                let color = theme_color(cx, UiColor::Accent);
                move |style, _, _, _| style.border_color(color)
            })
            .child(
                div()
                    .id(("column-header", column_id as u64))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .child(header_content)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if wip_over {
                                        theme_color(cx, UiColor::DangerForeground)
                                    } else {
                                        theme_color(cx, UiColor::MutedForeground)
                                    })
                                    .child(card_count_label),
                            )
                            .when(!is_editing, |this| {
                                this.child(
                                    Button::new(("column-menu", column_id as u64))
                                        .ghost()
                                        .compact()
                                        .label("…")
                                        .accessibility_label("カラムメニュー")
                                        .tooltip("カラムメニュー")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.toggle_column_context_menu(column_id, cx)
                                        })),
                                )
                            }),
                    ),
            )
            .h_full()
            .child(
                div()
                    .id(("column-cards", column_id as u64))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .overflow_y_scroll()
                    .track_scroll(&scroll_handle)
                    .vertical_scrollbar(&scroll_handle)
                    .on_drag_move({
                        let scroll_handle = scroll_handle.clone();
                        move |event: &DragMoveEvent<CardDrag>, _, _| {
                            let position = event.event.position;
                            if position.y < event.bounds.top() + px(48.)
                                || position.y > event.bounds.bottom() - px(48.)
                            {
                                // GPUI reports the current content offset in the
                                // handle, so this remains smooth while the drag
                                // pointer is held near an edge.
                                let offset = scroll_handle.offset();
                                let max_offset = scroll_handle.max_offset();
                                let y = if position.y < event.bounds.top() + px(48.) {
                                    (offset.y + px(20.)).min(px(0.))
                                } else {
                                    (offset.y - px(20.)).max(-max_offset.y)
                                };
                                scroll_handle.set_offset(point(offset.x, y));
                            }
                        }
                    })
                    .children(column.cards.iter().enumerate().map(|(index, card)| {
                        self.render_card(
                            column_id,
                            index,
                            card,
                            !card_matches_filter(card, self.due_filter, Local::now().date_naive())
                                || (!self.search_query.is_empty()
                                    && !card_matches_search(card, &self.search_query))
                                || self
                                    .tag_filter
                                    .is_some_and(|tag_id| !card.tag_ids.contains(&tag_id)),
                            cx,
                        )
                    }))
                    .child(
                        div()
                            .h(px(40.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .border_1()
                            .border_dashed()
                            .border_color(theme_color(cx, UiColor::Border))
                            .text_xs()
                            .text_color(theme_color(cx, UiColor::MutedForeground))
                            .child("ここにドロップ"),
                    ),
            )
            .child(
                Button::new(("add-card-to-column", column_id as u64))
                    .secondary()
                    .label("＋ カードを追加")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.add_card_to_column(column_id, window, cx)
                    })),
            )
            .when(self.context_menu_column == Some(column_id), |this| {
                this.child(self.render_column_context_menu(column_id, last_column, cx))
            })
    }

    fn render_column_context_menu(
        &self,
        column_id: ColumnId,
        last_column: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(("column-context-menu", column_id as u64))
            .absolute()
            .top(px(44.))
            .right(px(8.))
            .w(px(180.))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme_color(cx, UiColor::Border))
            .bg(theme_color(cx, UiColor::Popover))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(("context-sort-column", column_id as u64))
                    .ghost()
                    .label("期限順")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.sort_column_by_due_date(column_id, cx)
                    })),
            )
            .child(
                Button::new(("context-archive-column", column_id as u64))
                    .ghost()
                    .label("アーカイブ")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.request_archive_column(column_id, window, cx)
                    })),
            )
            .child(
                Button::new(("context-edit-column", column_id as u64))
                    .ghost()
                    .label("編集")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_column_edit(column_id, window, cx)
                    })),
            )
            .child(
                Button::new(("context-delete-column", column_id as u64))
                    .danger()
                    .disabled(last_column)
                    .label("削除")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.request_delete_column(column_id, window, cx)
                    })),
            )
    }

    fn render_archived(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        div()
            .id("archived-content")
            .flex_1()
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .overflow_y_scroll()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui_kit::FontWeight::BOLD)
                    .child("アーカイブ済みカード"),
            )
            .when(self.board.archived_cards.is_empty(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(theme_color(cx, UiColor::MutedForeground))
                        .child("アーカイブ済みのカードはありません"),
                )
            })
            .children(
                self.board
                    .archived_cards
                    .iter()
                    .map(|card| self.render_archived_card(card, today, cx)),
            )
    }

    fn render_archived_card(
        &self,
        card: &Card,
        today: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card_id = card.id;
        let dimmed = (!self.search_query.is_empty()
            && !card_matches_search(card, &self.search_query))
            || !card_matches_filter(card, self.due_filter, today)
            || self
                .tag_filter
                .is_some_and(|tag_id| !card.tag_ids.contains(&tag_id));
        div()
            .w_full()
            .max_w(px(720.))
            .p_3()
            .flex()
            .items_start()
            .justify_between()
            .gap_3()
            .rounded_md()
            .bg(theme_color(cx, UiColor::Surface))
            .border_1()
            .border_color(theme_color(cx, UiColor::Border))
            .when(dimmed, |this| this.opacity(0.35))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme_color(cx, UiColor::Foreground))
                            .child(card.title.clone()),
                    )
                    .when_some(
                        card.due_date.map(|due_date| {
                            render_due_badge(due_date, today, cx.theme()).into_any_element()
                        }),
                        |this, badge| this.child(badge),
                    )
                    .children(
                        card.tag_ids
                            .iter()
                            .filter_map(|tag_id| {
                                self.board.tags.iter().find(|tag| tag.id == *tag_id)
                            })
                            .map(|tag| render_tag_chip(tag, theme_color(cx, UiColor::Foreground))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme_color(cx, UiColor::MutedForeground))
                            .child(card.description.clone()),
                    ),
            )
            .child(
                Button::new(("restore-card", card_id as u64))
                    .primary()
                    .label("復元")
                    .on_click(cx.listener(move |this, _, _, cx| this.restore_card(card_id, cx))),
            )
    }

    fn render_add_column(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self
            .editing_column
            .as_ref()
            .filter(|editor| editor.column_id.is_none());
        div()
            .id("add-column-placeholder")
            .w(px(280.))
            .h(px(120.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_dashed()
            .border_color(theme_color(cx, UiColor::Border))
            .child(if let Some(editor) = editor {
                self.render_column_editor(editor, cx).into_any_element()
            } else {
                Button::new("add-column")
                    .secondary()
                    .label("＋ カラムを追加")
                    .on_click(cx.listener(|this, _, window, cx| this.begin_add_column(window, cx)))
                    .into_any_element()
            })
    }

    fn render_card_context_menu(
        &self,
        card_id: CardId,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card = self
            .board
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id);
        let tag_ids = card.map(|card| card.tag_ids.clone()).unwrap_or_default();
        div()
            .absolute()
            .top(px(8.))
            .left(px(8.))
            .w(px(190.))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme_color(cx, UiColor::Border))
            .bg(theme_color(cx, UiColor::Popover))
            .child(
                Button::new(("context-copy", card_id as u64))
                    .secondary()
                    .label("コピー")
                    .on_click(cx.listener(move |this, _, _, cx| this.copy_card(card_id, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("タグ"),
            )
            .children(self.board.tags.iter().map(|tag| {
                let tag_id = tag.id;
                let selected = tag_ids.contains(&tag_id);
                Button::new(format!("context-tag-{card_id}-{tag_id}"))
                    .ghost()
                    .label(format!(
                        "{}{}",
                        if selected { "✓ " } else { "□ " },
                        tag.name
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_card_tag_from_menu(card_id, tag_id, cx)
                    }))
            }))
            .child(
                Button::new(("context-archive", card_id as u64))
                    .ghost()
                    .label("アーカイブ")
                    .on_click(cx.listener(move |this, _, _, cx| this.archive_card(card_id, cx))),
            )
            .child(
                Button::new(("context-delete", card_id as u64))
                    .danger()
                    .label("削除")
                    .on_click(cx.listener(move |this, _, _, cx| this.delete_card(card_id, cx))),
            )
    }

    fn render_card(
        &self,
        column_id: ColumnId,
        index: usize,
        card: &Card,
        dimmed: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card_id = card.id;
        let title = card.title.clone();
        let drag_title = SharedString::from(card.title.clone());
        let today = Local::now().date_naive();
        let due_badge = card
            .due_date
            .map(|due_date| render_due_badge(due_date, today, cx.theme()).into_any_element());
        let is_selected = self.selected_card == Some(card_id);
        let context_menu_open = self.context_menu_card == Some(card_id);
        div()
            .id(("card", card_id as u64))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .relative()
            .bg(theme_color(cx, UiColor::Surface))
            .border_1()
            .border_color(theme_color(cx, UiColor::Border))
            .hover({
                let color = theme_color(cx, UiColor::SurfaceHover);
                move |this| this.bg(color)
            })
            .when(is_selected, {
                let color = theme_color(cx, UiColor::Accent);
                move |this| this.border_color(color)
            })
            .when(dimmed, |this| this.opacity(0.35))
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_card_panel(card_id, window, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.open_card_context_menu(card_id, event, window, cx)
                }),
            )
            .on_drop(cx.listener(move |this, drag: &CardDrag, _, cx| {
                this.move_card(drag.card_id, column_id, index, cx);
            }))
            .drag_over::<CardDrag>({
                let color = theme_color(cx, UiColor::Accent);
                move |style, _, _, _| style.border_color(color)
            })
            .child(
                div()
                    .id(("card-handle", card_id as u64))
                    .cursor_move()
                    .on_drag(CardDrag { card_id }, move |_, position, _, cx| {
                        cx.new(|_| CardDragPreview {
                            title: drag_title.clone(),
                            position,
                        })
                    })
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme_color(cx, UiColor::Foreground))
                            .child(title),
                    )
                    .when_some(due_badge, |this, badge| this.child(badge))
                    .when(!card.checklist_items.is_empty(), |this| {
                        this.child(render_checklist_progress(
                            &card.checklist_items,
                            theme_color(cx, UiColor::MutedForeground),
                        ))
                    })
                    .children(
                        card.tag_ids
                            .iter()
                            .filter_map(|tag_id| {
                                self.board.tags.iter().find(|tag| tag.id == *tag_id)
                            })
                            .map(|tag| render_tag_chip(tag, theme_color(cx, UiColor::Foreground))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme_color(cx, UiColor::MutedForeground))
                            .child(card.description.clone()),
                    ),
            )
            .when(context_menu_open, |this| {
                this.child(self.render_card_context_menu(card_id, cx))
            })
    }

    /// カードの詳細パネル。ボードに重ねず、右端に押し出して置く。
    /// 重ねると右端のカラムが隠れ、ドロップ先が見えなくなるため。
    fn render_card_panel(&self, editor: &CardEditor, cx: &mut Context<Self>) -> impl IntoElement {
        let savable = self.card_edit_is_savable(editor, cx);
        div()
            .id("card-panel")
            .w(px(380.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme_color(cx, UiColor::Surface))
            .border_l_1()
            .border_color(theme_color(cx, UiColor::Border))
            .child(self.render_card_panel_header(editor.card_id, cx))
            .child(
                div()
                    .id("card-panel-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_3()
                    .child(self.render_card_editor(editor, cx)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .p_3()
                    .border_t_1()
                    .border_color(theme_color(cx, UiColor::Border))
                    .child(
                        Button::new("cancel-card-edit")
                            .secondary()
                            .label("キャンセル")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_card_edit(cx))),
                    )
                    .child(
                        Button::new("save-card-edit")
                            .primary()
                            .disabled(!savable)
                            .label("保存")
                            .on_click(cx.listener(|this, _, _, cx| this.save_card_edit(cx))),
                    ),
            )
    }

    fn render_card_panel_header(
        &self,
        card_id: CardId,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let column_name = column_name_for_card(&self.board.columns, card_id)
            .unwrap_or("カラム不明")
            .to_string();
        let menu_open = self.card_panel_menu_open;
        div()
            .id("card-panel-header")
            .relative()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(theme_color(cx, UiColor::Border))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme_color(cx, UiColor::MutedForeground))
                            .child(format!("{column_name} のカード")),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(theme_color(cx, UiColor::Foreground))
                            .child(format!("#{card_id}")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("card-panel-menu").ghost().label("⋮").on_click(
                            cx.listener(|this, _, _, cx| this.toggle_card_panel_menu(cx)),
                        ),
                    )
                    .child(
                        Button::new("card-panel-close")
                            .ghost()
                            .label("✕")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_card_edit(cx))),
                    ),
            )
            .when(menu_open, |this| {
                this.child(self.render_card_panel_menu(card_id, cx))
            })
    }

    /// 常用しない操作はここに畳む（`docs/DESIGN.md`「常用しない操作を画面に常時出さない」）。
    fn render_card_panel_menu(&self, card_id: CardId, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("card-panel-menu-popup")
            .absolute()
            .top(px(48.))
            .right(px(8.))
            .w(px(180.))
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(theme_color(cx, UiColor::Border))
            .bg(theme_color(cx, UiColor::Popover))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Button::new(("panel-copy", card_id as u64))
                    .ghost()
                    .label("コピー")
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.copy_card_from_panel(card_id, cx)),
                    ),
            )
            .child(
                Button::new(("panel-archive", card_id as u64))
                    .ghost()
                    .label("アーカイブ")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.card_panel_menu_open = false;
                        this.archive_card(card_id, cx)
                    })),
            )
            .child(
                Button::new(("panel-delete", card_id as u64))
                    .danger()
                    .label("削除")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.card_panel_menu_open = false;
                        this.delete_card(card_id, cx)
                    })),
            )
    }

    fn render_card_editor(&self, editor: &CardEditor, cx: &mut Context<Self>) -> impl IntoElement {
        let title_value = editor.title.read(cx).value().to_string();
        let title_error = if title_value.trim().is_empty() {
            Some("タイトルを入力してください".to_string())
        } else {
            field_error_message(editor.error.as_ref(), EditorField::CardTitle, &title_value)
        };
        let due_date_value = editor.due_date.read(cx).value().to_string();
        let due_date_invalid = parse_due_date(&due_date_value).is_err();
        let due_date_error = if due_date_invalid {
            Some("YYYY-MM-DD 形式で入力してください（空欄で期限なし）".to_string())
        } else {
            field_error_message(editor.error.as_ref(), EditorField::DueDate, &due_date_value)
        };
        let today = Local::now().date_naive();
        let tomorrow = today + Duration::days(1);
        let days_until_saturday = (Weekday::Sat.num_days_from_monday() as i64
            - today.weekday().num_days_from_monday() as i64
            + 7)
            % 7;
        let weekend = today + Duration::days(days_until_saturday);
        let next_week = today + Duration::days(7 - today.weekday().num_days_from_monday() as i64);
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("タイトル"),
            )
            .child(themed_input(Input::new(&editor.title).small(), cx))
            .when_some(title_error, |this, message| {
                this.child(field_error_note(
                    message,
                    theme_color(cx, UiColor::DangerForeground),
                ))
            })
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("説明"),
            )
            .child(themed_textarea(
                Textarea::new(&editor.description).h(px(96.)),
                cx,
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("期限"),
            )
            .child(themed_input(Input::new(&editor.due_date).small(), cx))
            .when_some(due_date_error, |this, message| {
                this.child(field_error_note(
                    message,
                    theme_color(cx, UiColor::DangerForeground),
                ))
            })
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new(("due-today", editor.card_id as u64))
                            .secondary()
                            .label("今日")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_due_date_input(Some(today), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("due-tomorrow", editor.card_id as u64))
                            .secondary()
                            .label("明日")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_due_date_input(Some(tomorrow), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("due-weekend", editor.card_id as u64))
                            .secondary()
                            .label("今週末")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_due_date_input(Some(weekend), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("due-next-week", editor.card_id as u64))
                            .secondary()
                            .label("来週")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_due_date_input(Some(next_week), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("due-clear", editor.card_id as u64))
                            .secondary()
                            .label("クリア")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_due_date_input(None, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("チェックリスト"),
            )
            .children(
                editor
                    .checklist_items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new(format!("checklist-toggle-{}-{index}", editor.card_id))
                                    .secondary()
                                    .label(if item.checked { "☑" } else { "□" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_checklist_item(index, cx)
                                    })),
                            )
                            .child(themed_input(Input::new(&item.text).small(), cx))
                            .child(
                                Button::new(format!("checklist-up-{}-{index}", editor.card_id))
                                    .ghost()
                                    .disabled(index == 0)
                                    .label("↑")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_checklist_item_editor(
                                            index,
                                            CardDirection::Up,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(format!("checklist-down-{}-{index}", editor.card_id))
                                    .ghost()
                                    .disabled(index + 1 >= editor.checklist_items.len())
                                    .label("↓")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_checklist_item_editor(
                                            index,
                                            CardDirection::Down,
                                            cx,
                                        )
                                    })),
                            )
                            .child(
                                Button::new(format!("checklist-delete-{}-{index}", editor.card_id))
                                    .secondary()
                                    .label("削除")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.delete_checklist_item_editor(index, cx)
                                    })),
                            )
                            .when(item.text.read(cx).value().trim().is_empty(), |this| {
                                this.child(field_error_note(
                                    "項目名を入力してください".to_string(),
                                    theme_color(cx, UiColor::DangerForeground),
                                ))
                            })
                    }),
            )
            .child(
                Button::new(("checklist-add", editor.card_id as u64))
                    .secondary()
                    .label("＋ 項目を追加")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.add_checklist_item_editor(window, cx)
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child("タグ"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .children(self.board.tags.iter().map(|tag| {
                        let tag_id = tag.id;
                        let selected = editor.tag_ids.contains(&tag_id);
                        Button::new(("card-tag", tag_id as u64))
                            .secondary()
                            .label(format!("{}{}", if selected { "✓ " } else { "" }, tag.name))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.toggle_card_tag(tag_id, cx)),
                            )
                    })),
            )
    }
}

impl Render for BoardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for column in &self.board.columns {
            self.column_scroll_handles.entry(column.id).or_default();
        }
        let column_count = self.board.columns.len();
        let board_scroll_handle = self.board_scroll_handle.clone();
        let card_panel = if self.show_archived {
            None
        } else {
            self.editing_card
                .as_ref()
                .map(|editor| self.render_card_panel(editor, cx).into_any_element())
        };
        div()
            .key_context("Board")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_board_key_down(event, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &AddBoard, window, cx| this.begin_add_board(window, cx)),
            )
            .on_action(cx.listener(|this, _: &About, window, cx| this.show_about(window, cx)))
            .on_action(cx.listener(|this, _: &AddCard, window, cx| this.add_card(window, cx)))
            .on_action(
                cx.listener(|this, _: &AddColumn, window, cx| this.begin_add_column(window, cx)),
            )
            .on_action(cx.listener(|this, _: &AddTag, window, cx| this.begin_add_tag(window, cx)))
            .on_action(cx.listener(|this, _: &CancelEdit, _, cx| this.cancel_active_edit(cx)))
            .on_action(
                cx.listener(|this, _: &ClearSearch, window, cx| this.clear_search(window, cx)),
            )
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |state, cx| state.focus(window, cx));
            }))
            .on_action(
                cx.listener(|this, _: &SaveEdit, window, cx| this.save_active_edit(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &RenameBoard, window, cx| this.begin_board_edit(window, cx)),
            )
            .on_action(cx.listener(|this, _: &DeleteBoard, window, cx| {
                this.request_delete_board(this.board.id, window, cx)
            }))
            .on_action(cx.listener(|this, _: &Undo, window, cx| this.undo(window, cx)))
            .on_action(cx.listener(|this, _: &Redo, window, cx| this.redo(window, cx)))
            .on_action(cx.listener(|this, _: &ExportBoardJson, _, cx| {
                this.choose_export_path(ExportFormat::Json, cx)
            }))
            .on_action(cx.listener(|this, _: &ExportBoardMarkdown, _, cx| {
                this.choose_export_path(ExportFormat::Markdown, cx)
            }))
            .on_action(cx.listener(|this, _: &BackupDatabase, _, cx| this.backup_database(cx)))
            .on_action(cx.listener(|this, _: &RevealDatabase, _, cx| this.reveal_database(cx)))
            .on_action(cx.listener(|this, _: &UseLightTheme, window, cx| {
                this.set_theme_preference(ThemePreference::Light, window, cx)
            }))
            .on_action(cx.listener(|this, _: &UseDarkTheme, window, cx| {
                this.set_theme_preference(ThemePreference::Dark, window, cx)
            }))
            .on_action(cx.listener(|this, _: &UseSystemTheme, window, cx| {
                this.set_theme_preference(ThemePreference::System, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &ShowAllCards, _, cx| {
                    this.set_due_filter(DueFilter::None, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ToggleArchiveView, _, cx| this.toggle_archive_view(cx)),
            )
            .on_action(cx.listener(|this, _: &ShowOverdueCards, _, cx| {
                this.set_due_filter(DueFilter::Overdue, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowThisWeekCards, _, cx| {
                this.set_due_filter(DueFilter::ThroughThisWeek, cx)
            }))
            .on_action(cx.listener(|_, _: &ToggleFullscreen, window, _| {
                window.toggle_fullscreen();
            }))
            .size_full()
            .flex()
            .bg(theme_color(cx, UiColor::Background))
            .text_color(theme_color(cx, UiColor::Foreground))
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.render_header(cx))
                    .child(if self.show_archived {
                        self.render_archived(cx).into_any_element()
                    } else {
                        div()
                            .id("board-content")
                            .flex_1()
                            .flex()
                            .gap_4()
                            .p_6()
                            .overflow_x_scroll()
                            .track_scroll(&board_scroll_handle)
                            .horizontal_scrollbar(&board_scroll_handle)
                            .on_drag_move({
                                let board_scroll_handle = board_scroll_handle.clone();
                                move |event: &DragMoveEvent<CardDrag>, _, _| {
                                    auto_scroll_horizontal(
                                        &board_scroll_handle,
                                        event.event.position,
                                        event.bounds,
                                    );
                                }
                            })
                            .on_drag_move({
                                let board_scroll_handle = board_scroll_handle.clone();
                                move |event: &DragMoveEvent<ColumnDrag>, _, _| {
                                    auto_scroll_horizontal(
                                        &board_scroll_handle,
                                        event.event.position,
                                        event.bounds,
                                    );
                                }
                            })
                            .on_drop(cx.listener(move |this, drag: &ColumnDrag, _, cx| {
                                this.move_column(drag.column_id, column_count, cx);
                            }))
                            .children(
                                self.board
                                    .columns
                                    .iter()
                                    .enumerate()
                                    .map(|(index, column)| self.render_column(index, column, cx)),
                            )
                            .child(self.render_add_column(cx))
                            .into_any_element()
                    }),
            )
            .children(card_panel)
    }
}

#[derive(Clone, Copy)]
enum UiColor {
    Background,
    InputBackground,
    Surface,
    SurfaceHover,
    Foreground,
    MutedForeground,
    Border,
    Accent,
    Danger,
    DangerForeground,
    Success,
    SuccessForeground,
    Info,
    InfoForeground,
    Sidebar,
    SidebarAccent,
    Popover,
}

fn theme_color(cx: &gpui_kit::App, color: UiColor) -> gpui_kit::Hsla {
    let theme = cx.theme();
    match color {
        UiColor::Background => theme.background,
        UiColor::InputBackground => theme.input_background(),
        UiColor::Surface => theme.colors.list,
        UiColor::SurfaceHover => theme.colors.list_hover,
        UiColor::Foreground => theme.foreground,
        UiColor::MutedForeground => theme.muted_foreground,
        UiColor::Border => theme.border,
        UiColor::Accent => theme.accent,
        UiColor::Danger => theme.danger,
        UiColor::DangerForeground => theme.danger_foreground,
        UiColor::Success => theme.success,
        UiColor::SuccessForeground => theme.success_foreground,
        UiColor::Info => theme.info,
        UiColor::InfoForeground => theme.info_foreground,
        UiColor::Sidebar => theme.sidebar,
        UiColor::SidebarAccent => theme.sidebar_accent,
        UiColor::Popover => theme.popover,
    }
}

fn themed_input(input: Input, cx: &gpui_kit::App) -> Input {
    input
        .bg(theme_color(cx, UiColor::InputBackground))
        .text_color(theme_color(cx, UiColor::Foreground))
}

fn themed_textarea(textarea: Textarea, cx: &gpui_kit::App) -> Textarea {
    textarea
        .bg(theme_color(cx, UiColor::InputBackground))
        .text_color(theme_color(cx, UiColor::Foreground))
}

fn auto_scroll_horizontal(handle: &ScrollHandle, position: Point<Pixels>, bounds: Bounds<Pixels>) {
    let edge = px(48.);
    let offset = handle.offset();
    let max_offset = handle.max_offset();
    let next_x = if position.x < bounds.left() + edge {
        (offset.x + px(20.)).min(px(0.))
    } else if position.x > bounds.right() - edge {
        (offset.x - px(20.)).max(-max_offset.x)
    } else {
        return;
    };
    handle.set_offset(point(next_x, offset.y));
}

fn next_card_id(
    columns: &[Column],
    selected_card: Option<CardId>,
    direction: CardDirection,
) -> Option<CardId> {
    let first_card = || {
        columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .map(|card| card.id)
            .next()
    };
    let Some(selected_card) = selected_card else {
        return first_card();
    };
    let Some((column_index, card_index)) =
        columns
            .iter()
            .enumerate()
            .find_map(|(column_index, column)| {
                column
                    .cards
                    .iter()
                    .position(|card| card.id == selected_card)
                    .map(|card_index| (column_index, card_index))
            })
    else {
        return first_card();
    };

    match direction {
        CardDirection::Up => card_index
            .checked_sub(1)
            .and_then(|index| columns[column_index].cards.get(index))
            .map(|card| card.id),
        CardDirection::Down => columns[column_index]
            .cards
            .get(card_index + 1)
            .map(|card| card.id),
        CardDirection::Left | CardDirection::Right => {
            let step = if direction == CardDirection::Left {
                -1
            } else {
                1
            };
            let mut target = column_index as isize + step;
            while target >= 0 && (target as usize) < columns.len() {
                let column = &columns[target as usize];
                if !column.cards.is_empty() {
                    return column
                        .cards
                        .get(card_index.min(column.cards.len() - 1))
                        .map(|card| card.id);
                }
                target += step;
            }
            None
        }
    }
}

/// カードが今どのカラムにあるかを返す。詳細パネルのヘッダに出す。
fn column_name_for_card(columns: &[Column], card_id: CardId) -> Option<&str> {
    columns
        .iter()
        .find(|column| column.cards.iter().any(|card| card.id == card_id))
        .map(|column| column.name.as_str())
}

fn render_board_markdown(board: &Board) -> String {
    let mut markdown = format!("# {}\n\n", markdown_inline(&board.name));
    for column in &board.columns {
        markdown.push_str(&format!("## {}\n\n", markdown_inline(&column.name)));
        if column.cards.is_empty() {
            markdown.push_str("カードはありません。\n\n");
            continue;
        }
        for card in &column.cards {
            append_markdown_card(&mut markdown, card, board, None);
        }
    }

    if !board.archived_cards.is_empty() {
        markdown.push_str("## アーカイブ\n\n");
        for card in &board.archived_cards {
            let column_name = board
                .columns
                .iter()
                .find(|column| column.id == card.column_id)
                .map(|column| column.name.as_str());
            append_markdown_card(&mut markdown, card, board, column_name);
        }
    }

    markdown
}

fn suggested_export_name(board_name: &str, extension: &str) -> String {
    let stem = board_name
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let stem = stem.trim().trim_matches('.');
    let stem = if stem.is_empty() { "board" } else { stem };
    format!("{stem}.{extension}")
}

fn append_markdown_card(
    markdown: &mut String,
    card: &Card,
    board: &Board,
    column_name: Option<&str>,
) {
    markdown.push_str(&format!("- **{}**\n", markdown_inline(&card.title)));

    let mut metadata = Vec::new();
    if let Some(column_name) = column_name {
        metadata.push(format!("カラム: {}", markdown_inline(column_name)));
    }
    if let Some(due_date) = card.due_date {
        metadata.push(format!("期限: {due_date}"));
    }
    let tag_names = card
        .tag_ids
        .iter()
        .filter_map(|tag_id| board.tags.iter().find(|tag| tag.id == *tag_id))
        .map(|tag| markdown_inline(&tag.name))
        .collect::<Vec<_>>();
    if !tag_names.is_empty() {
        metadata.push(format!("タグ: {}", tag_names.join(", ")));
    }
    if card.archived_at.is_some() {
        metadata.push("アーカイブ済み".to_string());
    }
    for line in metadata {
        markdown.push_str(&format!("  - {line}\n"));
    }

    if !card.description.trim().is_empty() {
        for line in card.description.lines() {
            markdown.push_str(&format!("  > {}\n", markdown_inline(line)));
        }
    }
    for item in &card.checklist_items {
        let marker = if item.checked { 'x' } else { ' ' };
        markdown.push_str(&format!("  - [{marker}] {}\n", markdown_inline(&item.text)));
    }
    markdown.push('\n');
}

fn markdown_inline(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(['\r', '\n'], " ")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn render_due_badge(due_date: NaiveDate, today: NaiveDate, theme: &Theme) -> impl IntoElement {
    let status = due_status(Some(due_date), today);
    let (label, color) = match status {
        DueStatus::Overdue(days) => (
            format!("期限切れ {days}日 ({})", short_date(due_date)),
            theme.danger_foreground,
        ),
        DueStatus::Today => (
            format!("期限 今日 ({})", short_date(due_date)),
            theme.warning,
        ),
        DueStatus::Soon(days) => (
            format!("期限 あと {days}日 ({})", short_date(due_date)),
            theme.accent,
        ),
        DueStatus::Upcoming(_) => (
            format!("期限 {}", display_date(due_date, today)),
            theme.muted_foreground,
        ),
        DueStatus::None => return div(),
    };
    div().text_xs().text_color(color).child(label)
}

fn render_checklist_progress(
    items: &[ChecklistItem],
    text_color: gpui_kit::Hsla,
) -> impl IntoElement {
    let checked = items.iter().filter(|item| item.checked).count();
    let progress = format!(
        "{} {checked}/{}",
        items
            .iter()
            .map(|item| if item.checked { '■' } else { '□' })
            .collect::<String>(),
        items.len()
    );
    div().text_xs().text_color(text_color).child(progress)
}

fn render_tag_chip(tag: &Tag, text_color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .px_1()
        .rounded_sm()
        .bg(rgb(tag_color_value(&tag.color)))
        .text_xs()
        .text_color(text_color)
        .child(tag.name.clone())
}

fn tag_color_value(color: &str) -> u32 {
    let value = color.trim().trim_start_matches('#');
    if value.len() == 6 {
        u32::from_str_radix(value, 16).unwrap_or(0x64748b)
    } else {
        0x64748b
    }
}

fn short_date(date: NaiveDate) -> String {
    format!("{}/{}", date.month(), date.day())
}

fn display_date(date: NaiveDate, today: NaiveDate) -> String {
    if date.year() == today.year() {
        short_date(date)
    } else {
        format!("{}/{:02}/{:02}", date.year(), date.month(), date.day())
    }
}

fn card_matches_filter(card: &Card, filter: DueFilter, today: NaiveDate) -> bool {
    match filter {
        DueFilter::None => true,
        DueFilter::Overdue => matches!(due_status(card.due_date, today), DueStatus::Overdue(_)),
        DueFilter::ThroughThisWeek => {
            let Some(due_date) = card.due_date else {
                return false;
            };
            let days_until_sunday = 6 - i64::from(today.weekday().num_days_from_monday());
            due_date <= today + Duration::days(days_until_sunday)
        }
    }
}

fn board_error_detail(error: &BoardError) -> String {
    match error {
        BoardError::EmptyBoardName => "ボード名を入力してください".to_string(),
        BoardError::ColumnNotFound(column_id) => {
            format!("カラム #{column_id} が見つかりません。画面を更新してください")
        }
        BoardError::CardNotFound(card_id) => {
            format!("カード #{card_id} が見つかりません。画面を更新してください")
        }
        BoardError::EmptyCardTitle => "タイトルを入力してください".to_string(),
        BoardError::EmptyColumnName => "カラム名を入力してください".to_string(),
        BoardError::InvalidDueDate(value) => {
            format!("期限「{value}」は YYYY-MM-DD 形式で入力してください")
        }
        BoardError::InvalidWipLimit(value) => {
            format!("WIP 上限「{value}」は正の整数、または空欄で入力してください")
        }
        BoardError::EmptyTagName => "タグ名を入力してください".to_string(),
        BoardError::TagNotFound(tag_id) => {
            format!("タグ #{tag_id} が見つかりません。画面を更新してください")
        }
        BoardError::DuplicateTagName(name) => {
            format!("タグ「{name}」はすでに存在します。別の名前を入力してください")
        }
        BoardError::EmptyChecklistItemText => "チェック項目を入力してください".to_string(),
        BoardError::ChecklistItemNotFound(item_id, card_id) => {
            format!("カード #{card_id} のチェック項目 #{item_id} が見つかりません")
        }
        BoardError::LastColumn => "最後のカラムは削除できません".to_string(),
    }
}

fn db_error_detail(error: &DbError) -> String {
    match error {
        DbError::Sqlite(error) => match error {
            rusqlite::Error::SqliteFailure(sqlite_error, message) => {
                let reason = message.as_deref().unwrap_or("詳細情報なし");
                match sqlite_error.code {
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                        "データベースが使用中です。ほかの操作が終わってから再試行してください".to_string()
                    }
                    rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::PermissionDenied => {
                        "データベースに書き込めません。保存先の権限を確認してください".to_string()
                    }
                    rusqlite::ErrorCode::DiskFull => {
                        "ディスク容量が不足しています。空き容量を確保してください".to_string()
                    }
                    rusqlite::ErrorCode::DatabaseCorrupt
                    | rusqlite::ErrorCode::NotADatabase => {
                        "データベースが壊れているか、SQLite データベースではありません。バックアップを確認してください".to_string()
                    }
                    rusqlite::ErrorCode::CannotOpen => {
                        "データベースを開けません。保存先のパスと権限を確認してください".to_string()
                    }
                    _ => format!("SQLite の処理に失敗しました（{reason}）"),
                }
            }
            _ => format!("SQLite の処理に失敗しました（{error}）"),
        },
        DbError::NoBoard => "ボードが見つかりません。画面を更新してください".to_string(),
        DbError::LastBoard => "最後のボードは削除できません".to_string(),
        DbError::EmptyBoardName => "ボード名を入力してください".to_string(),
        DbError::InvalidAppState => {
            "保存されたアプリ状態を読み取れません。ボードを選び直してください".to_string()
        }
        DbError::Json(error) => format!("ボードデータの変換に失敗しました（{error}）"),
    }
}

fn field_error_for(error: &BoardError) -> Option<FieldError> {
    let (field, message, value) = match error {
        BoardError::EmptyCardTitle => (
            EditorField::CardTitle,
            "タイトルを入力してください",
            Some(String::new()),
        ),
        BoardError::InvalidDueDate(value) => (
            EditorField::DueDate,
            "YYYY-MM-DD 形式で入力してください（空欄で期限なし）",
            Some(value.clone()),
        ),
        BoardError::EmptyColumnName => (
            EditorField::ColumnName,
            "カラム名を入力してください",
            Some(String::new()),
        ),
        BoardError::InvalidWipLimit(value) => (
            EditorField::WipLimit,
            "WIP は正の整数、または空欄で入力してください",
            Some(value.clone()),
        ),
        BoardError::EmptyTagName => (
            EditorField::TagName,
            "タグ名を入力してください",
            Some(String::new()),
        ),
        BoardError::DuplicateTagName(name) => (
            EditorField::TagName,
            "タグ名を確認してください",
            Some(name.clone()),
        ),
        BoardError::EmptyChecklistItemText => (
            EditorField::ChecklistItem,
            "チェック項目を入力してください",
            Some(String::new()),
        ),
        BoardError::EmptyBoardName => (
            EditorField::BoardName,
            "ボード名を入力してください",
            Some(String::new()),
        ),
        BoardError::ColumnNotFound(_)
        | BoardError::CardNotFound(_)
        | BoardError::TagNotFound(_)
        | BoardError::ChecklistItemNotFound(_, _)
        | BoardError::LastColumn => return None,
    };
    Some(FieldError {
        field,
        message: message.to_string(),
        value,
    })
}

fn field_error_for_db(error: &DbError) -> Option<FieldError> {
    matches!(error, DbError::EmptyBoardName).then(|| FieldError {
        field: EditorField::BoardName,
        message: "ボード名を入力してください".to_string(),
        value: Some(String::new()),
    })
}

fn field_error_message(
    error: Option<&FieldError>,
    field: EditorField,
    value: &str,
) -> Option<String> {
    error
        .filter(|error| {
            error.field == field
                && error
                    .value
                    .as_deref()
                    .map(|expected| expected == value)
                    .unwrap_or(true)
        })
        .map(|error| error.message.clone())
}

fn field_error_note(message: String, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(color)
        .child(format!("⚠ {message}"))
}

#[cfg(test)]
mod tests {
    use super::{
        board_error_detail, column_name_for_card, db_error_detail, field_error_for, next_card_id,
        render_board_markdown, CardDirection, EditorField,
    };
    use crate::{
        db::DbError,
        model::{Board, BoardError},
    };

    #[test]
    fn arrow_navigation_moves_within_and_between_columns() {
        let board = Board::demo();
        let first_column_card = board.columns[0].cards[0].id;
        let second_column_card = board.columns[0].cards[1].id;
        let third_column_card = board.columns[1].cards[0].id;

        assert_eq!(
            next_card_id(&board.columns, Some(first_column_card), CardDirection::Down),
            Some(second_column_card)
        );
        assert_eq!(
            next_card_id(&board.columns, Some(second_column_card), CardDirection::Up),
            Some(first_column_card)
        );
        assert_eq!(
            next_card_id(
                &board.columns,
                Some(first_column_card),
                CardDirection::Right
            ),
            Some(third_column_card)
        );
        assert_eq!(
            next_card_id(&board.columns, Some(third_column_card), CardDirection::Left),
            Some(first_column_card)
        );
    }

    #[test]
    fn arrow_navigation_skips_empty_columns() {
        let mut board = Board::demo();
        let empty_column_id = board.add_column("空").expect("column can be added");
        board
            .move_column(empty_column_id, 1)
            .expect("column can be moved");
        let first_card = board.columns[0].cards[0].id;
        let next_card = board.columns[2].cards[0].id;

        assert_eq!(
            next_card_id(&board.columns, Some(first_card), CardDirection::Right),
            Some(next_card)
        );
    }

    #[test]
    fn arrow_navigation_starts_at_first_card_without_selection() {
        let board = Board::demo();
        let first_card = board.columns[0].cards[0].id;

        assert_eq!(
            next_card_id(&board.columns, None, CardDirection::Down),
            Some(first_card)
        );
    }

    #[test]
    fn shows_which_column_a_card_belongs_to() {
        let board = Board::demo();
        let first_card = board.columns[0].cards[0].id;
        let second_column_card = board.columns[1].cards[0].id;

        assert_eq!(
            column_name_for_card(&board.columns, first_card),
            Some(board.columns[0].name.as_str())
        );
        assert_eq!(
            column_name_for_card(&board.columns, second_column_card),
            Some(board.columns[1].name.as_str())
        );
        assert_eq!(column_name_for_card(&board.columns, 9_999), None);
    }

    #[test]
    fn maps_validation_errors_to_the_field_that_needs_attention() {
        let error = field_error_for(&BoardError::InvalidDueDate("2026/09/04".to_string()))
            .expect("due date has a field error");

        assert_eq!(error.field, EditorField::DueDate);
        assert!(error.message.contains("YYYY-MM-DD"));
    }

    #[test]
    fn explains_database_open_errors_in_user_facing_terms() {
        let error = DbError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::CannotOpen,
                extended_code: 14,
            },
            Some("unable to open database file".to_string()),
        ));

        let message = db_error_detail(&error);
        assert!(message.contains("開けません"));
        assert!(!message.contains("unable to open database file"));
    }

    #[test]
    fn keeps_domain_error_details_specific() {
        let message = board_error_detail(&BoardError::DuplicateTagName("bug".to_string()));

        assert_eq!(
            message,
            "タグ「bug」はすでに存在します。別の名前を入力してください"
        );
    }

    #[test]
    fn renders_board_markdown_with_descriptions_and_checklists() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;
        board
            .update_card_details_with_checklist(
                card_id,
                "Markdownカード",
                "説明の一行目\n説明の二行目",
                None,
                Vec::new(),
                vec![crate::model::ChecklistItemDraft {
                    id: None,
                    text: "確認済み".to_string(),
                    checked: true,
                }],
            )
            .unwrap();

        let markdown = render_board_markdown(&board);
        assert!(markdown.contains("# 個人 Kanban"));
        assert!(markdown.contains("- **Markdownカード**"));
        assert!(markdown.contains("> 説明の一行目"));
        assert!(markdown.contains("- [x] 確認済み"));
    }
}
