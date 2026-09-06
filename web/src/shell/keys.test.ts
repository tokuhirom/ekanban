// 取り消しのキーの振り分け（`docs/DESIGN.md`「メニューとキー割り当て」、「テスト」の「部品」）。
//
// **入力中の `Cmd+Z` が盤面を巻き戻してはいけません。**
// 割り当てを持たせないだけでは足りず、キーを受けたときに行き先を選び分ける
// ところまでが揃ってはじめて満たせます。

import { describe, expect, it } from "vitest";

import { targetOf, undoIntent } from "./keys";

function press(init: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    key: "z",
    isComposing: false,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    target: null,
    ...init,
  } as KeyboardEvent;
}

/// 焦点のある要素の代わり。`targetOf` は形だけを見るので、これで足りる。
function element(tagName: string, isContentEditable = false): EventTarget {
  return { tagName, isContentEditable } as unknown as EventTarget;
}

describe("undoIntent", () => {
  it("macOS は Cmd+Z、ほかの OS は Ctrl+Z", () => {
    expect(undoIntent(press({ metaKey: true }), "macos")?.kind).toBe("undo");
    expect(undoIntent(press({ ctrlKey: true }), "linux")?.kind).toBe("undo");
    expect(undoIntent(press({ ctrlKey: true }), "windows")?.kind).toBe("undo");
    // 取り違えると割り当てが丸ごと効かない。UA ではなく Rust の platform で決める。
    expect(undoIntent(press({ ctrlKey: true }), "macos")).toBeNull();
    expect(undoIntent(press({ metaKey: true }), "linux")).toBeNull();
  });

  it("Shift を足すとやり直し", () => {
    expect(undoIntent(press({ ctrlKey: true, shiftKey: true }), "linux")?.kind).toBe("redo");
  });

  it("ほかの修飾キーが混ざっていたら、別の割り当てに譲る", () => {
    expect(undoIntent(press({ ctrlKey: true, altKey: true }), "linux")).toBeNull();
    expect(undoIntent(press({ ctrlKey: true, metaKey: true }), "linux")).toBeNull();
  });

  it("変換中は取らない", () => {
    expect(undoIntent(press({ ctrlKey: true, isComposing: true }), "linux")).toBeNull();
  });

  it("Z 以外は取らない", () => {
    expect(undoIntent(press({ key: "y", ctrlKey: true }), "linux")).toBeNull();
  });

  /// 打っている最中の取り消しは webview のもの。盤面を巻き戻さない。
  it("入力欄の中では行き先が field になる", () => {
    const inField = press({ ctrlKey: true, target: element("TEXTAREA") });
    expect(undoIntent(inField, "linux")?.target).toBe("field");
    const onBoard = press({ ctrlKey: true, target: element("DIV") });
    expect(undoIntent(onBoard, "linux")?.target).toBe("board");
  });
});

describe("targetOf", () => {
  it("入力欄と編集できる場所は webview のもの", () => {
    expect(targetOf(element("INPUT"))).toBe("field");
    expect(targetOf(element("TEXTAREA"))).toBe("field");
    expect(targetOf(element("DIV", true))).toBe("field");
  });

  it("それ以外は盤面のもの", () => {
    expect(targetOf(null)).toBe("board");
    expect(targetOf(element("BODY"))).toBe("board");
    expect(targetOf(element("BUTTON"))).toBe("board");
  });
});
