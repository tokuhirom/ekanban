//! クイックキャプチャの割り当て（`docs/DESIGN.md`「クイックキャプチャ」、[ADR 0012]）。
//!
//! **`app_state` に入る形（`ctrl-alt-shift-cmd-n`）を変えません。** この形で
//! 書かれたデータベースが既にあるので、変えれば移行が要り、読めなかった
//! 割り当ては黙って消えます。移行が要らないなら、作らないほうがよい。
//!
//! そのため、ここが 3 つの表記の間に立ちます。画面から届くのは
//! `KeyboardEvent.code`（`"KeyN"`）、登録に渡すのは `Code` と `Modifiers`、
//! 保存するのは `"n"` のようなキー名です。変換を両方向ともここに閉じ込めるので、
//! 外の 2 つは互いの表記を知りません。
//!
//! [ADR 0012]: ../../../docs/adr/0012-focus-after-quick-capture-on-linux.md

// 保存の形と `KeyboardEvent.code` の間の変換は、殻を外しても要ります
// （`KeyPress` は webview から届く形です）。**登録できる形（[`Shortcut`]）だけが
// 殻の側**——グローバルホットキーはブラウザに無いので、`shell` を外すと
// まるごと消えます（[ADR 0035]）。
//
// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
#[cfg(feature = "shell")]
use std::fmt;
#[cfg(feature = "shell")]
use std::str::FromStr;

#[cfg(feature = "shell")]
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut as GlobalShortcut};

/// 割り当てを受け付けられない理由。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShortcutError {
    #[error("ショートカットとして読み取れません: {0}")]
    Unparsable(String),
    #[error("修飾キーを 1 つ以上含めてください。修飾キーなしの割り当ては、ほかのアプリでそのキーが打てなくなります")]
    NoModifier,
    #[error("このキーはグローバルホットキーに使えません: {0}")]
    UnsupportedKey(String),
}

#[cfg(feature = "shell")]
/// クイックキャプチャに割り当てられたキーの組み合わせ。
///
/// 作れた時点で、グローバルホットキーとして登録できる形だと分かっています。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    /// 保存に使うキー名（`"n"`、`"f12"`、`"left"`）。`app_state` に入るのがこれ。
    key: String,
    code: Code,
}

#[cfg(feature = "shell")]
impl Shortcut {
    /// 保存してある文字列から復元する。
    pub fn parse(source: &str) -> Result<Self, ShortcutError> {
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut meta = false;
        let mut parts = source.split('-').peekable();
        let mut key = None;
        while let Some(part) = parts.next() {
            // 最後の 1 つがキー。手前は修飾キー。
            if parts.peek().is_none() {
                key = Some(part.to_string());
                break;
            }
            match part {
                "ctrl" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                "cmd" => meta = true,
                _ => return Err(ShortcutError::Unparsable(source.to_string())),
            }
        }
        let key = key
            .filter(|key| !key.is_empty())
            .ok_or_else(|| ShortcutError::Unparsable(source.to_string()))?;
        if !(ctrl || alt || shift || meta) {
            return Err(ShortcutError::NoModifier);
        }
        let code = key_code(&key).ok_or_else(|| ShortcutError::UnsupportedKey(key.clone()))?;
        Ok(Self {
            ctrl,
            alt,
            shift,
            meta,
            key,
            code,
        })
    }

    /// 登録に渡す形。
    pub fn to_global(&self) -> GlobalShortcut {
        let mut modifiers = Modifiers::empty();
        if self.ctrl {
            modifiers |= Modifiers::CONTROL;
        }
        if self.alt {
            modifiers |= Modifiers::ALT;
        }
        if self.shift {
            modifiers |= Modifiers::SHIFT;
        }
        if self.meta {
            // macOS では Cmd、X11 では Mod4（Super）に落ちる。
            modifiers |= Modifiers::SUPER;
        }
        GlobalShortcut::new(Some(modifiers), self.code)
    }
}

#[cfg(feature = "shell")]
impl fmt::Display for Shortcut {
    /// 保存と表示に使う正規形。修飾キーの順序を固定するので、`cmd-shift-n` と
    /// `shift-cmd-n` は同じ文字列になる。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("ctrl-")?;
        }
        if self.alt {
            f.write_str("alt-")?;
        }
        if self.shift {
            f.write_str("shift-")?;
        }
        if self.meta {
            f.write_str("cmd-")?;
        }
        f.write_str(&self.key)
    }
}

#[cfg(feature = "shell")]
/// 保存する側のキー名を W3C の `code` に直す。
///
/// **逆向きの表は画面側にあります**（`web/src/shell/shortcut.ts` の `keyName`）。
/// 押されたキーを文字列にするのは画面の判断で（[ADR 0039]）、ここはその文字列を
/// OS に登録できる形に戻すところです。**2 つが食い違うと、保存した割り当てを
/// 登録し直せません。**
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
fn key_code(key: &str) -> Option<Code> {
    let name = match key {
        "space" => "Space".to_string(),
        "enter" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "escape" => "Escape".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "insert" => "Insert".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "up" => "ArrowUp".to_string(),
        "down" => "ArrowDown".to_string(),
        "left" => "ArrowLeft".to_string(),
        "right" => "ArrowRight".to_string(),
        _ => {
            let mut chars = key.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if c.is_ascii_alphabetic() => {
                    format!("Key{}", c.to_ascii_uppercase())
                }
                (Some(c), None) if c.is_ascii_digit() => format!("Digit{c}"),
                (Some('f'), Some(d)) if d.is_ascii_digit() => {
                    let number = &key[1..];
                    if number.chars().all(|c| c.is_ascii_digit()) {
                        format!("F{number}")
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
    };
    Code::from_str(&name).ok()
}

/// この環境でグローバルホットキーを使えるか。使えないときは理由を返す。
///
/// **登録の戻り値を信じずに、環境そのものを先に見ます**——X11 の実装は、
/// 使えない環境でも成功したように見えるためです（`docs/DESIGN.md`
/// 「クイックキャプチャ」）。
pub fn platform_support() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Ok(())
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        fn env(key: &str) -> Option<String> {
            std::env::var(key).ok().filter(|value| !value.is_empty())
        }
        x11_support(
            env("WAYLAND_DISPLAY").as_deref(),
            env("XDG_SESSION_TYPE").as_deref(),
            env("DISPLAY").as_deref(),
        )
    }

    // ブラウザ。**アプリの外まで届くキーの割り当ては、ページには作れません。**
    #[cfg(target_family = "wasm")]
    {
        Err("ブラウザでは使えません".to_string())
    }

    #[cfg(not(any(
        target_family = "wasm",
        target_os = "macos",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    {
        Err("この OS はまだ対象外です".to_string())
    }
}

/// Linux / BSD での判定。環境変数を引数に取り、テストできるようにしてある。
///
/// Wayland にはアプリから使えるグローバルホットキーの共通の仕組みが無い。
/// XWayland 越しに登録しても、Wayland のクライアントが前面にいる間はイベントが
/// 来ないので、使えるとは言えない。
// macOS とブラウザでは `platform_support` が環境変数を見ないので、ここは
// テストからしか呼ばれない。
#[cfg_attr(any(target_os = "macos", target_family = "wasm"), allow(dead_code))]
fn x11_support(
    wayland_display: Option<&str>,
    session_type: Option<&str>,
    x11_display: Option<&str>,
) -> Result<(), String> {
    if wayland_display.is_some()
        || session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
    {
        return Err(
            "Wayland ではグローバルホットキーを使えません。X11 のセッションで起動してください"
                .to_string(),
        );
    }
    if x11_display.is_none() {
        return Err("X11 のディスプレイが見つかりません".to_string());
    }
    Ok(())
}

#[cfg(all(test, feature = "shell"))]
mod shortcut_tests {
    use super::*;

    fn shortcut(source: &str) -> Shortcut {
        Shortcut::parse(source).expect("the shortcut parses")
    }

    /// 保存の形が変わっていないこと。**ここが変わると、既にあるデータベースの
    /// 割り当てが読めなくなります。**
    #[test]
    fn round_trips_a_shortcut_through_its_stored_form() {
        for source in [
            "ctrl-alt-shift-cmd-n",
            "cmd-n",
            "ctrl-shift-f1",
            "alt-space",
            "ctrl-left",
        ] {
            let parsed = shortcut(source);
            assert_eq!(parsed.to_string(), source);
            assert_eq!(shortcut(&parsed.to_string()), parsed);
        }
    }

    #[test]
    fn normalizes_the_order_of_the_modifiers() {
        assert_eq!(shortcut("cmd-shift-n"), shortcut("shift-cmd-n"));
        assert_eq!(shortcut("shift-cmd-n").to_string(), "shift-cmd-n");
    }

    /// 修飾キーの無い割り当ては、ほかのアプリでそのキーを奪う。
    ///
    /// **画面も同じことを断ります**（`web/src/shell/shortcut.ts`）。断るのは
    /// そちらが先で、ここは保存されている文字列を読み直す側の砦です。
    #[test]
    fn rejects_a_shortcut_without_a_modifier() {
        assert_eq!(Shortcut::parse("n"), Err(ShortcutError::NoModifier));
    }

    #[test]
    fn rejects_something_that_is_not_a_shortcut() {
        assert!(matches!(
            Shortcut::parse("hyper-n"),
            Err(ShortcutError::Unparsable(_))
        ));
        assert!(matches!(
            Shortcut::parse(""),
            Err(ShortcutError::Unparsable(_))
        ));
    }

    #[test]
    fn rejects_a_key_that_cannot_be_registered() {
        assert!(matches!(
            Shortcut::parse("ctrl-§"),
            Err(ShortcutError::UnsupportedKey(_))
        ));
    }

    /// 画面が組み立てた文字列を、そのまま読めること。
    ///
    /// **押されたキーを文字列にするのは画面**です（`web/src/shell/shortcut.ts`
    /// の `readKeyPress`）。ここが見るのは、その綴りを登録できる形に戻せること
    /// ——2 つの表が食い違うと、保存した割り当てが効かなくなります。
    #[test]
    fn reads_every_key_the_screen_can_hand_over() {
        for source in ["ctrl-n", "ctrl-7", "ctrl-f12", "ctrl-left", "ctrl-space"] {
            assert_eq!(shortcut(source).to_string(), source);
        }
    }

    /// 登録に渡す形が、修飾キーごとに変わること。
    #[test]
    fn carries_every_modifier_into_the_registration() {
        assert_eq!(
            shortcut("ctrl-alt-shift-cmd-n").to_global(),
            GlobalShortcut::new(
                Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT | Modifiers::SUPER),
                Code::KeyN
            )
        );
    }
}

/// 環境の判定は、殻を外しても残ります——メニューが「使えない理由」を文言に
/// 入れるのに使うためです（`menu::quick_capture_item`）。
#[cfg(test)]
mod platform_tests {
    use super::x11_support;

    #[test]
    fn wayland_has_no_global_hotkeys() {
        assert!(x11_support(Some("wayland-0"), None, Some(":0")).is_err());
        assert!(x11_support(None, Some("wayland"), Some(":0")).is_err());
        assert!(x11_support(None, Some("Wayland"), Some(":0")).is_err());
    }

    #[test]
    fn an_x11_display_is_enough() {
        assert_eq!(x11_support(None, Some("x11"), Some(":0")), Ok(()));
        assert_eq!(x11_support(None, None, Some(":0")), Ok(()));
    }

    #[test]
    fn no_display_means_no_global_hotkeys() {
        assert!(x11_support(None, None, None).is_err());
    }
}
