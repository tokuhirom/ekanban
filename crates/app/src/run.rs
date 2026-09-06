//! アプリを起動する。
//!
//! ウィンドウを 1 つ開き、メニューバーを組み、覚えていた矩形を戻します
//! （`docs/TAURI-MIGRATION.md` §7・§8）。クイックキャプチャのウィンドウと
//! グローバルな割り当ては段階 8 で足します。

use std::sync::Arc;

use ekanban_core::{database_path, diagnostics, instance};
use tauri::{Emitter as _, Manager as _, RunEvent, WindowEvent};

use crate::commands;
use crate::events;
use crate::ipc;
use crate::menu::{self, Action, WindowAction};
use crate::window::BoundsSaver;

/// 盤面のウィンドウのラベル。`tauri.conf.json` と揃えてあります。
pub(crate) const BOARD_WINDOW: &str = "board";

/// 起動の入口。失敗したら記録して静かに終わる。
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

    // データベースを開くより前に握る。開いてからでは `migrate` と `seed_if_empty`
    // が 2 つのプロセスから走る。ロックは `run()` が終わるまで持ったままにする
    // （落とすと外れる）。`tauri-plugin-single-instance` は使わない——あれは
    // アプリ 1 つに対する制限で、ADR 0004 が決めた「ロックはデータベースのパス
    // 単位」を壊す（§8）。
    let _instance = match instance::acquire(&path) {
        Ok(lock) => lock,
        Err(instance::InstanceError::AlreadyRunning(_)) => {
            diagnostics::report_fatal(&format!(
                "ekanban はすでに起動しています（{}）。\n\n\
                 同じデータベースを 2 つのプロセスで開くと、あとから保存したほうが\n\
                 もう片方で足したカードを消してしまうため、2 つ目は起動しません。\n\
                 開いているウィンドウを使ってください。",
                path.display()
            ));
            return;
        }
        Err(error) => {
            diagnostics::report_fatal(&format!("起動中かどうかを確かめられませんでした: {error}"));
            return;
        }
    };

    let (state, startup) = match commands::load_startup_state(&path) {
        Ok(loaded) => loaded,
        Err(error) => {
            diagnostics::report_fatal(&format!(
                "failed to open {}: {}\n\n{}",
                path.display(),
                error.title,
                error.detail
            ));
            return;
        }
    };

    // その日ぶんの控えを 1 つ残す。起動を遅らせないよう別のスレッドで取り、
    // 失敗しても起動は止めない（`docs/DESIGN.md`）。
    let backup_source = path.clone();
    std::thread::spawn(move || commands::run_daily_backup(&backup_source));

    // 矩形を覚える先。動かしている間の値をまとめて、静まってから 1 回書く。
    let bounds = Arc::new(BoundsSaver::spawn(path.clone()));
    let saved_bounds = startup.window_bounds;
    let bounds_for_events = Arc::clone(&bounds);

    let app = tauri::Builder::default()
        // ファイルを選ばせるのと、場所を開くのに使う（§9）。どちらも Rust から
        // 呼ぶので、webview に権限は開けていません。
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .menu(menu::build)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_window_event(move |window, event| {
            if window.label() != BOARD_WINDOW {
                return;
            }
            if !matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
                return;
            }
            let Some(window) = window.app_handle().get_webview_window(BOARD_WINDOW) else {
                return;
            };
            if let Some(current) = crate::window::current_bounds(&window) {
                bounds_for_events.record(current);
            }
        })
        .setup(move |app| {
            // 画面が組み上がってから出す。設定で `visible: false` にしてある。
            // **戻すのは出す前**にする。出してから動かすと、既定の位置で一度
            // 描かれてから飛ぶ。
            if let Some(window) = app.get_webview_window(BOARD_WINDOW) {
                crate::window::restore(&window, saved_bounds);
                window.show()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::startup_state,
            ipc::snapshot,
            ipc::create_board,
            ipc::rename_board,
            ipc::delete_board,
            ipc::switch_board,
            ipc::add_card,
            ipc::update_card,
            ipc::move_card,
            ipc::copy_card,
            ipc::delete_card,
            ipc::archive_card,
            ipc::restore_card,
            ipc::set_card_due_date,
            ipc::set_card_tags,
            ipc::add_column,
            ipc::rename_column,
            ipc::remove_column,
            ipc::move_column,
            ipc::set_column_wip_limit,
            ipc::sort_column_by_due_date,
            ipc::archive_column,
            ipc::add_tag,
            ipc::rename_tag,
            ipc::set_tag_color,
            ipc::remove_tag,
            ipc::undo,
            ipc::redo,
            ipc::filter_cards,
            ipc::set_filter_state,
            ipc::set_theme_preference,
            ipc::set_sidebar_collapsed,
            ipc::set_window_bounds,
            ipc::set_window_title,
            ipc::suggested_export_name,
            ipc::choose_save_path,
            ipc::export_board,
            ipc::backup_database,
            ipc::database_location,
            ipc::reveal_path,
            ipc::reveal_database,
            ipc::reveal_backups,
            ipc::description_links,
            ipc::open_url,
            ipc::capture_card,
            ipc::set_capture_target,
            ipc::set_quick_capture_shortcut,
            ipc::log_frontend_error,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(error) => {
            diagnostics::report_fatal(&format!("failed to start ekanban: {error}"));
            return;
        }
    };

    app.run(move |app, event| {
        // 終わる前に、まだ書いていない矩形を書ききる。ここで書かないと、
        // 動かしてすぐ終了したぶんが落ちる。
        if matches!(event, RunEvent::Exit) {
            bounds.flush();
        }
        handle_run_event(app, &event);
    });
}

/// アプリそのものに届く出来事。
///
/// macOS ではウィンドウを閉じてもプロセスが残ります。閉じたあとに Dock の
/// アイコンから戻れる必要があり、そこを `Reopen` が受けます（§8）。ほかの
/// 環境では、閉じたら終わりで正しい。
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn handle_run_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: &RunEvent) {
    match event {
        RunEvent::ExitRequested { api, .. } if cfg!(target_os = "macos") => api.prevent_exit(),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => reopen(app),
        _ => {}
    }
}

/// メニューが押されたときの行き先（§7）。
///
/// 盤面と下書きに触るものは webview へ流します。**ここで盤面を触りません**
/// ——開いているパネルや選んでいるカードを知っているのは画面のほうで、
/// 同じ判断を 2 か所に置くとずれます。
fn handle_menu_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) {
    match Action::from_id(id) {
        Some(Action::App(action)) => {
            if let Err(error) = app.emit_to(BOARD_WINDOW, events::APP_ACTION, action) {
                diagnostics::log(&format!("failed to deliver {id} to the board: {error}"));
            }
        }
        Some(Action::Window(WindowAction::CloseWindow)) => {
            if let Some(window) = app.get_webview_window(BOARD_WINDOW) {
                let _ = window.close();
            }
        }
        Some(Action::Window(WindowAction::ToggleFullscreen)) => {
            if let Some(window) = app.get_webview_window(BOARD_WINDOW) {
                let full = window.is_fullscreen().unwrap_or(false);
                let _ = window.set_fullscreen(!full);
            }
        }
        Some(Action::Window(WindowAction::Quit)) => app.exit(0),
        // OS が持っている項目（カット・ペースト・隠す）はここへ来ない。
        None => {}
    }
}

/// Dock のアイコンを押されたときに、閉じたウィンドウを開き直す（macOS）。
///
/// ほかの環境では最後のウィンドウを閉じた時点でプロセスも終わるので、開き直す
/// 相手がいません。
///
/// 開いているなら前面に出すだけ。**盤面を読み直すのは webview の側**で、
/// 開いた画面が `startup_state` を呼びます。閉じている間にクイックキャプチャが
/// 足したカードも、それで出ます（`docs/DESIGN.md`）。
#[cfg(target_os = "macos")]
fn reopen<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window(BOARD_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let Some(config) = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == BOARD_WINDOW)
        .cloned()
    else {
        diagnostics::log("the board window is missing from tauri.conf.json");
        return;
    };

    match tauri::WebviewWindowBuilder::from_config(app, &config) {
        Ok(builder) => match builder.build() {
            Ok(window) => {
                let _ = window.show();
            }
            Err(error) => diagnostics::log(&format!("failed to reopen the board: {error}")),
        },
        Err(error) => diagnostics::log(&format!("failed to reopen the board: {error}")),
    }
}
