//! クイックキャプチャの窓と、グローバルな割り当て（`docs/DESIGN.md`「クイックキャプチャ」、[ADR 0012]）。
//!
//! ホットキーで 1 行入力の小さい窓を出し、`Enter` で 1 枚足して閉じます。
//! **書くのはボードと同じ経路**（`save_document`）なので、Undo の対象に
//! なり、`card_events` にも 1 件積まれます。
//!
//! ここは Tauri を知っている側です。窓の開け閉めと登録がここにあり、盤面に触る
//! 判断は `commands` に残っています。
//!
//! [ADR 0012]: ../../../docs/adr/0012-focus-after-quick-capture-on-linux.md

use std::sync::Mutex;

use ekanban_core::diagnostics;
use tauri::{AppHandle, Manager as _, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt as _;

use crate::commands;
use crate::error::{AppError, ErrorKind};
use crate::run::BOARD_WINDOW;
use crate::shortcut::{platform_support, KeyPress, Shortcut};
use crate::snapshot::QuickCaptureStatus;
use crate::state::AppState;

/// キャプチャの窓のラベル。
pub(crate) const CAPTURE_WINDOW: &str = "capture";

/// 保存されている割り当てが、いま効いているかどうか。
///
/// **登録に失敗した理由を捨てません**（`docs/DESIGN.md`「クイックキャプチャ」、
/// [ADR 0030]）。保存されているだけで登録できていない割り当ては、設定を見ても
/// 効いているようにしか見えないので、押しても何も起きない理由が誰にも分かり
/// ません。ここに残して、割り当てのダイアログがその場で読みます。
///
/// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md
#[derive(Default)]
pub struct Registration(Mutex<Option<String>>);

impl Registration {
    fn set(&self, failure: Option<String>) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = failure;
    }

    fn failure(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// いまの状態を読む。
pub(crate) fn status(registration: &Registration) -> QuickCaptureStatus {
    QuickCaptureStatus {
        unavailable: platform_support().err(),
        failure: registration.failure(),
    }
}

/// 起動のときに、保存されている割り当てを登録する。
///
/// 登録できなかった理由は捨てずに返します。**起動のたび黙って失敗する状態を
/// 作りません。** 設定そのものは消しません——ほかのアプリを閉じれば、次の起動
/// では通るかもしれないからです。
pub(crate) fn register_saved<R: Runtime>(
    app: &AppHandle<R>,
    registration: &Registration,
    saved: Option<&str>,
) -> Option<String> {
    let failure = saved.and_then(|saved| match Shortcut::parse(saved) {
        Ok(shortcut) => register(app, &shortcut).err(),
        Err(error) => Some(format!(
            "保存されている割り当て「{saved}」を読み取れませんでした: {error}"
        )),
    });
    registration.set(failure.clone());
    failure
}

/// 割り当てを登録する。前の割り当ては、呼ぶ側が先に外しておく。
fn register<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut) -> Result<(), String> {
    // 登録の戻り値は当てにならない環境がある（X11）。環境そのものを先に見る。
    platform_support()?;

    let global = shortcut.to_global();
    app.global_shortcut()
        .on_shortcut(global, move |app, _, event| {
            // 押した瞬間だけ。離したときにも来るので、そこで二度開かない。
            if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                open(app);
            }
        })
        .map_err(|error| format!("「{shortcut}」を登録できませんでした: {error}"))?;
    Ok(())
}

/// 画面から届いた押しかたを、割り当てとして受け取る。
///
/// **登録に成功してから保存します**（`docs/DESIGN.md`「クイックキャプチャ」）。順番が逆だと、次の起動で黙って
/// 失敗する割り当てが残ります。`None` で解除します。
pub(crate) fn set<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    registration: &Registration,
    press: Option<KeyPress>,
) -> Result<Option<String>, AppError> {
    let shortcut = match press {
        Some(press) => Some(Shortcut::from_key_press(&press).map_err(|error| {
            AppError::new(
                ErrorKind::Shortcut,
                "ショートカットを割り当てられません",
                error.to_string(),
            )
        })?),
        None => None,
    };

    let previous = commands::quick_capture_shortcut(state)?;

    // 前のものを外してから登録する。同じキーを 2 つ登録できない環境がある。
    if let Err(error) = app.global_shortcut().unregister_all() {
        diagnostics::log(&format!("failed to release the old shortcut: {error}"));
    }

    if let Some(shortcut) = shortcut.as_ref() {
        if let Err(reason) = register(app, shortcut) {
            // **外したものを戻します。** 設定は変更前のまま残るので（`docs/DESIGN.md`
            // 「クイックキャプチャ」）、戻さなければ、保存されているのに次の起動まで
            // 効かない割り当てができます。
            register_saved(app, registration, previous.as_deref());
            return Err(AppError::new(
                ErrorKind::Shortcut,
                "ショートカットを登録できませんでした",
                reason,
            ));
        }
    }

    let stored = shortcut.as_ref().map(Shortcut::to_string);
    commands::set_quick_capture_shortcut(state, stored.as_deref())?;
    registration.set(None);
    Ok(stored)
}

/// ホットキーが押されたとき。1 行入力の窓を出す。
///
/// すでに出ているなら前に出すだけ。二重に開きません。
pub(crate) fn open<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(CAPTURE_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let built =
        WebviewWindowBuilder::new(app, CAPTURE_WINDOW, WebviewUrl::App("capture.html".into()))
            .title("クイックキャプチャ")
            .inner_size(480., 132.)
            .resizable(false)
            .center()
            .always_on_top(true)
            // 装飾なしにすると、ウィンドウマネージャによっては閉じる手が無くなる。
            // 閉じるのは `Escape` だが、それが効かない状態のために枠は残す。
            .decorations(true)
            .skip_taskbar(true)
            .build();

    match built {
        Ok(window) => {
            // メニューバーはアプリ全体のもので、放っておくとこの窓にも付く
            // （macOS 以外）。1 行を放り込むだけの窓に「ファイル」から
            // 「ヘルプ」まで並ぶと、窓の半分が menu になる。
            #[cfg(not(target_os = "macos"))]
            let _ = window.remove_menu();
            let _ = window.set_focus();
        }
        Err(error) => diagnostics::log(&format!("failed to open the capture window: {error}")),
    }
}

/// キャプチャの窓を閉じ、フォーカスをボードへ返す。
///
/// 直前のアプリへ返すことは、Linux では ekanban 側で決められません（[ADR 0012]）。
/// **保証するのは「ボードの窓が後ろにいれば前に出る」ところまで**です。
///
/// [ADR 0012]: ../../../docs/adr/0012-focus-after-quick-capture-on-linux.md
pub(crate) fn close<R: Runtime>(app: &AppHandle<R>, focus_board: bool) {
    if let Some(window) = app.get_webview_window(CAPTURE_WINDOW) {
        let _ = window.close();
    }
    if focus_board {
        if let Some(board) = app.get_webview_window(BOARD_WINDOW) {
            let _ = window_to_front(&board);
        }
    }
}

fn window_to_front<R: Runtime>(window: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
    window.show()?;
    window.set_focus()
}
