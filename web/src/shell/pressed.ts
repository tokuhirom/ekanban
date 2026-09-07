// 押されているキーを、その場に映すための組み立て（`docs/DESIGN.md`「クイックキャプチャ」、[ADR 0030]）。
//
// **表記だけを持ちます。** どの組み合わせを受け付けるかも、保存の形も Rust の
// `shortcut.rs` が決めていて、ここはその両方を人が読む形に直すだけです。判定を
// こちらにも書くと、受け付けられる組み合わせが 2 か所に分かれます。
//
// 修飾キーの並び順は、保存の形（`ctrl-alt-shift-cmd-n`）と同じにしてあります。
// macOS で慣用の ⌃⌥⇧⌘ と同じ順なので、どちらの OS でも並べ替えは要りません。
//
// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md

import type { Platform } from "../ipc/types/Platform";

/** 修飾キーだけを押している途中は、まだ組み合わせが決まっていない。 */
export const MODIFIER_CODES = /^(Control|Alt|Shift|Meta)(Left|Right)$/;

/** いま押されているもの。 */
export interface Held {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  /** 修飾キー以外で押されているキーの `code`。まだ無ければ `null`。 */
  code: string | null;
}

export const NOTHING_HELD: Held = {
  ctrl: false,
  alt: false,
  shift: false,
  meta: false,
  code: null,
};

/** `keydown` や `keyup` から見た、押されているキーの読み取り元。 */
interface KeyState {
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  code: string;
}

export function isHolding(held: Held): boolean {
  return held.ctrl || held.alt || held.shift || held.meta || held.code !== null;
}

/// 押された瞬間の状態。
export function heldOnKeyDown(event: KeyState): Held {
  return {
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
    code: MODIFIER_CODES.test(event.code) ? null : event.code,
  };
}

/// 離した瞬間の状態。
///
/// **修飾キーは離したあとのイベントの値をそのまま使います。** `keyup` の
/// `shiftKey` は、その `Shift` を離したあとの値なので、自分で数え直すより
/// 確かです。macOS では Cmd を押している間ほかのキーの `keyup` が届かない
/// ことがあり、数えていると押しっぱなしに見えます。
export function heldOnKeyUp(previous: Held, event: KeyState): Held {
  return {
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
    // 離したのが修飾キー以外なら、それが押されていたキー。
    code: MODIFIER_CODES.test(event.code) ? previous.code : null,
  };
}

/// 押されているものを、1 つずつ人が読む形にする。
export function describeHeld(held: Held, platform: Platform): string[] {
  const keys = modifierLabels(held, platform);
  if (held.code !== null) keys.push(codeLabel(held.code));
  return keys;
}

/// 保存されている形（`cmd-shift-n`）を、同じ書き方で並べる。
///
/// 読めない文字列はそのまま返します。**黙って別のキーに丸めません**——
/// 見えているものと登録されているものが食い違うと、押しても開かない理由が
/// 分からなくなります。
export function describeStored(stored: string, platform: Platform): string[] {
  const parts = stored.split("-");
  const key = parts.pop();
  if (key === undefined || key === "") return [stored];
  const held: Held = { ...NOTHING_HELD };
  for (const part of parts) {
    if (part === "ctrl") held.ctrl = true;
    else if (part === "alt") held.alt = true;
    else if (part === "shift") held.shift = true;
    else if (part === "cmd") held.meta = true;
    else return [stored];
  }
  return [...modifierLabels(held, platform), storedKeyLabel(key)];
}

function modifierLabels(held: Held, platform: Platform): string[] {
  const mac = platform === "macos";
  const keys: string[] = [];
  if (held.ctrl) keys.push(mac ? "⌃" : "Ctrl");
  if (held.alt) keys.push(mac ? "⌥" : "Alt");
  if (held.shift) keys.push(mac ? "⇧" : "Shift");
  if (held.meta) keys.push(mac ? "⌘" : platform === "windows" ? "Win" : "Super");
  return keys;
}

/** 名前のあるキーの見せ方。`code` 側と保存の形の側で綴りが違うだけ。 */
const NAMED_LABELS: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Escape: "Esc",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

const STORED_LABELS: Record<string, string> = {
  space: "Space",
  enter: "Enter",
  tab: "Tab",
  escape: "Esc",
  backspace: "Backspace",
  delete: "Delete",
  insert: "Insert",
  home: "Home",
  end: "End",
  pageup: "PageUp",
  pagedown: "PageDown",
  up: "↑",
  down: "↓",
  left: "←",
  right: "→",
};

/// `KeyboardEvent.code` の見せ方。
///
/// 割り当てに使えないキーもそのまま出します。**受け付けられるかどうかは
/// Rust が決める**ので、ここで先に消すと、押したのに何も出ない画面になります。
function codeLabel(code: string): string {
  const named = NAMED_LABELS[code];
  if (named !== undefined) return named;
  const single = /^(?:Key|Digit)([A-Z0-9])$/.exec(code);
  return single?.[1] ?? code;
}

function storedKeyLabel(key: string): string {
  return STORED_LABELS[key] ?? key.toUpperCase();
}
