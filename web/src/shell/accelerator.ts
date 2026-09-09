// メニューに付いているキーの割り当てを、ページの側で受ける（[ADR 0035]）。
//
// **配るアプリでは、ここは動きません。** キーを取るのは OS のメニューバーで、
// アクセラレータは Rust が付けています（`crates/app/src/menu.rs`）。ブラウザには
// そのメニューバーが無いので、同じ割り当てをページが引き受けます。
//
// 読む文字列は muda の書き方（`"CmdOrCtrl+Shift+B"`）のままです。**書き方を
// 2 つにしません**——メニューの構成は Rust が持っており、ブラウザ用に書き直すと、
// 割り当てを直したときに片方だけ変わります。
//
// **ブラウザが自分で取ってしまう組み合わせがあります。** `Ctrl+N`、`Ctrl+T`、
// `Ctrl+W` は、多くのブラウザでページに届く前にブラウザの操作になります。
// 止める手立てはないので、その項目はメニューから押してもらいます。
//
// [ADR 0035]: ../../docs/adr/0035-a-browser-build-of-the-real-core.md

import type { Platform } from "../ipc/types/Platform";
import { isComposing } from "./ime";

interface Accelerator {
  /** `KeyboardEvent.code`。`"KeyN"`、`"Digit7"`、`"F11"`。 */
  code: string;
  /** macOS では Cmd、ほかでは Ctrl。`CmdOrCtrl` がこれ。 */
  secondary: boolean;
  ctrl: boolean;
  meta: boolean;
  shift: boolean;
  alt: boolean;
}

/// muda の書き方を読む。読めなければ `null`。
///
/// 読めないものを黙って別のキーに丸めません。丸めると、**メニューに書いてある
/// のと違うキーが効く**という、いちばん分かりにくい壊れ方をします。
export function parseAccelerator(accelerator: string): Accelerator | null {
  const parts = accelerator.split("+").filter((part) => part !== "");
  const key = parts.pop();
  if (key === undefined) return null;

  const parsed: Accelerator = {
    code: null as unknown as string,
    secondary: false,
    ctrl: false,
    meta: false,
    shift: false,
    alt: false,
  };
  for (const part of parts) {
    switch (part.toLowerCase()) {
      case "cmdorctrl":
      case "commandorcontrol":
        parsed.secondary = true;
        break;
      case "cmd":
      case "command":
      case "super":
      case "meta":
        parsed.meta = true;
        break;
      case "ctrl":
      case "control":
        parsed.ctrl = true;
        break;
      case "shift":
        parsed.shift = true;
        break;
      case "alt":
      case "option":
        parsed.alt = true;
        break;
      default:
        return null;
    }
  }

  const code = codeOf(key);
  if (code === null) return null;
  return { ...parsed, code };
}

/// 割り当てのキー名を `KeyboardEvent.code` に直す。
///
/// `code` で見るのは、**押された物理キーが配列と修飾キーで変わらない**ためです
/// （`crates/app/src/shortcut.rs` の `KeyPress` と同じ考え方）。`Shift+B` の
/// `event.key` は `"B"`、`Shift` 無しなら `"b"` で、そこで分岐を書きはじめると
/// 配列の違いに追われます。
function codeOf(key: string): string | null {
  if (/^[A-Za-z]$/.test(key)) return `Key${key.toUpperCase()}`;
  if (/^[0-9]$/.test(key)) return `Digit${key}`;
  if (/^F([1-9]|1[0-9]|2[0-4])$/i.test(key)) return key.toUpperCase();
  return PUNCTUATION.get(key) ?? null;
}

/// 英数字でもファンクションキーでもない、割り当てに使うキー。
///
/// `,` は「設定…」の `CmdOrCtrl+,` です（ADR 0047）。muda 側は `,` も `Comma` も
/// 同じ `Code::Comma` に読むので、**書くほうを 1 つに決めます**——`shell/menu.ts`
/// には `,` と書き、その形をここでも読みます。増やすときは、必ず muda が
/// 読める綴りであることを確かめること（読めない文字列はメニューを組む時点で
/// `Err` になり、メニューが掛からないまま終わります）。
const PUNCTUATION = new Map<string, string>([[",", "Comma"]]);

/// 割り当てのキーを、メニューに出す形に戻す。`codeOf` の逆。
function keyOf(code: string): string {
  for (const [key, mapped] of PUNCTUATION) if (mapped === code) return key;
  return code.replace(/^Key|^Digit/, "");
}

/// 押されたキーが、この割り当てかどうか。
///
/// 書いてある修飾キーが全部押されていて、書いていない修飾キーが 1 つも押されて
/// いないときだけ一致とします。**余分な修飾キーを見逃すと**、`Cmd+Shift+F` の
/// つもりの打鍵が `Cmd+F` にも当たります。
export function matchesAccelerator(
  event: KeyboardEvent,
  accelerator: string,
  platform: Platform,
): boolean {
  if (isComposing(event)) return false;
  const wanted = parseAccelerator(accelerator);
  if (wanted === null) return false;
  if (event.code !== wanted.code) return false;

  const isMac = platform === "macos";
  const meta = wanted.meta || (wanted.secondary && isMac);
  const ctrl = wanted.ctrl || (wanted.secondary && !isMac);
  return (
    event.metaKey === meta &&
    event.ctrlKey === ctrl &&
    event.shiftKey === wanted.shift &&
    event.altKey === wanted.alt
  );
}

/// メニューに出す形。macOS は記号、ほかは `Ctrl+Shift+B`。
export function formatAccelerator(accelerator: string, platform: Platform): string {
  const parsed = parseAccelerator(accelerator);
  if (parsed === null) return accelerator;
  const isMac = platform === "macos";
  const key = keyOf(parsed.code);

  if (isMac) {
    // macOS の並び順は Ctrl → Alt → Shift → Cmd で固定です。
    return `${parsed.ctrl ? "⌃" : ""}${parsed.alt ? "⌥" : ""}${parsed.shift ? "⇧" : ""}${
      parsed.meta || parsed.secondary ? "⌘" : ""
    }${key}`;
  }
  const parts: string[] = [];
  if (parsed.ctrl || parsed.secondary) parts.push("Ctrl");
  if (parsed.meta) parts.push("Super");
  if (parsed.alt) parts.push("Alt");
  if (parsed.shift) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}
