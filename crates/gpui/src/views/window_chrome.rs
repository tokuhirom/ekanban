//! ウィンドウマネージャが枠を描かないときに、自分で描く枠。
//!
//! GNOME (Mutter) のように xdg-decoration で ServerSide を返さない環境では、
//! 枠もタイトルバーも閉じるボタンも一切出ない。移動もリサイズも、ウィンドウ
//! マネージャ側のキーボード操作に頼るしかなくなる。
//!
//! 縁とリサイズは `gpui_kit::component::window_border` が持っている（影・境界線・
//! 8 方向の当たり判定、タイル配置のときに存在しない辺を掴まない扱いまで）ので、
//! ここが足すのは移動のための掴み代と、しまう / 拡大 / 閉じるだけ。
//!
//! 判定は「Wayland かどうか」ではなく `Decorations` で行う。X11 でもコンポジタが
//! ServerSide を返さない構成はありうるし、Wayland でも Mutter 以外は返す。

use gpui_kit::{
    component::{button::Button, button::ButtonVariants as _},
    div,
    prelude::*,
    px, Decorations, IntoElement, MouseButton, SharedString, Window,
};

use super::board::{theme_color, UiColor};

/// タイトルバーの高さ。GNOME の既定より少し低くして、ボードの見える範囲を削らない。
const TITLE_BAR_HEIGHT: f32 = 32.;

/// ウィンドウマネージャが枠を描いてくれないか。
///
/// `true` なら、移動・リサイズ・クローズをアプリ側で出さないと、画面内の操作
/// だけではウィンドウを動かせない。
pub(crate) fn draws_own_chrome(decorations: Decorations) -> bool {
    matches!(decorations, Decorations::Client { .. })
}

/// 掴んで動かすための行と、ウィンドウ操作のボタン。
///
/// ヘッダとは別の行にする。既存のヘッダに掴み判定を足すと、`≡`・検索欄・
/// 「＋ カードを追加」を押しただけでウィンドウが動く。
pub(crate) fn title_bar(title: impl Into<SharedString>, cx: &gpui_kit::App) -> impl IntoElement {
    div()
        .id("window-title-bar")
        .w_full()
        .h(px(TITLE_BAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .px_2()
        .border_b_1()
        .border_color(theme_color(cx, UiColor::Border))
        .bg(theme_color(cx, UiColor::Sidebar))
        // 掴み代。ボタンの側は自分で mouse down を止めるので、ここには落ちてこない。
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            window.start_window_move();
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .px_2()
                .text_xs()
                .text_color(theme_color(cx, UiColor::MutedForeground))
                .child(title.into()),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_1()
                // ボタンを押しただけでウィンドウが動かないよう、ここで止める。
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    Button::new("window-minimize")
                        .ghost()
                        .label("▁")
                        .on_click(|_, window: &mut Window, _| window.minimize_window()),
                )
                .child(
                    Button::new("window-zoom")
                        .ghost()
                        .label("□")
                        .on_click(|_, window: &mut Window, _| window.zoom_window()),
                )
                .child(
                    Button::new("window-close")
                        .ghost()
                        .label("✕")
                        .on_click(|_, window: &mut Window, _| window.remove_window()),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    use gpui_kit::Tiling;

    #[test]
    fn leaves_the_frame_to_the_window_manager_when_it_draws_one() {
        assert!(
            !draws_own_chrome(Decorations::Server),
            "X11 and macOS get their frame from the system, so nothing is drawn here"
        );
    }

    #[test]
    fn draws_the_frame_when_the_compositor_does_not() {
        assert!(
            draws_own_chrome(Decorations::Client {
                tiling: Tiling::default()
            }),
            "a compositor that hands decorations back to the client needs our own frame"
        );
    }
}
