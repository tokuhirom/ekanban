// 説明の中の URL を見つける規則のテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// **拾う規則はここにあります**（#129、ADR 0033）。Markdown のエディタでは
// 「どこがリンクか」が編集器の中のノードなので、Rust から位置を受け取る形には
// 戻せません。以前 Rust の `find_urls` が守っていた振る舞いを、そのままここで
// 書き下しています。

import { describe, expect, it } from "vitest";

import { opensLink, urlAround } from "./links";

function found(text: string): string | null {
  const span = urlAround(text);
  return span === null ? null : text.slice(span.start, span.end);
}

describe("urlAround", () => {
  it("http と https だけを拾う", () => {
    expect(found("詳しくは https://example.com/a を見てください")).toBe("https://example.com/a");
    expect(found("改行のあと\nhttp://example.com/plain")).toBe("http://example.com/plain");
  });

  it("URL でない文字列は拾わない", () => {
    expect(found("example.com は URL ではない")).toBeNull();
    expect(found("ftp://example.com も拾わない")).toBeNull();
    expect(found("スキームだけの https:// は URL ではない")).toBeNull();
  });

  it("末尾に付いた句読点は URL に含めない", () => {
    expect(found("詳しくは https://example.com/a 。")).toBe("https://example.com/a");
    expect(found("(https://example.com/b) を見る")).toBe("https://example.com/b");
  });

  /// 対応する開き括弧が中にあるなら、閉じ括弧は URL の一部。
  it("対応の取れた閉じ括弧は残す", () => {
    expect(found("https://ja.wikipedia.org/wiki/Rust_(プログラミング言語)")).toBe(
      "https://ja.wikipedia.org/wiki/Rust_(プログラミング言語)",
    );
  });

  it("いちばん先に出てくる 1 つを返す", () => {
    expect(found("https://a.example と https://b.example")).toBe("https://a.example");
  });
});

describe("opensLink", () => {
  function press(init: Partial<MouseEvent>): MouseEvent {
    return { metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...init } as MouseEvent;
  }

  it("macOS は Cmd、ほかは Ctrl", () => {
    expect(opensLink(press({ metaKey: true }), "macos")).toBe(true);
    expect(opensLink(press({ ctrlKey: true }), "macos")).toBe(false);
    expect(opensLink(press({ ctrlKey: true }), "linux")).toBe(true);
    expect(opensLink(press({ metaKey: true }), "windows")).toBe(false);
  });

  it("修飾キー無しのクリックでは開かない", () => {
    expect(opensLink(press({}), "linux")).toBe(false);
  });

  it("ほかの修飾キーが混ざっていたら開かない", () => {
    expect(opensLink(press({ ctrlKey: true, shiftKey: true }), "linux")).toBe(false);
    expect(opensLink(press({ metaKey: true, altKey: true }), "macos")).toBe(false);
  });
});
