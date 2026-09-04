use std::{
    collections::VecDeque,
    path::PathBuf,
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
    component::{button::Button, button::ButtonVariants as _},
    div,
    prelude::*,
    px, rgb, rgba, Context, Entity, Focusable as _, Half, IntoElement, KeyDownEvent, Pixels, Point,
    Render, SharedString, Window,
};

use crate::{
    actions::{
        About, AddBoard, AddCard, AddColumn, AddTag, CancelEdit, ClearSearch, CloseWindow,
        DeleteBoard, FocusSearch, Redo, RenameBoard, SaveEdit, ShowAllCards, ShowOverdueCards,
        ShowThisWeekCards, ToggleArchiveView, ToggleFullscreen, Undo,
    },
    db::{save_board_snapshot, Database, DbError},
    model::{
        card_matches_search, due_status, parse_due_date, parse_wip_limit, Board, BoardError,
        BoardId, BoardSummary, Card, CardId, Column, ColumnId, DueStatus, Tag, TagId,
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
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
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
                    .bg(rgba(0x334155f0))
                    .border_1()
                    .border_color(rgb(0x93c5fd))
                    .rounded_lg()
                    .shadow_lg()
                    .text_color(rgb(0xf8fafc))
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
}

struct ColumnEditor {
    column_id: Option<ColumnId>,
    name: Entity<InputState>,
    wip_limit: Entity<InputState>,
}

struct TagEditor {
    tag_id: Option<TagId>,
    name: Entity<InputState>,
    color: Entity<InputState>,
}

struct BoardEditor {
    board_id: Option<BoardId>,
    name: Entity<InputState>,
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

impl Render for ColumnDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
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
                    .bg(rgba(0x1d4ed8f0))
                    .border_1()
                    .border_color(rgb(0x93c5fd))
                    .rounded_lg()
                    .shadow_lg()
                    .text_color(rgb(0xf8fafc))
                    .child(self.name.clone()),
            )
    }
}

pub struct BoardView {
    board: Board,
    boards: Vec<BoardSummary>,
    database_path: PathBuf,
    save_lock: Arc<Mutex<()>>,
    next_save_id: u64,
    pending_saves: VecDeque<PendingSave>,
    active_save: Option<ActiveSave>,
    status: Option<String>,
    editing_card: Option<CardEditor>,
    editing_column: Option<ColumnEditor>,
    editing_tag: Option<TagEditor>,
    editing_board: Option<BoardEditor>,
    due_filter: DueFilter,
    tag_filter: Option<TagId>,
    show_archived: bool,
    search: Entity<InputState>,
    search_query: String,
}

impl BoardView {
    pub fn new(
        board: Board,
        boards: Vec<BoardSummary>,
        database_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("タイトル・説明を検索"));
        Self {
            board,
            boards,
            database_path,
            save_lock: Arc::new(Mutex::new(())),
            next_save_id: 0,
            pending_saves: VecDeque::new(),
            active_save: None,
            status: None,
            editing_card: None,
            editing_column: None,
            editing_tag: None,
            editing_board: None,
            due_filter: DueFilter::None,
            tag_filter: None,
            show_archived: false,
            search,
            search_query: String::new(),
        }
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
        self.status = Some("保存が完了するまでボードを変更できません".to_string());
        cx.notify();
        true
    }

    fn reset_board_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing_card = None;
        self.editing_column = None;
        self.editing_tag = None;
        self.editing_board = None;
        self.due_filter = DueFilter::None;
        self.tag_filter = None;
        self.show_archived = false;
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.search_query.clear();
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
            self.status = Some("編集中はボードを切り替えられません".to_string());
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
                    Ok(()) => self.status = Some(format!("「{name}」に切り替えました")),
                    Err(error) => {
                        self.status = Some(format!("ボードを切り替えました（記憶に失敗: {error}）"))
                    }
                }
            }
            Err(error) => self.status = Some(format_db_error(error)),
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
        });
        cx.notify();
    }

    fn cancel_board_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_board.take().is_some() {
            self.status = Some("ボード名の編集をキャンセルしました".to_string());
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
                        self.status = Some("ボードを追加しました".to_string());
                    }
                    Err(error) => {
                        self.editing_board = Some(editor);
                        self.status = Some(format_db_error(error));
                    }
                }
            }
            Some(board_id) => {
                let before = self.board.clone();
                match self.board.rename(name) {
                    Ok(false) => self.status = Some("ボード名に変更はありません".to_string()),
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
                        self.editing_board = Some(editor);
                        self.status = Some(format_board_error(error));
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
            self.status = Some("ボードが見つかりません".to_string());
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
                                self.status = Some(format!(
                                    "ボードを削除して切り替えました（記憶に失敗: {error}）"
                                ));
                            } else {
                                self.status = Some("ボードを削除しました".to_string());
                            }
                        }
                        Err(error) => self.status = Some(format_db_error(error)),
                    }
                } else {
                    self.status = Some("ボードを削除しました".to_string());
                }
            }
            Err(error) => self.status = Some(format_db_error(error)),
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
        self.status = Some("保存中…".to_string());
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
                    self.status = Some(active.success_message);
                } else {
                    self.status = Some("保存中…".to_string());
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
                self.status = Some(format!("保存に失敗しました: {error}"));
            }
        }
        cx.notify();
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
            Ok(true) => self.enqueue_save(before, "保存しました", SaveFailure::None, cx),
            Err(error) => self.status = Some(format_move_error(error)),
        }
        cx.notify();
    }

    fn move_column(&mut self, column_id: ColumnId, target_index: usize, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.move_column(column_id, target_index) {
            Ok(false) => return,
            Ok(true) => self.enqueue_save(before, "カラムを並べ替えました", SaveFailure::None, cx),
            Err(error) => self.status = Some(format_move_error(error)),
        }
        cx.notify();
    }

    fn add_card(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_archived {
            self.status = Some("アーカイブ表示中はカードを追加できません".to_string());
            cx.notify();
            return;
        }
        let Some(column_id) = self.board.columns.first().map(|column| column.id) else {
            return;
        };
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
            Err(error) => self.status = Some(format_move_error(error)),
        }
        cx.notify();
    }

    fn begin_card_edit(&mut self, card_id: CardId, window: &mut Window, cx: &mut Context<Self>) {
        let Some((title, description, due_date, tag_ids)) = self
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
                )
            })
        else {
            self.status = Some(format_card_error(BoardError::CardNotFound(card_id)));
            cx.notify();
            return;
        };

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
        title_input.update(cx, |state, cx| state.focus(window, cx));
        self.editing_card = Some(CardEditor {
            card_id,
            title: title_input,
            description: description_input,
            due_date: due_date_input,
            tag_ids,
        });
        cx.notify();
    }

    fn cancel_card_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_card.take().is_some() {
            self.status = Some("カードの編集をキャンセルしました".to_string());
            cx.notify();
        }
    }

    fn save_card_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editing_card.take() else {
            return;
        };
        let title = editor.title.read(cx).value().to_string();
        let description = editor.description.read(cx).value().to_string();
        let due_date_text = editor.due_date.read(cx).value().to_string();
        let tag_ids = editor.tag_ids.clone();
        let due_date = match parse_due_date(&due_date_text) {
            Ok(due_date) => due_date,
            Err(error) => {
                self.editing_card = Some(editor);
                self.status = Some(format!("期限を確認してください: {error}"));
                cx.notify();
                return;
            }
        };
        let before = self.board.clone();

        let changed = match self.board.update_card_details(
            editor.card_id,
            title,
            description,
            due_date,
            tag_ids,
        ) {
            Ok(changed) => changed,
            Err(error) => {
                self.editing_card = Some(editor);
                self.status = Some(format_card_error(error));
                cx.notify();
                return;
            }
        };

        if !changed {
            self.status = Some("カードに変更はありません".to_string());
        } else {
            self.enqueue_save(
                before,
                "カードを更新しました",
                SaveFailure::RestoreCardEditor(editor),
                cx,
            );
        }
        cx.notify();
    }

    fn delete_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.delete_card(card_id) {
            Ok(()) => {
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
            Err(error) => self.status = Some(format_card_error(error)),
        }
        cx.notify();
    }

    fn archive_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.archive_card(card_id) {
            Ok(true) => {
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
            Err(error) => self.status = Some(format_card_error(error)),
        }
        cx.notify();
    }

    fn archive_column(&mut self, column_id: ColumnId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.archive_column(column_id) {
            Ok(0) => self.status = Some("アーカイブするカードがありません".to_string()),
            Ok(count) => {
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
            Err(error) => self.status = Some(format_column_error(error)),
        }
        cx.notify();
    }

    fn restore_card(&mut self, card_id: CardId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.restore_card(card_id) {
            Ok(true) => self.enqueue_save(before, "カードを復元しました", SaveFailure::None, cx),
            Ok(false) => {}
            Err(error) => self.status = Some(format_card_error(error)),
        }
        cx.notify();
    }

    fn toggle_archive_view(&mut self, cx: &mut Context<Self>) {
        self.show_archived = !self.show_archived;
        self.editing_card = None;
        self.editing_column = None;
        self.status = Some(if self.show_archived {
            "アーカイブを表示しています".to_string()
        } else {
            "ボードを表示しています".to_string()
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
            self.status = Some(format_tag_error(BoardError::TagNotFound(tag_id)));
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
        });
        cx.notify();
    }

    fn cancel_tag_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_tag.take().is_some() {
            self.status = Some("タグの編集をキャンセルしました".to_string());
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
            Ok(false) => self.status = Some("タグに変更はありません".to_string()),
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
                self.status = Some(format_tag_error(error));
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
            Err(error) => self.status = Some(format_tag_error(error)),
        }
        cx.notify();
    }

    fn set_tag_filter(&mut self, tag_id: TagId, cx: &mut Context<Self>) {
        self.tag_filter = if self.tag_filter == Some(tag_id) {
            None
        } else {
            Some(tag_id)
        };
        self.status = Some("タグフィルターを変更しました".to_string());
        cx.notify();
    }

    fn begin_add_column(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_archived {
            self.status = Some("アーカイブ表示中はカラムを追加できません".to_string());
            cx.notify();
            return;
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("カラム名"));
        name.update(cx, |state, cx| state.focus(window, cx));
        self.editing_column = Some(ColumnEditor {
            column_id: None,
            name,
            wip_limit: cx.new(|cx| InputState::new(window, cx).placeholder("WIP 上限")),
        });
        cx.notify();
    }

    fn begin_column_edit(
        &mut self,
        column_id: ColumnId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            self.status = Some(format_column_error(BoardError::ColumnNotFound(column_id)));
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
        });
        cx.notify();
    }

    fn cancel_column_edit(&mut self, cx: &mut Context<Self>) {
        if self.editing_column.take().is_some() {
            self.status = Some("カラムの編集をキャンセルしました".to_string());
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
                self.status = Some(format!("WIP 上限を確認してください: {error}"));
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
                self.status = Some("カラムに変更はありません".to_string());
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
                self.status = Some(format_column_error(error));
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
        let Some(column) = self
            .board
            .columns
            .iter()
            .find(|column| column.id == column_id)
        else {
            self.status = Some(format_column_error(BoardError::ColumnNotFound(column_id)));
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
        let before = self.board.clone();
        match self.board.remove_column(column_id) {
            Ok(()) => {
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
            Err(error) => self.status = Some(format_column_error(error)),
        }
        cx.notify();
    }

    fn sort_column_by_due_date(&mut self, column_id: ColumnId, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.sort_column_by_due_date(column_id) {
            Ok(false) => {
                self.status = Some("期限順に変更はありません".to_string());
            }
            Ok(true) => self.enqueue_save(before, "期限順に並べ替えました", SaveFailure::None, cx),
            Err(error) => self.status = Some(format_column_error(error)),
        }
        cx.notify();
    }

    fn set_due_filter(&mut self, filter: DueFilter, cx: &mut Context<Self>) {
        self.due_filter = if self.due_filter == filter {
            DueFilter::None
        } else {
            filter
        };
        self.status = Some("表示フィルターを変更しました".to_string());
        cx.notify();
    }

    fn commit_search(&mut self, cx: &mut Context<Self>) {
        self.search_query = self.search.read(cx).value().to_string();
        self.status = if self.search_query.trim().is_empty() {
            Some("検索をクリアしました".to_string())
        } else {
            Some(format!("「{}」で検索中", self.search_query))
        };
        cx.notify();
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search
            .update(cx, |state, cx| state.set_value("", window, cx));
        self.search_query.clear();
        self.status = Some("検索をクリアしました".to_string());
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
            self.status = Some("編集中は元に戻せません".to_string());
            cx.notify();
            return;
        }

        let before = self.board.clone();
        match self.board.undo() {
            Ok(false) => self.status = Some("元に戻す操作がありません".to_string()),
            Ok(true) => self.enqueue_save(before, "元に戻しました", SaveFailure::None, cx),
            Err(error) => self.status = Some(format!("元に戻せませんでした: {error}")),
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
            self.status = Some("編集中はやり直せません".to_string());
            cx.notify();
            return;
        }

        let before = self.board.clone();
        match self.board.redo() {
            Ok(false) => self.status = Some("やり直す操作がありません".to_string()),
            Ok(true) => self.enqueue_save(before, "やり直しました", SaveFailure::None, cx),
            Err(error) => self.status = Some(format!("やり直せませんでした: {error}")),
        }
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
            .child(Input::new(&editor.name).small())
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("save-board-edit")
                            .primary()
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
            .border_color(rgb(0x253047))
            .bg(rgb(0x111827))
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
                                rgb(0x1d4ed8)
                            } else {
                                rgb(0x1e293b)
                            })
                            .hover(|this| {
                                this.bg(if selected {
                                    rgb(0x2563eb)
                                } else {
                                    rgb(0x334155)
                                })
                            })
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.switch_board(board_id, window, cx)
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0xf8fafc))
                                    .child(summary.name.clone()),
                            )
                    }))
                    .when(self.boards.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x94a3b8))
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
                            .danger()
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
            .child(Input::new(&self.search).small())
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
        let wip_limit_invalid = parse_wip_limit(&editor.wip_limit.read(cx).value()).is_err();
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
            .child(Input::new(&editor.name).small())
            .child(Input::new(&editor.wip_limit).small())
            .when(wip_limit_invalid, |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xf87171))
                        .child("WIP は正の整数"),
                )
            })
            .child(
                Button::new(("save-column", editor_kind.unwrap_or(0) as u64))
                    .primary()
                    .disabled(wip_limit_invalid)
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
            .child(Input::new(&editor.name).small())
            .child(Input::new(&editor.color).small())
            .child(
                Button::new(("save-tag", editor_kind.unwrap_or(0) as u64))
                    .primary()
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
            .child(div().text_xs().text_color(rgb(0x94a3b8)).child("タグ"))
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
                            .danger()
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
        let status = self
            .status
            .clone()
            .unwrap_or_else(|| "ローカル SQLite".to_string());
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .p_4()
            .border_b_1()
            .border_color(rgb(0x253047))
            .bg(rgb(0x111827))
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
                    .child(div().text_xs().text_color(rgb(0x94a3b8)).child(status))
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
                            .label("すべて")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_due_filter(DueFilter::None, cx)
                            })),
                    )
                    .child(
                        Button::new("filter-overdue")
                            .secondary()
                            .label("期限切れ")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_due_filter(DueFilter::Overdue, cx)
                            })),
                    )
                    .child(
                        Button::new("filter-week")
                            .secondary()
                            .label("今週まで")
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
                        .text_sm()
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(rgb(0xe2e8f0))
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
            .bg(rgb(0x1e293b))
            .border_1()
            .border_color(rgb(0x334155))
            .on_drop(cx.listener(move |this, drag: &CardDrag, _, cx| {
                this.move_card(drag.card_id, column_id, end_index, cx);
            }))
            .on_drop(cx.listener(move |this, drag: &ColumnDrag, _, cx| {
                this.move_column(drag.column_id, column_index, cx);
            }))
            .drag_over::<CardDrag>(|style, _, _, _| style.border_color(rgb(0x60a5fa)))
            .drag_over::<ColumnDrag>(|style, _, _, _| style.border_color(rgb(0x818cf8)))
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
                                        rgb(0xf87171)
                                    } else {
                                        rgb(0x94a3b8)
                                    })
                                    .child(card_count_label),
                            )
                            .when(!is_editing, |this| {
                                this.child(
                                    Button::new(("sort-column", column_id as u64))
                                        .ghost()
                                        .label("期限順")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.sort_column_by_due_date(column_id, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("archive-column", column_id as u64))
                                        .ghost()
                                        .label("アーカイブ")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.archive_column(column_id, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("edit-column", column_id as u64))
                                        .ghost()
                                        .label("編集")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.begin_column_edit(column_id, window, cx)
                                        })),
                                )
                                .child(
                                    Button::new(("delete-column", column_id as u64))
                                        .danger()
                                        .disabled(last_column)
                                        .label("削除")
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.request_delete_column(column_id, window, cx)
                                        })),
                                )
                            }),
                    ),
            )
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
                    .border_color(rgb(0x475569))
                    .text_xs()
                    .text_color(rgb(0x64748b))
                    .child("ここにドロップ"),
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
                        .text_color(rgb(0x94a3b8))
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
            .bg(rgb(0x334155))
            .border_1()
            .border_color(rgb(0x475569))
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
                            .text_color(rgb(0xf8fafc))
                            .child(card.title.clone()),
                    )
                    .when_some(
                        card.due_date
                            .map(|due_date| render_due_badge(due_date, today).into_any_element()),
                        |this, badge| this.child(badge),
                    )
                    .children(
                        card.tag_ids
                            .iter()
                            .filter_map(|tag_id| {
                                self.board.tags.iter().find(|tag| tag.id == *tag_id)
                            })
                            .map(render_tag_chip),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
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
            .border_color(rgb(0x475569))
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
            .map(|due_date| render_due_badge(due_date, today).into_any_element());
        let is_editing = self
            .editing_card
            .as_ref()
            .is_some_and(|editor| editor.card_id == card_id);
        let editor = self.editing_card.as_ref();
        div()
            .id(("card", card_id as u64))
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded_md()
            .bg(rgb(0x334155))
            .border_1()
            .border_color(rgb(0x475569))
            .hover(|this| this.bg(rgb(0x3f4f66)))
            .when(dimmed, |this| this.opacity(0.35))
            .on_drop(cx.listener(move |this, drag: &CardDrag, _, cx| {
                this.move_card(drag.card_id, column_id, index, cx);
            }))
            .drag_over::<CardDrag>(|style, _, _, _| style.border_color(rgb(0x60a5fa)))
            .child(if is_editing {
                self.render_card_editor(editor.expect("editing card exists"), cx)
                    .into_any_element()
            } else {
                div()
                    .id(("card-handle", card_id as u64))
                    .cursor_move()
                    .on_drag(CardDrag { card_id }, move |_, position, _, cx| {
                        cx.new(|_| CardDragPreview {
                            title: drag_title.clone(),
                            position,
                        })
                    })
                    .child(div().text_sm().text_color(rgb(0xf8fafc)).child(title))
                    .when_some(due_badge, |this, badge| this.child(badge))
                    .children(
                        card.tag_ids
                            .iter()
                            .filter_map(|tag_id| {
                                self.board.tags.iter().find(|tag| tag.id == *tag_id)
                            })
                            .map(render_tag_chip),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child(card.description.clone()),
                    )
                    .into_any_element()
            })
            .when(!is_editing, |this| {
                this.child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_1()
                        .child(
                            Button::new(("edit-card", card_id as u64))
                                .ghost()
                                .label("編集")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.begin_card_edit(card_id, window, cx)
                                })),
                        )
                        .child(
                            Button::new(("archive-card", card_id as u64))
                                .ghost()
                                .label("アーカイブ")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.archive_card(card_id, cx)
                                })),
                        )
                        .child(
                            Button::new(("delete-card", card_id as u64))
                                .danger()
                                .label("削除")
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.delete_card(card_id, cx)
                                    }),
                                ),
                        ),
                )
            })
    }

    fn render_card_editor(&self, editor: &CardEditor, cx: &mut Context<Self>) -> impl IntoElement {
        let due_date_value = editor.due_date.read(cx).value().to_string();
        let due_date_invalid = parse_due_date(&due_date_value).is_err();
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
            .child(div().text_xs().text_color(rgb(0x94a3b8)).child("タイトル"))
            .child(Input::new(&editor.title).small())
            .child(div().text_xs().text_color(rgb(0x94a3b8)).child("説明"))
            .child(Textarea::new(&editor.description).h(px(96.)))
            .child(div().text_xs().text_color(rgb(0x94a3b8)).child("期限"))
            .child(Input::new(&editor.due_date).small())
            .when(due_date_invalid, |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(rgb(0xf87171))
                        .child("YYYY-MM-DD 形式で入力してください"),
                )
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
            .child(div().text_xs().text_color(rgb(0x94a3b8)).child("タグ"))
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
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-card-edit")
                            .secondary()
                            .label("キャンセル")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_card_edit(cx))),
                    )
                    .child(
                        Button::new("save-card-edit")
                            .primary()
                            .disabled(due_date_invalid)
                            .label("保存")
                            .on_click(cx.listener(|this, _, _, cx| this.save_card_edit(cx))),
                    ),
            )
    }
}

impl Render for BoardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let column_count = self.board.columns.len();
        div()
            .key_context("Board")
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
            .bg(rgb(0x0f172a))
            .text_color(rgb(0xf8fafc))
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
    }
}

fn render_due_badge(due_date: NaiveDate, today: NaiveDate) -> impl IntoElement {
    let status = due_status(Some(due_date), today);
    let (label, color) = match status {
        DueStatus::Overdue(days) => (
            format!("期限切れ {days}日 ({})", short_date(due_date)),
            0xf87171,
        ),
        DueStatus::Today => (format!("期限 今日 ({})", short_date(due_date)), 0xfbbf24),
        DueStatus::Soon(days) => (
            format!("期限 あと {days}日 ({})", short_date(due_date)),
            0x60a5fa,
        ),
        DueStatus::Upcoming(_) => (format!("期限 {}", display_date(due_date, today)), 0x94a3b8),
        DueStatus::None => return div(),
    };
    div().text_xs().text_color(rgb(color)).child(label)
}

fn render_tag_chip(tag: &Tag) -> impl IntoElement {
    div()
        .px_1()
        .rounded_sm()
        .bg(rgb(tag_color_value(&tag.color)))
        .text_xs()
        .text_color(rgb(0xf8fafc))
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

fn format_move_error(error: BoardError) -> String {
    format!("移動できませんでした: {error}")
}

fn format_column_error(error: BoardError) -> String {
    format!("カラムを操作できませんでした: {error}")
}

fn format_card_error(error: BoardError) -> String {
    format!("カードを操作できませんでした: {error}")
}

fn format_tag_error(error: BoardError) -> String {
    format!("タグを操作できませんでした: {error}")
}

fn format_board_error(error: BoardError) -> String {
    format!("ボードを操作できませんでした: {error}")
}

fn format_db_error(error: DbError) -> String {
    format!("ボードを操作できませんでした: {error}")
}
