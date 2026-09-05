pub mod actions;
pub mod db;
pub mod diagnostics;
pub mod hotkey;
pub mod menu;
pub mod model;
pub mod paths;
pub mod views;

use std::path::PathBuf;

use db::{Database, WindowBoundsState};
use gpui_kit::component::{Root, Theme};
use gpui_kit::{
    px, size, App, AppContext, BorrowAppContext as _, Bounds, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use hotkey::{QuickCapture, Shortcut};
use views::{parse_theme_preference, window_title, BoardView, CaptureTarget, QuickCaptureState};

/// ウィンドウタイトルやバンドルに使うアプリ名。`script/bundle-mac` の `APP_NAME` と揃える。
pub const APP_NAME: &str = "Ekanban";

/// デスクトップ環境がウィンドウをアプリに結びつけるための識別子。
/// `script/bundle-mac` の `BUNDLE_ID` と揃える。
pub const APP_ID: &str = "dev.tokuhirom.ekanban";

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
    let (
        board,
        boards,
        filter_state,
        saved_window_bounds,
        theme_preference,
        sidebar_collapsed,
        quick_capture_shortcut,
        saved_capture_target,
    ) = match Database::open(&path).and_then(|database| {
        let boards = database.load_boards()?;
        let board_id = database
            .load_last_board_id()?
            .filter(|board_id| boards.iter().any(|board| board.id == *board_id))
            .or_else(|| boards.first().map(|board| board.id))
            .ok_or(db::DbError::NoBoard)?;
        let board = database.load_board_by_id(board_id)?;
        database.set_last_board_id(board.id)?;
        let mut filter_state = database.load_filter_state().unwrap_or_default();
        if filter_state
            .tag_id
            .is_some_and(|tag_id| !board.tags.iter().any(|tag| tag.id == tag_id))
        {
            filter_state.tag_id = None;
            database.set_filter_state(&filter_state)?;
        }
        let saved_window_bounds = database.load_window_bounds().ok().flatten();
        let theme_preference =
            parse_theme_preference(database.load_theme_preference().ok().flatten().as_deref());
        let sidebar_collapsed = database.load_sidebar_collapsed().unwrap_or(false);
        let quick_capture_shortcut = database.load_quick_capture_shortcut().unwrap_or(None);
        // キャプチャ先のボードやカラムが消えていたら、黙って既定に戻す。
        // フィルター状態の復元と同じ扱いで、起動を妨げない。
        let capture_target = match database.load_capture_target().unwrap_or(None) {
            Some((capture_board_id, capture_column_id)) => {
                let column_name = database
                    .load_column_name(capture_board_id, capture_column_id)
                    .unwrap_or(None);
                let board_name = boards
                    .iter()
                    .find(|summary| summary.id == capture_board_id)
                    .map(|summary| summary.name.clone());
                match (board_name, column_name) {
                    (Some(board_name), Some(column_name)) => Some(CaptureTarget {
                        board_id: capture_board_id,
                        column_id: capture_column_id,
                        board_name,
                        column_name,
                    }),
                    _ => {
                        database.set_capture_target(None)?;
                        None
                    }
                }
            }
            None => None,
        };
        Ok((
            board,
            boards,
            filter_state,
            saved_window_bounds,
            theme_preference,
            sidebar_collapsed,
            quick_capture_shortcut,
            capture_target,
        ))
    }) {
        Ok(value) => value,
        Err(error) => {
            diagnostics::report_fatal(&format!("failed to open {}: {error}", path.display()));
            return;
        }
    };

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        Theme::sync_system_appearance(None, cx);
        Theme::sync_scrollbar_appearance(cx);
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        cx.on_action(|_: &actions::HideApplication, cx| cx.hide());
        cx.on_action(|_: &actions::HideOtherApplications, cx| cx.hide_other_apps());
        cx.on_action(|_: &actions::ShowAllApplications, cx| cx.unhide_other_apps());
        menu::install(cx);
        cx.set_global(QuickCapture::new());
        let (shortcut, shortcut_error) = register_saved_shortcut(quick_capture_shortcut, cx);
        let quick_capture = QuickCaptureState {
            shortcut,
            error: shortcut_error,
            capture_target: saved_capture_target,
        };
        let bounds = restored_window_bounds(saved_window_bounds, cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // Linux にはバンドルが無く、ここで渡さないとタイトルも
                // アプリ識別子も設定されないまま WM に渡る。
                titlebar: Some(TitlebarOptions {
                    title: Some(window_title(&board.name).into()),
                    ..Default::default()
                }),
                app_id: Some(APP_ID.to_string()),
                ..Default::default()
            },
            move |window, cx| {
                let view = cx.new(|cx| {
                    BoardView::new(
                        board,
                        boards,
                        path,
                        filter_state,
                        WindowBoundsState {
                            x: window.bounds().origin.x.as_f32(),
                            y: window.bounds().origin.y.as_f32(),
                            width: window.bounds().size.width.as_f32(),
                            height: window.bounds().size.height.as_f32(),
                        },
                        theme_preference,
                        sidebar_collapsed,
                        quick_capture,
                        window,
                        cx,
                    )
                });
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open ekanban window");

        cx.activate(true);
    });
}

/// 保存済みの割り当てを登録する。
///
/// 登録できなかった理由は捨てずに持ち回し、`BoardView` の通知に出す。起動のたび
/// 黙って失敗する状態を作らない。設定そのものは消さない（ほかのアプリを閉じれば
/// 次の起動では通る可能性があるため）。
fn register_saved_shortcut(
    saved: Option<String>,
    cx: &mut App,
) -> (Option<Shortcut>, Option<String>) {
    let Some(saved) = saved else {
        return (None, None);
    };

    let shortcut = match Shortcut::parse(&saved) {
        Ok(shortcut) => shortcut,
        Err(error) => {
            return (
                None,
                Some(format!(
                    "保存されているクイックキャプチャの割り当てを読み取れませんでした: {error}"
                )),
            )
        }
    };

    match cx.update_global::<QuickCapture, _>(|quick_capture, _| {
        quick_capture.set(Some(shortcut.clone()))
    }) {
        Ok(()) => (Some(shortcut), None),
        Err(message) => (None, Some(message)),
    }
}

fn restored_window_bounds(saved: Option<WindowBoundsState>, cx: &App) -> Bounds<gpui_kit::Pixels> {
    let default_size = size(px(1200.), px(800.));
    let Some(saved) = saved else {
        return Bounds::centered(None, default_size, cx);
    };
    let bounds = Bounds {
        origin: gpui_kit::point(px(saved.x), px(saved.y)),
        size: size(px(saved.width), px(saved.height)),
    };
    let visible_on_display = cx.displays().iter().any(|display| {
        let visible = display.visible_bounds();
        bounds.origin.x >= visible.left()
            && bounds.right() <= visible.right()
            && bounds.origin.y >= visible.top()
            && bounds.bottom() <= visible.bottom()
    });
    if visible_on_display {
        bounds
    } else {
        Bounds::centered(None, default_size, cx)
    }
}
