//! アプリを起動する。
//!
//! 段階 3 の時点では、ウィンドウを 1 つ開いて盤面を読むところまでです。矩形の
//! 復元・テーマ・メニュー・キー割り当ては段階 6、クイックキャプチャのウィンドウ
//! と割り当ては段階 8 で足します（`docs/TAURI-MIGRATION.md`）。

use ekanban_core::{database_path, diagnostics, instance};
use tauri::Manager as _;

use crate::commands;
use crate::ipc;

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

    let (state, _startup) = match commands::load_startup_state(&path) {
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

    let result = tauri::Builder::default()
        .manage(state)
        .setup(|app| {
            // 画面が組み上がってから出す。設定で `visible: false` にしてある。
            if let Some(window) = app.get_webview_window("board") {
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
            ipc::suggested_export_name,
            ipc::export_board,
            ipc::backup_database,
            ipc::database_location,
            ipc::reveal_database,
            ipc::reveal_backups,
            ipc::capture_card,
            ipc::set_capture_target,
            ipc::set_quick_capture_shortcut,
            ipc::log_frontend_error,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        diagnostics::report_fatal(&format!("failed to start ekanban: {error}"));
    }
}
