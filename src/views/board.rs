use gpui_kit::{
    component::{button::Button, button::ButtonVariants as _},
    div,
    prelude::*,
    px, rgb, rgba, Context, Half, IntoElement, Pixels, Point, Render, SharedString, Window,
};

use crate::{
    db::Database,
    model::{Board, BoardError, Card, CardId, Column, ColumnId},
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
    database: Database,
    status: Option<String>,
}

impl BoardView {
    pub fn new(board: Board, database: Database) -> Self {
        Self {
            board,
            database,
            status: None,
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
            Ok(true) => match self.database.save_board(&self.board) {
                Ok(()) => self.status = Some("保存しました".to_string()),
                Err(error) => {
                    self.board = before;
                    self.status = Some(format!("保存に失敗しました: {error}"));
                }
            },
            Err(error) => self.status = Some(format_move_error(error)),
        }
        cx.notify();
    }

    fn move_column(&mut self, column_id: ColumnId, target_index: usize, cx: &mut Context<Self>) {
        let before = self.board.clone();
        match self.board.move_column(column_id, target_index) {
            Ok(false) => return,
            Ok(true) => match self.database.save_board(&self.board) {
                Ok(()) => self.status = Some("カラムを並べ替えました".to_string()),
                Err(error) => {
                    self.board = before;
                    self.status = Some(format!("保存に失敗しました: {error}"));
                }
            },
            Err(error) => self.status = Some(format_move_error(error)),
        }
        cx.notify();
    }

    fn add_card(&mut self, cx: &mut Context<Self>) {
        let Some(column_id) = self.board.columns.first().map(|column| column.id) else {
            return;
        };
        let before = self.board.clone();
        let result = self
            .board
            .add_card(column_id, "新しいカード", "説明を追加してください");
        match result {
            Ok(_) => match self.database.save_board(&self.board) {
                Ok(()) => self.status = Some("カードを追加しました".to_string()),
                Err(error) => {
                    self.board = before;
                    self.status = Some(format!("保存に失敗しました: {error}"));
                }
            },
            Err(error) => self.status = Some(format_move_error(error)),
        }
        cx.notify();
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
                    .child(div().text_xs().text_color(rgb(0x94a3b8)).child(status)),
            )
            .child(
                Button::new("add-card")
                    .primary()
                    .label("＋ カードを追加")
                    .on_click(cx.listener(|this, _, _, cx| this.add_card(cx))),
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
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x94a3b8))
                            .child(format!("{} cards", column.cards.len())),
                    ),
            )
            .children(
                column
                    .cards
                    .iter()
                    .enumerate()
                    .map(|(index, card)| self.render_card(column_id, index, card, cx)),
            )
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

    fn render_card(
        &self,
        column_id: ColumnId,
        index: usize,
        card: &Card,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let card_id = card.id;
        let title = card.title.clone();
        let drag_title = SharedString::from(card.title.clone());
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
            .cursor_move()
            .on_drag(CardDrag { card_id }, move |_, position, _, cx| {
                cx.new(|_| CardDragPreview {
                    title: drag_title.clone(),
                    position,
                })
            })
            .on_drop(cx.listener(move |this, drag: &CardDrag, _, cx| {
                this.move_card(drag.card_id, column_id, index, cx);
            }))
            .drag_over::<CardDrag>(|style, _, _, _| style.border_color(rgb(0x60a5fa)))
            .child(div().text_sm().text_color(rgb(0xf8fafc)).child(title))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x94a3b8))
                    .child(card.description.clone()),
            )
    }
}

impl Render for BoardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let column_count = self.board.columns.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0f172a))
            .text_color(rgb(0xf8fafc))
            .child(self.render_header(cx))
            .child(
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
                    ),
            )
    }
}

fn format_move_error(error: BoardError) -> String {
    format!("移動できませんでした: {error}")
}
