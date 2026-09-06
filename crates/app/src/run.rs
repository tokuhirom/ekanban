//! アプリを起動する。
//!
//! ウィンドウを 1 つ開き、メニューバーを組み、覚えていた矩形を戻します
//! （`docs/DESIGN.md`「メニューとキー割り当て」「ウィンドウ」）。クイックキャプチャの
//! ウィンドウと、覚えていたグローバルな割り当ての登録もここから始まります。

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

    // **データベースに触るより前に断ります。** 画面が出ないと分かっている起動で
    // ロックを握り、控えを取り、盤面を読むのは、どれも無駄で、どれも副作用が
    // あります。
    let context = tauri::generate_context!();
    if let Err(reason) = check_dev_server(context.config()) {
        diagnostics::report_fatal(&reason);
        return;
    }

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
    // 単位」を壊す（`docs/DESIGN.md`「ウィンドウ」）。
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
    let saved_shortcut = startup.quick_capture_shortcut.clone();
    let bounds_for_events = Arc::clone(&bounds);

    let app = tauri::Builder::default()
        // ファイルを選ばせるのと、場所を開くのに使う（`docs/DESIGN.md`「アプリが伝えること」）。どちらも Rust から
        // 呼ぶので、webview に権限は開けていません。
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
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
            // 保存されている割り当てを登録する。登録できなくても起動は続け、
            // 理由は記録に残す（設定は消さない、`docs/DESIGN.md`「クイックキャプチャ」）。
            if let Some(reason) =
                crate::capture::register_saved(app.handle(), saved_shortcut.as_deref())
            {
                diagnostics::log(&format!("quick capture is not registered: {reason}"));
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
            ipc::capture_target,
            ipc::set_capture_target,
            ipc::set_capture_column,
            ipc::quick_capture_support,
            ipc::set_quick_capture_shortcut,
            ipc::set_quick_capture_shortcut_from_key,
            ipc::close_capture_window,
            ipc::log_frontend_error,
        ])
        .build(context);

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

/// デバッグビルドが繋ぎに行く開発サーバが、上がっているか。
///
/// **デバッグビルドには `devUrl` が焼き込まれています。** 開発サーバが無いまま
/// 起動すると、ウィンドウは開いてメニューバーまで出るのに、中身が webview の
/// 「Connection refused」になります。**動いているように見えて動いていない**ので、
/// ここで先に断って、次に何をすればいいかを出します
/// （`docs/DESIGN.md`「アプリが伝えること」）。
///
/// リリースビルドは画面を埋め込んでいるので、この関門は通りません。
fn check_dev_server(config: &tauri::Config) -> Result<(), String> {
    if !tauri::is_dev() {
        return Ok(());
    }
    let Some(url) = config.build.dev_url.as_ref() else {
        return Ok(());
    };
    if dev_server_is_up(url) {
        return Ok(());
    }

    // 行ごとに書くのは、打ってもらうコマンドの字下げをそのまま残すためです。
    // 文字列の行末に `\` を置くと、次の行の頭の空白まで消えます。
    Err([
        &format!("画面を読み込めませんでした（{url} に繋がりません）。"),
        "",
        "デバッグビルドは Vite の開発サーバから画面を読みます。",
        "次のどちらかで起動してください。",
        "",
        "  make dev",
        "      開発サーバごと起動する（ふだんはこちら）",
        "",
        "  cd crates/app && ../../web/node_modules/.bin/tauri build --debug --no-bundle",
        "      画面を埋め込んだバイナリを作る（できたら target/debug/ekanban）",
    ]
    .join("\n"))
}

/// `url` の宛先に TCP で繋げるか。
///
/// HTTP は投げません。**知りたいのは「誰かが待ち受けているか」だけ**で、それは
/// 接続できるかどうかで分かります。名前が引けない・宛先が無いときも「上がって
/// いない」に倒します——起動を止める判断なので、迷ったら通さないほうではなく、
/// 理由を出すほうに倒しています。
fn dev_server_is_up(url: &tauri::Url) -> bool {
    use std::net::{TcpStream, ToSocketAddrs as _};
    use std::time::Duration;

    let Some(host) = url.host_str() else {
        return false;
    };
    let Some(port) = url.port_or_known_default() else {
        return false;
    };
    let Ok(addresses) = (host, port).to_socket_addrs() else {
        return false;
    };
    // 相手は自分の機械の中にいるので、待つのは一瞬でいい。ここで長く待つと、
    // 開発サーバを上げ忘れたときに「固まった」ように見えます。
    let timeout = Duration::from_millis(500);
    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, timeout).is_ok())
}

/// アプリそのものに届く出来事。
///
/// macOS ではウィンドウを閉じてもプロセスが残ります。閉じたあとに Dock の
/// アイコンから戻れる必要があり、そこを `Reopen` が受けます（`docs/DESIGN.md`「ウィンドウ」）。ほかの
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

/// メニューが押されたときの行き先（`docs/DESIGN.md`「メニューとキー割り当て」）。
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::TcpListener;

    /// 待ち受けている相手には繋がる。
    ///
    /// ポートを固定しないのは、走らせる機械で埋まっているかもしれないためです。
    /// 0 番を頼むと OS が空いているものを選びます。
    #[test]
    fn finds_a_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("空いているポートがある");
        let port = listener.local_addr().expect("番地が読める").port();
        let url = tauri::Url::parse(&format!("http://127.0.0.1:{port}")).expect("URL になる");

        assert!(dev_server_is_up(&url));
    }

    /// 誰も待ち受けていなければ、上がっていないと見る。
    ///
    /// **繋いだ相手を閉じてから聞きます。** 開いたまま番号だけ変えると、たまたま
    /// 別のものが使っている番号を引いて、通ってしまうことがあります。
    #[test]
    fn misses_a_closed_port() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("空いているポートがある");
        let port = listener.local_addr().expect("番地が読める").port();
        drop(listener);
        let url = tauri::Url::parse(&format!("http://127.0.0.1:{port}")).expect("URL になる");

        assert!(!dev_server_is_up(&url));
    }

    /// 名前が引けないときも「上がっていない」に倒す。
    #[test]
    fn treats_an_unresolvable_host_as_down() {
        let url = tauri::Url::parse("http://ekanban.invalid:1420").expect("URL になる");

        assert!(!dev_server_is_up(&url));
    }
}
