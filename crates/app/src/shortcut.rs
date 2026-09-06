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

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut as GlobalShortcut};
use ts_rs::TS;

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

/// 画面が押されたキーを渡す形。`KeyboardEvent` の modifiers と `code`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KeyPress {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// macOS の Cmd、ほかの OS の Super。`KeyboardEvent.metaKey`。
    pub meta: bool,
    /// `KeyboardEvent.code`。**`key` ではありません**——`key` は修飾キーと配列で
    /// 変わるので、同じ物理キーが別の名前で届きます。
    pub code: String,
}

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

impl Shortcut {
    /// 画面から届いた押しかたから作る。受け付けられない組み合わせは断る。
    pub fn from_key_press(press: &KeyPress) -> Result<Self, ShortcutError> {
        // 修飾キーなしの割り当ては、ほかのアプリでそのキーを奪う。
        if !(press.ctrl || press.alt || press.shift || press.meta) {
            return Err(ShortcutError::NoModifier);
        }
        let key = key_name(&press.code)
            .ok_or_else(|| ShortcutError::UnsupportedKey(press.code.clone()))?;
        let code = key_code(&key).ok_or_else(|| ShortcutError::UnsupportedKey(key.clone()))?;
        Ok(Self {
            ctrl: press.ctrl,
            alt: press.alt,
            shift: press.shift,
            meta: press.meta,
            key,
            code,
        })
    }

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

/// `KeyboardEvent.code` を、保存する側のキー名に直す。
///
/// 対応しないものは `None` を返し、呼ぶ側が断ります。**取りこぼしを黙って別の
/// キーに丸めません**——別のキーが登録されると、押しても開かない割り当てが
/// 残ります。
fn key_name(code: &str) -> Option<String> {
    let name = match code {
        "Space" => "space",
        "Enter" => "enter",
        "Tab" => "tab",
        "Escape" => "escape",
        "Backspace" => "backspace",
        "Delete" => "delete",
        "Insert" => "insert",
        "Home" => "home",
        "End" => "end",
        "PageUp" => "pageup",
        "PageDown" => "pagedown",
        "ArrowUp" => "up",
        "ArrowDown" => "down",
        "ArrowLeft" => "left",
        "ArrowRight" => "right",
        _ => {
            if let Some(letter) = code.strip_prefix("Key") {
                return single_ascii(letter).map(|c| c.to_ascii_lowercase().to_string());
            }
            if let Some(digit) = code.strip_prefix("Digit") {
                return single_ascii(digit).map(|c| c.to_string());
            }
            if let Some(number) = code.strip_prefix('F') {
                return (!number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
                    .then(|| format!("f{number}"));
            }
            return None;
        }
    };
    Some(name.to_string())
}

fn single_ascii(value: &str) -> Option<char> {
    let mut chars = value.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c),
        _ => None,
    }
}

/// 保存する側のキー名を W3C の `code` に直す。[`key_name`] の逆向きで、
/// 2 つの表が食い違うと保存した割り当てを登録し直せなくなる。
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

    #[cfg(not(any(
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
#[cfg_attr(target_os = "macos", allow(dead_code))]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(source: &str) -> Shortcut {
        Shortcut::parse(source).expect("the shortcut parses")
    }

    fn press(code: &str) -> KeyPress {
        KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            meta: false,
            code: code.to_string(),
        }
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

    #[test]
    fn rejects_a_shortcut_without_a_modifier() {
        assert_eq!(Shortcut::parse("n"), Err(ShortcutError::NoModifier));
        assert_eq!(
            Shortcut::from_key_press(&KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                meta: false,
                code: "KeyN".to_string(),
            }),
            Err(ShortcutError::NoModifier)
        );
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
            Shortcut::from_key_press(&press("IntlBackslash")),
            Err(ShortcutError::UnsupportedKey(_))
        ));
        assert!(matches!(
            Shortcut::parse("ctrl-§"),
            Err(ShortcutError::UnsupportedKey(_))
        ));
    }

    /// 画面から届くのは `code`。修飾キーと配列で変わる `key` は見ない。
    #[test]
    fn reads_the_physical_key_the_browser_reports() {
        assert_eq!(
            Shortcut::from_key_press(&press("KeyN")).expect("KeyN is a key"),
            shortcut("ctrl-n")
        );
        assert_eq!(
            Shortcut::from_key_press(&press("Digit7")).expect("Digit7 is a key"),
            shortcut("ctrl-7")
        );
        assert_eq!(
            Shortcut::from_key_press(&press("F12")).expect("F12 is a key"),
            shortcut("ctrl-f12")
        );
        assert_eq!(
            Shortcut::from_key_press(&press("ArrowLeft")).expect("ArrowLeft is a key"),
            shortcut("ctrl-left")
        );
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
