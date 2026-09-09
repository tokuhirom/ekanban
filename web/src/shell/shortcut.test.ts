import { describe, expect, it } from "vitest";

import { readKeyPress } from "./shortcut";

function press(over: Partial<Parameters<typeof readKeyPress>[0]> = {}) {
  return { ctrl: false, alt: false, shift: false, meta: false, code: "KeyN", ...over };
}

describe("readKeyPress", () => {
  it("は修飾キーの順序を固定する", () => {
    expect(readKeyPress(press({ meta: true, shift: true, ctrl: true }))).toEqual({
      ok: true,
      shortcut: "ctrl-shift-cmd-n",
    });
  });

  it("は修飾キーの無い組み合わせを断る", () => {
    const read = readKeyPress(press());
    // 打ち直せるように、何が足りないかを言う。
    expect(read.ok ? "" : read.reason).toContain("修飾キー");
  });

  /// 取りこぼしを別のキーに丸めない。丸めると、押しても開かない割り当てが残る。
  it("は知らないキーを断る", () => {
    const read = readKeyPress(press({ ctrl: true, code: "MediaPlayPause" }));
    expect(read.ok ? "" : read.reason).toContain("MediaPlayPause");
  });

  it("は数字と機能キーと矢印を読む", () => {
    expect(readKeyPress(press({ ctrl: true, code: "Digit1" }))).toEqual({
      ok: true,
      shortcut: "ctrl-1",
    });
    expect(readKeyPress(press({ alt: true, code: "F12" }))).toEqual({
      ok: true,
      shortcut: "alt-f12",
    });
    expect(readKeyPress(press({ shift: true, code: "ArrowLeft" }))).toEqual({
      ok: true,
      shortcut: "shift-left",
    });
  });
});
