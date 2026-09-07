// 変換中の判定（`docs/DESIGN.md`「メニューとキー割り当て」、「テスト」の「部品」）。
//
// **変換を確定する `Enter` で保存してはいけません。** `isComposing` だけを
// 見ていたときの #124 がこれで、WebKit では確定の `keydown` が
// `isComposing === false` で届きます。

import { describe, expect, it } from "vitest";

import { isComposing } from "./ime";

describe("isComposing", () => {
  it("変換中は取らない", () => {
    expect(isComposing({ isComposing: true, keyCode: 229 })).toBe(true);
  });

  /// WebKit は `compositionend` を先に出すので、確定の `Enter` は
  /// `isComposing === false` で届く。`keyCode` だけが変換の名残りを持つ。
  it("確定の Enter は isComposing が false でも取らない", () => {
    expect(isComposing({ isComposing: false, keyCode: 229 })).toBe(true);
  });

  it("ふつうの Enter は通す", () => {
    expect(isComposing({ isComposing: false, keyCode: 13 })).toBe(false);
  });

  it("ふつうの Escape は通す", () => {
    expect(isComposing({ isComposing: false, keyCode: 27 })).toBe(false);
  });
});
