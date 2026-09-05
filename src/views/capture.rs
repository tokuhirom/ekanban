//! クイックキャプチャの入力ウィンドウ。
//!
//! ホットキーで開く 1 行入力だけの小さいウィンドウ。ボードのウィンドウとは別
//! ウィンドウだが、保存は `BoardView` に依頼する。ここから直接 SQLite を触ると
//! ボード側の非同期保存と競合するため。

use gpui_kit::{
    component::input::{Input, InputState},
    component::Sizable,
    div,
    prelude::*,
    px, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Render, SharedString, WeakEntity,
    Window,
};

use super::board::{theme_color, BoardView, UiColor};

pub(crate) struct CaptureView {
    board_view: WeakEntity<BoardView>,
    title: Entity<InputState>,
    /// 「〇〇ボード / △△カラム」。どこに入るのかを常に見せる。
    destination: SharedString,
    error: Option<String>,
    /// 保存を依頼して結果を待っている間は `true`。`Enter` の二重押しを受けない。
    saving: bool,
    focus_handle: FocusHandle,
}

impl CaptureView {
    pub(crate) fn new(
        board_view: WeakEntity<BoardView>,
        destination: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("思いついたことを 1 行で"));
        title.update(cx, |state, cx| state.focus(window, cx));
        Self {
            board_view,
            title,
            destination,
            error: None,
            saving: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// 保存を依頼する。結果は `BoardView` から返ってくる。
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        let title = self.title.read(cx).value().trim().to_string();
        if title.is_empty() {
            return;
        }
        let Some(board_view) = self.board_view.upgrade() else {
            self.error = Some("ボードのウィンドウが閉じられています".to_string());
            cx.notify();
            return;
        };

        match board_view.update(cx, |view, cx| view.capture_card(&title, cx)) {
            Ok(()) => {
                self.saving = true;
                self.error = None;
            }
            Err(message) => self.error = Some(message),
        }
        cx.notify();
    }

    /// 保存に失敗したとき。ウィンドウは閉じず、入力を残す。
    pub(crate) fn show_save_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.saving = false;
        self.error = Some(message);
        cx.notify();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.remove_window();
        if let Some(board_view) = self.board_view.upgrade() {
            board_view.update(cx, |view, cx| view.on_capture_window_closed(cx));
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "enter" => {
                cx.stop_propagation();
                self.save(cx);
            }
            "escape" => {
                cx.stop_propagation();
                self.cancel(window, cx);
            }
            _ => {}
        }
    }
}

impl Focusable for CaptureView {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CaptureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("QuickCapture")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(theme_color(cx, UiColor::Popover))
            .text_color(theme_color(cx, UiColor::Foreground))
            .child(
                div()
                    .text_xs()
                    .text_color(theme_color(cx, UiColor::MutedForeground))
                    .child(self.destination.clone()),
            )
            .child(
                Input::new(&self.title)
                    .small()
                    .bg(theme_color(cx, UiColor::InputBackground))
                    .text_color(theme_color(cx, UiColor::Foreground)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if self.error.is_some() {
                        theme_color(cx, UiColor::Danger)
                    } else {
                        theme_color(cx, UiColor::MutedForeground)
                    })
                    .child(match (self.error.as_ref(), self.saving) {
                        (Some(message), _) => format!("⚠ {message}"),
                        (None, true) => "保存中…".to_string(),
                        (None, false) => "Enter で追加、Escape で閉じる".to_string(),
                    }),
            )
            .h(px(120.))
    }
}
