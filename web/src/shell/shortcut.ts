// 押されたキーを、割り当ての文字列にする（[ADR 0030]、[ADR 0039]）。
//
// **これは画面の判断です。** どのキーを押したのか、その組み合わせを受け付けて
// よいのか——ウェブアプリならブラウザ側に書くだろうものです。OS に登録する
// ところだけが殻の仕事で、そちらは `crates/app/src/shortcut.rs` が
// この文字列を読んで行います。
//
// **綴りは変えません**（`docs/DESIGN.md`「クイックキャプチャ」）。`app_state`
// に入っている `ctrl-shift-n` の形をそのまま書きます。形を変えれば移行が要り、
// 読めなかった割り当ては黙って消えます。
//
// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

/// 画面が受け取った押しかた。`KeyboardEvent` の modifiers と `code`。
export interface KeyPress {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  /// macOS の Cmd、ほかの OS の Super。`KeyboardEvent.metaKey`。
  meta: boolean;
  /// `KeyboardEvent.code`。**`key` ではありません**——`key` は修飾キーと配列で
  /// 変わるので、同じ物理キーが別の名前で届きます。
  code: string;
}

/// 読めたか、断ったか。**断りは例外にしません**——ダイアログの中に出すもので、
/// 打ち直せば直ります（ADR 0016）。
export type ShortcutRead = { ok: true; shortcut: string } | { ok: false; reason: string };

/// `KeyboardEvent.code` を、保存する側のキー名に直す。
///
/// 対応しないものは `null` を返し、呼ぶ側が断ります。**取りこぼしを黙って別の
/// キーに丸めません**——別のキーが登録されると、押しても開かない割り当てが
/// 残ります。
///
/// 逆向きの表は `crates/app/src/shortcut.rs` の `key_code` にあります。保存した
/// 割り当てを登録し直すのがそちらなので、**2 つが食い違うと登録できません**。
export function keyName(code: string): string | null {
  const named: Record<string, string> = {
    Space: "space",
    Enter: "enter",
    Tab: "tab",
    Escape: "escape",
    Backspace: "backspace",
    Delete: "delete",
    Insert: "insert",
    Home: "home",
    End: "end",
    PageUp: "pageup",
    PageDown: "pagedown",
    ArrowUp: "up",
    ArrowDown: "down",
    ArrowLeft: "left",
    ArrowRight: "right",
  };
  const found = named[code];
  if (found !== undefined) return found;

  const letter = /^Key([A-Za-z])$/.exec(code);
  if (letter?.[1] !== undefined) return letter[1].toLowerCase();
  const digit = /^Digit([0-9])$/.exec(code);
  if (digit?.[1] !== undefined) return digit[1];
  const fn = /^F([0-9]+)$/.exec(code);
  if (fn?.[1] !== undefined) return `f${fn[1]}`;
  return null;
}

/// 押されたキーから、保存する形を作る。
///
/// **修飾キーの順序は固定**します。`cmd-shift-n` と `shift-cmd-n` が同じ
/// 文字列になるように。
export function readKeyPress(press: KeyPress): ShortcutRead {
  // 修飾キーなしの割り当ては、ほかのアプリでそのキーを奪う。
  if (!(press.ctrl || press.alt || press.shift || press.meta)) {
    return { ok: false, reason: "修飾キーと組み合わせてください（Ctrl、Alt、Shift、Cmd）" };
  }
  const key = keyName(press.code);
  if (key === null) {
    return { ok: false, reason: `${press.code} は割り当てに使えません` };
  }
  const parts = [
    press.ctrl ? "ctrl" : null,
    press.alt ? "alt" : null,
    press.shift ? "shift" : null,
    press.meta ? "cmd" : null,
    key,
  ].filter((part): part is string => part !== null);
  return { ok: true, shortcut: parts.join("-") };
}
