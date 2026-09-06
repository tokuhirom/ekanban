// 取り消しのキーを、盤面と入力欄に振り分ける（`docs/DESIGN.md`「メニューとキー割り当て」）。
//
// **`Cmd/Ctrl+Z` にメニューのアクセラレータを付けていません。** 付けると、説明欄を
// 打っている最中の取り消しまで盤面の Undo になり、書いていた行がまとめて消えた
// ように見えます。キーはここで受けて、フォーカスの居場所で行き先を決めます。
//
// 入力欄の中では**何もしません**。既定の動きを止めなければ、webview が自分の
// 編集履歴で取り消します。gpui 版の `undo()` も、編集中は盤面を巻き戻さずに
// 戻っていました——振る舞いはそのままです。

import type { Platform } from "../ipc/types/Platform";

export type UndoKind = "undo" | "redo";
/** 取り消しの行き先。`field` は「webview に任せる」という意味。 */
export type UndoTarget = "field" | "board";

export interface UndoIntent {
  kind: UndoKind;
  target: UndoTarget;
}

/// 打たれたキーが取り消し・やり直しか。違えば `null`。
///
/// `secondary` は macOS では Cmd、ほかでは Ctrl。**どの OS かは Rust から
/// 受け取ります**（`StartupState.platform`）——`navigator.userAgent` は webview が
/// 書き換えられる文字列で、取り違えると割り当てが丸ごと効きません。
export function undoIntent(event: KeyboardEvent, platform: Platform): UndoIntent | null {
  if (event.isComposing) return null;
  if (event.key.toLowerCase() !== "z") return null;
  const isMac = platform === "macos";
  const secondary = isMac ? event.metaKey : event.ctrlKey;
  const other = isMac ? event.ctrlKey : event.metaKey;
  if (!secondary || other || event.altKey) return null;
  return {
    kind: event.shiftKey ? "redo" : "undo",
    target: targetOf(event.target),
  };
}

/// 取り消しの行き先。入力欄と、その中の編集できる場所は webview のもの。
///
/// メニューから押されたときはキーのイベントが無いので、`document.activeElement`
/// を渡します。同じ判断を 2 通り書かないための引数です。
///
/// 見るのは `instanceof` ではなく形です。要素の種類は文字列で分かるうえ、
/// `instanceof` は DOM が無いところ（Vitest の node 環境）では確かめられません。
export function targetOf(element: unknown): UndoTarget {
  if (element === null || typeof element !== "object") return "board";
  const node = element as Partial<HTMLElement>;
  if (node.isContentEditable === true) return "field";
  const tag = typeof node.tagName === "string" ? node.tagName.toLowerCase() : "";
  return tag === "input" || tag === "textarea" ? "field" : "board";
}
