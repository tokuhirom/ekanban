pub mod actions;
pub mod backup;
pub mod db;
pub mod diagnostics;
pub mod hotkey;
pub mod menu;
pub mod model;
pub mod paths;
pub mod views;

use std::path::{Path, PathBuf};

use chrono::Local;
use db::{Database, FilterState, WindowBoundsState};
use gpui_kit::component::{Root, Theme};
use gpui_kit::{
    px, size, App, AppContext, BorrowAppContext as _, Bounds, TitlebarOptions, WindowBounds,
    WindowOptions,
};
use hotkey::{QuickCapture, Shortcut};
use model::{Board, BoardSummary};
use views::{
    parse_theme_preference, window_title, BoardView, CaptureTarget, QuickCaptureState,
    ThemePreference,
};

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

/// 起動時とウィンドウを開き直すときに、データベースから読み直す状態。
///
/// メモリ上の値を抱えて使い回さない。ウィンドウを閉じている間もクイックキャプチャ
/// はカードを足せるので、閉じたときの値で開き直すと古い盤面が出る（`docs/DESIGN.md`）。
pub(crate) struct StartupState {
    board: Board,
    boards: Vec<BoardSummary>,
    filter_state: FilterState,
    window_bounds: Option<WindowBoundsState>,
    theme_preference: ThemePreference,
    sidebar_collapsed: bool,
    quick_capture_shortcut: Option<String>,
    capture_target: Option<CaptureTarget>,
}

/// 開くボードと、その付随状態をデータベースから読む。
///
/// 最後に開いていたボードが消えていれば先頭のボードに、絞り込みのタグや
/// キャプチャ先が消えていれば既定に、それぞれ黙って戻す。起動を妨げない。
pub(crate) fn load_startup_state(path: &Path) -> Result<StartupState, db::DbError> {
    let database = Database::open(path)?;
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
    let window_bounds = database.load_window_bounds().ok().flatten();
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

    Ok(StartupState {
        board,
        boards,
        filter_state,
        window_bounds,
        theme_preference,
        sidebar_collapsed,
        quick_capture_shortcut,
        capture_target,
    })
}

/// ボードのウィンドウを開く。起動のときも、閉じたあとに開き直すときも、ここを通る。
fn open_board_window(
    path: PathBuf,
    state: StartupState,
    quick_capture: QuickCaptureState,
    cx: &mut App,
) -> gpui_kit::Result<()> {
    let StartupState {
        board,
        boards,
        filter_state,
        window_bounds,
        theme_preference,
        sidebar_collapsed,
        ..
    } = state;
    let bounds = restored_window_bounds(window_bounds, cx);
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
    )?;
    cx.activate(true);
    Ok(())
}

/// Dock のアイコンを押されたときに、閉じたウィンドウを開き直す。
///
/// macOS では最後のウィンドウを閉じてもプロセスが残る（gpui の `QuitMode::Default`）。
/// 受け口が無いと、`Cmd+W` のあとは `Cmd+Q` で終了して起動し直すしか道がない。
fn reopen_board_window(path: &Path, cx: &mut App) {
    // 既に開いているなら前面に出すだけ。ボードのウィンドウを二重に開かない。
    if !cx.windows().is_empty() {
        cx.activate(true);
        return;
    }

    let state = match load_startup_state(path) {
        Ok(state) => state,
        Err(error) => {
            diagnostics::log(&format!("failed to reopen {}: {error}", path.display()));
            return;
        }
    };
    // 割り当てはアプリが持ったままなので登録し直さない。起動時のエラーも
    // 蒸し返さない。
    let quick_capture = QuickCaptureState {
        // 起動の途中で押されても落ちないよう `try_global` で見る。開き直しの
        // ハンドラでパニックすると、そのままアプリごと落ちる。
        shortcut: cx
            .try_global::<QuickCapture>()
            .and_then(|quick_capture| quick_capture.registered().cloned()),
        error: None,
        capture_target: state.capture_target.clone(),
    };
    if let Err(error) = open_board_window(path.to_path_buf(), state, quick_capture, cx) {
        diagnostics::log(&format!("failed to reopen {}: {error}", path.display()));
    }
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

    let state = match load_startup_state(&path) {
        Ok(state) => state,
        Err(error) => {
            diagnostics::report_fatal(&format!("failed to open {}: {error}", path.display()));
            return;
        }
    };

    // その日ぶんの控えを 1 つ残す。起動を遅らせないよう別のスレッドで取り、
    // 失敗しても起動は止めない（`docs/DESIGN.md`）。取るのは起動時で、終了時では
    // ない。終了時に取ると、壊した状態のほうを保存することになる。
    let backup_source = path.clone();
    std::thread::spawn(move || {
        if let Err(error) = backup::run_daily(&backup_source, Local::now().date_naive()) {
            diagnostics::log(&format!(
                "failed to back up {}: {error}",
                backup_source.display()
            ));
        }
    });

    let application = gpui_kit::application();
    // `on_reopen` は `App` ではなく `Application` に生えているので、`run` の前に
    // 登録する。
    let reopen_path = path.clone();
    application.on_reopen(move |cx| reopen_board_window(&reopen_path, cx));

    application.run(move |cx: &mut App| {
        gpui_kit::init(cx);
        Theme::sync_system_appearance(None, cx);
        Theme::sync_scrollbar_appearance(cx);
        cx.on_action(|_: &actions::Quit, cx| cx.quit());
        cx.on_action(|_: &actions::HideApplication, cx| cx.hide());
        cx.on_action(|_: &actions::HideOtherApplications, cx| cx.hide_other_apps());
        cx.on_action(|_: &actions::ShowAllApplications, cx| cx.unhide_other_apps());
        menu::install(cx);
        cx.set_global(QuickCapture::new());
        let (shortcut, shortcut_error) =
            register_saved_shortcut(state.quick_capture_shortcut.clone(), cx);
        let quick_capture = QuickCaptureState {
            shortcut,
            error: shortcut_error,
            capture_target: state.capture_target.clone(),
        };
        open_board_window(path, state, quick_capture, cx).expect("failed to open ekanban window");
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

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    /// ウィンドウを開き直すたびに通る経路。閉じている間に変わった内容が出る
    /// ことと、`app_state` に残した表示の状態が戻ることを見る。
    #[test]
    fn reads_the_startup_state_from_the_database() {
        let directory = tempdir().expect("a temporary directory is available");
        let path = directory.path().join("board.sqlite3");

        let stored = {
            let database = Database::open(&path).expect("a new database is created");
            database
                .set_theme_preference("dark")
                .expect("the theme is stored");
            database
                .set_sidebar_collapsed(true)
                .expect("the sidebar state is stored");
            database
                .set_window_bounds(WindowBoundsState {
                    x: 10.,
                    y: 20.,
                    width: 900.,
                    height: 600.,
                })
                .expect("the window rectangle is stored");
            database.load_board().expect("the seeded board loads")
        };

        let state = load_startup_state(&path).expect("the startup state is read back");

        assert_eq!(state.board, stored, "the stored board is the one opened");
        assert_eq!(state.theme_preference, ThemePreference::Dark);
        assert!(state.sidebar_collapsed);
        assert_eq!(
            state.window_bounds.map(|bounds| bounds.width),
            Some(900.),
            "the window comes back the size it was left"
        );
    }

    #[test]
    fn sees_the_cards_added_while_the_window_was_closed() {
        let directory = tempdir().expect("a temporary directory is available");
        let path = directory.path().join("board.sqlite3");

        {
            let mut database = Database::open(&path).expect("a new database is created");
            let mut board = database.load_board().expect("the seeded board loads");
            let column_id = board.columns[0].id;
            board
                .add_card(column_id, "閉じている間に足したカード", "")
                .expect("the column takes a card");
            database.save_board(&mut board).expect("the card is stored");
        }

        let state = load_startup_state(&path).expect("the startup state is read back");

        assert!(
            state.board.columns.iter().any(|column| column
                .cards
                .iter()
                .any(|card| card.title == "閉じている間に足したカード")),
            "reopening reads the database rather than the state the window was closed with"
        );
    }
}
