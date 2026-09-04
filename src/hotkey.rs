//! クイックキャプチャのグローバルホットキー。
//!
//! 割り当ては gpui のキーストローク表記（`ctrl-alt-shift-cmd-n` の順）で
//! `app_state` に持つ。`Keystroke` の `Display` は `⌘N` のような画面表示用で
//! プラットフォームごとに変わるため、保存には使わない。

use std::fmt;
use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey, Modifiers as HotKeyModifiers};
use global_hotkey::GlobalHotKeyManager;
use gpui_kit::{Global, Keystroke, Modifiers};

/// 割り当てを受け付けられない理由。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ShortcutError {
    #[error("ショートカットとして読み取れません: {0}")]
    Unparsable(String),
    #[error("修飾キーを 1 つ以上含めてください。修飾キーなしの割り当ては、ほかのアプリでそのキーが打てなくなります")]
    NoModifier,
    #[error("fn キーはグローバルホットキーに使えません")]
    FunctionModifier,
    #[error("このキーはグローバルホットキーに使えません: {0}")]
    UnsupportedKey(String),
}

/// クイックキャプチャに割り当てられたキーの組み合わせ。
///
/// 生成できた時点で、グローバルホットキーとして登録できる形だと分かっている。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    keystroke: Keystroke,
    hotkey: HotKey,
}

impl Shortcut {
    /// 押されたキーストロークから作る。受け付けられない組み合わせは弾く。
    pub fn from_keystroke(keystroke: &Keystroke) -> Result<Self, ShortcutError> {
        let modifiers = keystroke.modifiers;
        if modifiers.function {
            return Err(ShortcutError::FunctionModifier);
        }
        if !modifiers.control && !modifiers.alt && !modifiers.shift && !modifiers.platform {
            return Err(ShortcutError::NoModifier);
        }

        let code = key_code(&keystroke.key)
            .ok_or_else(|| ShortcutError::UnsupportedKey(keystroke.key.clone()))?;
        let hotkey = HotKey::new(Some(hotkey_modifiers(&modifiers)), code);

        Ok(Self {
            keystroke: Keystroke {
                modifiers,
                key: keystroke.key.clone(),
                key_char: None,
            },
            hotkey,
        })
    }

    /// 保存してある文字列から復元する。
    pub fn parse(source: &str) -> Result<Self, ShortcutError> {
        let keystroke =
            Keystroke::parse(source).map_err(|_| ShortcutError::Unparsable(source.to_string()))?;
        Self::from_keystroke(&keystroke)
    }

    /// 登録に渡すホットキー。
    pub fn hotkey(&self) -> HotKey {
        self.hotkey
    }

    /// このホットキーのイベント ID。
    pub fn id(&self) -> u32 {
        self.hotkey.id()
    }
}

impl fmt::Display for Shortcut {
    /// 保存と表示に使う正規形。修飾キーの順序を固定するので、`cmd-shift-n` と
    /// `shift-cmd-n` は同じ文字列になる。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifiers = &self.keystroke.modifiers;
        if modifiers.control {
            f.write_str("ctrl-")?;
        }
        if modifiers.alt {
            f.write_str("alt-")?;
        }
        if modifiers.shift {
            f.write_str("shift-")?;
        }
        if modifiers.platform {
            f.write_str("cmd-")?;
        }
        f.write_str(&self.keystroke.key)
    }
}

fn hotkey_modifiers(modifiers: &Modifiers) -> HotKeyModifiers {
    let mut mods = HotKeyModifiers::empty();
    if modifiers.control {
        mods |= HotKeyModifiers::CONTROL;
    }
    if modifiers.alt {
        mods |= HotKeyModifiers::ALT;
    }
    if modifiers.shift {
        mods |= HotKeyModifiers::SHIFT;
    }
    if modifiers.platform {
        // macOS では Cmd、X11 では Mod4（Super）に落ちる。gpui の platform と同じ。
        mods |= HotKeyModifiers::SUPER;
    }
    mods
}

/// gpui のキー名を W3C の `code` に移す。
///
/// 対応しないキーで `None` を返し、呼び出し側がエラーにする。取りこぼしを黙って
/// 別のキーに丸めない。
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

/// アプリ全体で 1 つだけ持つホットキーの登録状態。
///
/// `GlobalHotKeyManager` はメインスレッドで作り、ここに置いたまま動かさない。
pub struct QuickCapture {
    manager: Option<GlobalHotKeyManager>,
    registered: Option<Shortcut>,
    /// マネージャを作れなかった理由。作れていれば `None`。
    unavailable: Option<String>,
}

impl Global for QuickCapture {}

impl QuickCapture {
    /// マネージャの生成を試みる。失敗しても起動は続ける。
    pub fn new() -> Self {
        match GlobalHotKeyManager::new() {
            Ok(manager) => Self {
                manager: Some(manager),
                registered: None,
                unavailable: None,
            },
            Err(error) => Self {
                manager: None,
                registered: None,
                unavailable: Some(error.to_string()),
            },
        }
    }

    /// この環境でグローバルホットキーを使えるか。
    pub fn unavailable_reason(&self) -> Option<&str> {
        self.unavailable.as_deref()
    }

    /// 今登録されている割り当て。
    pub fn registered(&self) -> Option<&Shortcut> {
        self.registered.as_ref()
    }

    /// 割り当てを差し替える。`None` で解除する。
    ///
    /// 失敗したときは以前の割り当てを残したまま `Err` を返す。登録できていない
    /// のに設定だけ変わっている状態を作らない。
    pub fn set(&mut self, shortcut: Option<Shortcut>) -> Result<(), String> {
        let Some(manager) = self.manager.as_ref() else {
            return Err(self
                .unavailable
                .clone()
                .unwrap_or_else(|| "グローバルホットキーを使えません".to_string()));
        };

        if self.registered.as_ref() == shortcut.as_ref() {
            return Ok(());
        }

        if let Some(next) = shortcut.as_ref() {
            manager
                .register(next.hotkey())
                .map_err(|error| format!("「{next}」を登録できませんでした: {error}"))?;
        }
        if let Some(previous) = self.registered.take() {
            // 新しい割り当ての登録に成功してから外す。失敗したときに何も
            // 効かない状態にしないため。
            let _ = manager.unregister(previous.hotkey());
        }
        self.registered = shortcut;
        Ok(())
    }
}

impl Default for QuickCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{key_code, Shortcut, ShortcutError};
    use global_hotkey::hotkey::Code;
    use gpui_kit::Keystroke;

    fn shortcut(source: &str) -> Shortcut {
        Shortcut::parse(source).expect("should parse")
    }

    #[test]
    fn round_trips_a_shortcut_through_its_stored_form() {
        for source in [
            "ctrl-alt-shift-cmd-n",
            "cmd-n",
            "ctrl-shift-f1",
            "alt-space",
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
    }

    #[test]
    fn rejects_the_function_modifier() {
        assert_eq!(
            Shortcut::parse("fn-cmd-n"),
            Err(ShortcutError::FunctionModifier)
        );
    }

    #[test]
    fn rejects_a_key_that_cannot_be_registered() {
        let mut keystroke = Keystroke::parse("cmd-n").expect("should parse");
        keystroke.key = "§".to_string();
        assert_eq!(
            Shortcut::from_keystroke(&keystroke),
            Err(ShortcutError::UnsupportedKey("§".to_string()))
        );
    }

    #[test]
    fn maps_gpui_key_names_to_codes() {
        assert_eq!(key_code("n"), Some(Code::KeyN));
        assert_eq!(key_code("7"), Some(Code::Digit7));
        assert_eq!(key_code("f12"), Some(Code::F12));
        assert_eq!(key_code("left"), Some(Code::ArrowLeft));
        assert_eq!(key_code("space"), Some(Code::Space));
        assert_eq!(key_code("§"), None);
    }
}
