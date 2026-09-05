//! OS がメニューバーを描かない環境で、アプリが自分で描くメニューバー。
//!
//! `cx.set_menus` から本物のメニューバーを作るのは macOS だけで、Linux と Windows
//! ではしまうだけになる。そこで同じ定義を読んで、画面の上端に自分で描く。
//!
//! 中身は `gpui_kit::component::menu::AppMenuBar` が持っている。クリックで開く、
//! 開いている間はほかのメニュー名にホバーすると切り替わる、`Esc` で閉じて元の
//! フォーカスへ戻る、`←` `→` でメニューを、`↑` `↓` で項目を移動して `Enter` で
//! 実行する、項目の右にショートカットを出す──VS Code や Zed と同じ作法が
//! ひととおり入っている。ここが足すのは、ボードの見た目に合わせた枠だけ。
//!
//! 読み元は `GlobalState::app_menus` で、そこへ渡すのは
//! [`crate::menu::install`] の仕事。

use gpui_kit::{component::menu::AppMenuBar, div, prelude::*, px, App, Div, Entity, Stateful};

use super::board::{theme_color, UiColor};

/// メニューバーの行の高さ。ボードの見える範囲を削らないよう、タイトルバーより低くする。
const MENU_BAR_HEIGHT: f32 = 30.;

/// メニューバーを持つなら作る。
///
/// macOS では OS が描くので `None`。二重に出しても、たどれる操作は増えない。
pub(crate) fn build(cx: &mut App) -> Option<Entity<AppMenuBar>> {
    crate::menu::draws_its_own_menu_bar().then(|| AppMenuBar::new(cx))
}

/// メニューバーを 1 行として描く。
pub(crate) fn menu_bar(bar: &Entity<AppMenuBar>, cx: &App) -> Stateful<Div> {
    div()
        .id("app-menu-bar-row")
        .w_full()
        .h(px(MENU_BAR_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .px_1()
        .border_b_1()
        .border_color(theme_color(cx, UiColor::Border))
        .bg(theme_color(cx, UiColor::Sidebar))
        .child(bar.clone())
}
