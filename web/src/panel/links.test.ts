// 説明の中のリンク（ADR 0002、§10 の「部品」）。
//
// **見つけ方はここにありません**——それは Rust の `description_links` で、ここが
// 確かめるのは、返ってきた位置をどう描き、どこで押されたら開くかです。

import { describe, expect, it } from "vitest";

import type { UrlSpan } from "../ipc/types/UrlSpan";
import { linkAt, opensLink, segments } from "./links";

function span(start: number, end: number, url: string): UrlSpan {
  return { start, end, url };
}

describe("segments", () => {
  it("本文をリンクとそれ以外に切り分ける", () => {
    const text = "詳しくは https://example.com/a を見てください";
    const start = text.indexOf("https://");
    const links = [span(start, start + "https://example.com/a".length, "https://example.com/a")];
    expect(segments(text, links)).toEqual([
      { text: "詳しくは ", url: null },
      { text: "https://example.com/a", url: "https://example.com/a" },
      { text: " を見てください", url: null },
    ]);
  });

  it("リンクが無ければ本文ひとつ", () => {
    expect(segments("ただの説明", [])).toEqual([{ text: "ただの説明", url: null }]);
  });

  it("空の本文からは何も出さない", () => {
    expect(segments("", [])).toEqual([]);
  });

  /// 打っている途中は、1 つ前の本文で見つけた位置が届くことがある。ずれた位置で
  /// 色を付けるより、その一片を捨てて色が付かないほうがよい。
  it("本文からはみ出した位置は捨てる", () => {
    expect(segments("短い", [span(0, 40, "https://example.com")])).toEqual([
      { text: "短い", url: null },
    ]);
  });
});

describe("linkAt", () => {
  const links = [span(4, 25, "https://example.com/a")];

  it("端も含めて当たる", () => {
    // `https` の `h` の上と、末尾の 1 文字の後ろでも開ける（gpui 版と同じ）。
    expect(linkAt(links, 4)?.url).toBe("https://example.com/a");
    expect(linkAt(links, 25)?.url).toBe("https://example.com/a");
    expect(linkAt(links, 12)?.url).toBe("https://example.com/a");
  });

  it("外なら当たらない", () => {
    expect(linkAt(links, 3)).toBeNull();
    expect(linkAt(links, 26)).toBeNull();
  });
});

describe("opensLink", () => {
  function click(init: Partial<MouseEvent>): MouseEvent {
    return { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...init } as MouseEvent;
  }

  it("macOS は Cmd、ほかは Ctrl", () => {
    expect(opensLink(click({ metaKey: true }), "macos")).toBe(true);
    expect(opensLink(click({ ctrlKey: true }), "linux")).toBe(true);
    expect(opensLink(click({ ctrlKey: true }), "windows")).toBe(true);
    expect(opensLink(click({ ctrlKey: true }), "macos")).toBe(false);
  });

  /// 修飾キー無しのクリックは、文章のどこかを指すためのもの（ADR 0002）。
  it("修飾キーが無ければ開かない", () => {
    expect(opensLink(click({}), "linux")).toBe(false);
    expect(opensLink(click({ shiftKey: true }), "linux")).toBe(false);
    expect(opensLink(click({ ctrlKey: true, altKey: true }), "linux")).toBe(false);
  });
});
