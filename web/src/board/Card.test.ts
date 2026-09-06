// 日付の表示のような、純粋な部分のテスト（`docs/TAURI-MIGRATION.md` §10 の「部品」）。
//
// 操作から SQLite までを通した振る舞いは Playwright ＋ ハーネスの担当です。
// ここに画面の組み立てを持ち込むと、両方で同じことを確かめることになります。

import { describe, expect, it } from "vitest";

import { dueBadge, shortDate } from "./Card";

describe("shortDate", () => {
  const thisYear = new Date().getFullYear();

  it("今年の日付は月日だけにする", () => {
    expect(shortDate(`${thisYear}-03-04`)).toBe("3/4");
    expect(shortDate(`${thisYear}-12-25`)).toBe("12/25");
  });

  it("年をまたぐものには年を出す", () => {
    expect(shortDate("2019-01-02")).toBe("2019/01/02");
  });
});

describe("dueBadge", () => {
  const thisYear = new Date().getFullYear();
  const due = `${thisYear}-09-04`;

  it("状態ごとに文言と色の系統を分ける", () => {
    expect(dueBadge({ kind: "overdue", days: 2 }, due)).toEqual({
      tone: "danger",
      text: "期限切れ 2日 (9/4)",
    });
    expect(dueBadge({ kind: "today" }, due)).toEqual({ tone: "warning", text: "期限 今日 (9/4)" });
    expect(dueBadge({ kind: "soon", days: 1 }, due)).toEqual({
      tone: "info",
      text: "期限 あと 1日 (9/4)",
    });
    expect(dueBadge({ kind: "upcoming", days: 8 }, due)).toEqual({
      tone: "muted",
      text: "期限 9/4",
    });
  });

  it("期限が無いカードには何も出さない", () => {
    expect(dueBadge({ kind: "none" }, due)).toBeNull();
  });

  /// 色だけに意味を持たせない（`docs/DESIGN.md`）。記号でも語でも読めること。
  it("どの状態も、色を見なくても文言だけで区別できる", () => {
    const texts = [
      dueBadge({ kind: "overdue", days: 2 }, due)?.text,
      dueBadge({ kind: "today" }, due)?.text,
      dueBadge({ kind: "soon", days: 1 }, due)?.text,
      dueBadge({ kind: "upcoming", days: 8 }, due)?.text,
    ];
    expect(new Set(texts).size).toBe(texts.length);
  });
});
