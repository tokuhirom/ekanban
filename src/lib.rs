pub mod db;
pub mod model;
pub mod views;

use std::path::PathBuf;

use db::Database;
use gpui_kit::component::Root;
use gpui_kit::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};
use views::BoardView;

pub fn database_path() -> PathBuf {
    std::env::var_os("EKANBAN_DATABASE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".ekanban.sqlite3"))
}

pub fn run() {
    let path = database_path();
    let (database, board) = match Database::open(&path).and_then(|database| {
        let board = database.load_board()?;
        Ok((database, board))
    }) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("failed to open {}: {error}", path.display());
            return;
        }
    };

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|_| BoardView::new(board, database));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open ekanban window");

        cx.activate(true);
    });
}
