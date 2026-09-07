import { describe, expect, it } from "vitest";

import { formatAccelerator, matchesAccelerator, parseAccelerator } from "./accelerator";

type Flags = "ctrlKey" | "metaKey" | "shiftKey" | "altKey" | "isComposing";

/// `KeyboardEvent` の代わり。node 環境なので、見ている形だけ作ります。
function press(code: string, flags: Partial<Record<Flags, boolean>> = {}): KeyboardEvent {
  return {
    code,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    isComposing: false,
    keyCode: 0,
    ...flags,
  } as KeyboardEvent;
}

describe("parseAccelerator", () => {
  it("メニューに出てくる書き方をすべて読める", () => {
    expect(parseAccelerator("CmdOrCtrl+N")).toEqual({
      code: "KeyN",
      secondary: true,
      ctrl: false,
      meta: false,
      shift: false,
      alt: false,
    });
    expect(parseAccelerator("Cmd+Ctrl+S")?.code).toBe("KeyS");
    expect(parseAccelerator("F11")?.code).toBe("F11");
  });

  it("読めないものは丸めずに断る", () => {
    // 丸めると、メニューに書いてあるのと違うキーが効く。
    expect(parseAccelerator("Hyper+N")).toBeNull();
    expect(parseAccelerator("CmdOrCtrl+§")).toBeNull();
    expect(parseAccelerator("")).toBeNull();
  });
});

describe("matchesAccelerator", () => {
  it("secondary は macOS だけ Cmd", () => {
    expect(matchesAccelerator(press("KeyN", { metaKey: true }), "CmdOrCtrl+N", "macos")).toBe(true);
    expect(matchesAccelerator(press("KeyN", { ctrlKey: true }), "CmdOrCtrl+N", "macos")).toBe(false);
    expect(matchesAccelerator(press("KeyN", { ctrlKey: true }), "CmdOrCtrl+N", "linux")).toBe(true);
    expect(matchesAccelerator(press("KeyN", { metaKey: true }), "CmdOrCtrl+N", "linux")).toBe(false);
  });

  it("余分な修飾キーが付いていたら一致しない", () => {
    // これを見逃すと `Cmd+Shift+F` が `Cmd+F` にも当たる。
    expect(
      matchesAccelerator(press("KeyF", { metaKey: true, shiftKey: true }), "CmdOrCtrl+F", "macos"),
    ).toBe(false);
    expect(
      matchesAccelerator(
        press("KeyF", { metaKey: true, shiftKey: true }),
        "CmdOrCtrl+Shift+F",
        "macos",
      ),
    ).toBe(true);
  });

  it("変換中は何にも当たらない", () => {
    const composing = press("KeyN", { metaKey: true, isComposing: true });
    expect(matchesAccelerator(composing, "CmdOrCtrl+N", "macos")).toBe(false);
  });

  it("押されたのが同じ物理キーかどうかで見る", () => {
    expect(matchesAccelerator(press("Digit7", { ctrlKey: true }), "CmdOrCtrl+7", "linux")).toBe(
      true,
    );
    expect(matchesAccelerator(press("KeyM", { ctrlKey: true }), "CmdOrCtrl+N", "linux")).toBe(false);
  });
});

describe("formatAccelerator", () => {
  it("OS ごとの書き方に直す", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+B", "macos")).toBe("⇧⌘B");
    expect(formatAccelerator("CmdOrCtrl+Shift+B", "linux")).toBe("Ctrl+Shift+B");
    expect(formatAccelerator("Cmd+Ctrl+S", "macos")).toBe("⌃⌘S");
  });

  it("読めないものはそのまま出す", () => {
    expect(formatAccelerator("Hyper+N", "linux")).toBe("Hyper+N");
  });
});
