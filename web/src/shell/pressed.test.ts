import { describe, expect, it } from "vitest";

import {
  describeHeld,
  describeStored,
  heldOnKeyDown,
  heldOnKeyUp,
  isHolding,
  NOTHING_HELD,
} from "./pressed";

function event(code: string, modifiers: Partial<KeyboardEventInit> = {}) {
  return {
    code,
    ctrlKey: modifiers.ctrlKey ?? false,
    altKey: modifiers.altKey ?? false,
    shiftKey: modifiers.shiftKey ?? false,
    metaKey: modifiers.metaKey ?? false,
  };
}

describe("押されているキーの読み取り", () => {
  it("修飾キーだけを押している途中も、押されているものとして見える", () => {
    const held = heldOnKeyDown(event("MetaLeft", { metaKey: true }));
    expect(held).toEqual({ ctrl: false, alt: false, shift: false, meta: true, code: null });
    expect(isHolding(held)).toBe(true);
  });

  it("修飾キーとキーが揃うと、両方が並ぶ", () => {
    const held = heldOnKeyDown(event("KeyK", { metaKey: true, shiftKey: true }));
    expect(describeHeld(held, "macos")).toEqual(["⇧", "⌘", "K"]);
    expect(describeHeld(held, "linux")).toEqual(["Shift", "Super", "K"]);
  });

  it("何も押していなければ、押されていないと分かる", () => {
    expect(isHolding(NOTHING_HELD)).toBe(false);
    expect(describeHeld(NOTHING_HELD, "macos")).toEqual([]);
  });

  it("離した修飾キーは、離したあとのイベントの値をそのまま使う", () => {
    const down = heldOnKeyDown(event("KeyK", { ctrlKey: true, altKey: true }));
    const up = heldOnKeyUp(down, event("AltLeft", { ctrlKey: true }));
    expect(up).toEqual({ ctrl: true, alt: false, shift: false, meta: false, code: "KeyK" });
  });

  it("修飾キー以外を離すと、そのキーが落ちる", () => {
    const down = heldOnKeyDown(event("KeyK", { ctrlKey: true }));
    const up = heldOnKeyUp(down, event("KeyK", { ctrlKey: true }));
    expect(up.code).toBeNull();
    expect(isHolding(up)).toBe(true);
  });
});

describe("キーの見せ方", () => {
  it("名前のあるキーは名前で出す", () => {
    const named: [string, string][] = [
      ["Space", "Space"],
      ["ArrowLeft", "←"],
      ["Escape", "Esc"],
      ["F12", "F12"],
      ["Digit7", "7"],
    ];
    for (const [code, label] of named) {
      expect(describeHeld(heldOnKeyDown(event(code, { ctrlKey: true })), "linux")).toEqual([
        "Ctrl",
        label,
      ]);
    }
  });

  it("割り当てに使えないキーも、押されたことは見せる", () => {
    // 受け付けるかどうかは Rust が決める。ここで先に消すと、押したのに
    // 何も出ない画面になる。
    expect(describeHeld(heldOnKeyDown(event("IntlBackslash", { ctrlKey: true })), "linux")).toEqual([
      "Ctrl",
      "IntlBackslash",
    ]);
  });
});

describe("保存されている形の見せ方", () => {
  it("保存の形を、押しているときと同じ書き方で並べる", () => {
    expect(describeStored("ctrl-alt-shift-cmd-n", "macos")).toEqual(["⌃", "⌥", "⇧", "⌘", "N"]);
    expect(describeStored("ctrl-alt-shift-cmd-n", "windows")).toEqual([
      "Ctrl",
      "Alt",
      "Shift",
      "Win",
      "N",
    ]);
    expect(describeStored("ctrl-left", "linux")).toEqual(["Ctrl", "←"]);
    expect(describeStored("shift-cmd-f12", "macos")).toEqual(["⇧", "⌘", "F12"]);
  });

  it("読めない文字列は、そのまま出す", () => {
    // 別のキーに丸めると、見えているものと登録されているものが食い違う。
    expect(describeStored("hyper-n", "macos")).toEqual(["hyper-n"]);
    expect(describeStored("", "macos")).toEqual([""]);
  });
});
