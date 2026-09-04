pub mod actions;
pub mod db;
pub mod diagnostics;
pub mod menu;
pub mod model;
pub mod paths;
pub mod views;

use std::path::PathBuf;

use db::Database;
use gpui_kit::component::Root;
use gpui_kit::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};
use views::BoardView;

/// データベースの置き場所を決める。
///
/// GUI から起動するとカレントディレクトリが当てにならないため、相対パスは使わない。
/// `EKANBAN_DATABASE` が指定されていればそれを、なければ OS ごとの標準の場所を使う。
pub fn database_path() -> PathBuf {
    if let Some(path) = std::env::var_os("EKANBAN_DATABASE") {
        return PathBuf::from(path);
    }
    paths::data_dir().join("ekanban.sqlite3")
}

pub fn run() {
    diagnostics::install_panic_hook();

    let path = database_path();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                diagnostics::report_fatal(&format!(
                    "failed to create {}: {error}",
                    parent.display()
                ));
                return;
            }
        }
    }
    let board = match Database::open(&path).and_then(|database| {
        let board = database.load_board()?;
        Ok(board)
    }) {
        Ok(value) => value,
        Err(error) => {
            diagnostics::report_fatal(&format!("failed to open {}: {error}", path.display()));
            return;
        }
    };

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        cx.on_action(|_: &actions::HideApplication, cx| cx.hide());
        cx.on_action(|_: &actions::HideOtherApplications, cx| cx.hide_other_apps());
        cx.on_action(|_: &actions::ShowAllApplications, cx| cx.unhide_other_apps());
        menu::install(cx);
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| BoardView::new(board, path, window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open ekanban window");

        cx.activate(true);
    });
}
