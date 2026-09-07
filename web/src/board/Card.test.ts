// 日付の表示のような、純粋な部分のテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// 操作から SQLite までを通した振る舞いは Playwright ＋ ハーネスの担当です。
// ここに画面の組み立てを持ち込むと、両方で同じことを確かめることになります。
//
// **時計を読むテストはありません**（#133）。今日が何日かは Rust から来る
// `Snapshot.today` で、ここに渡すのもその値です。

import { describe, expect, it } from "vitest";

import { dueBadge, shortDate } from "./Card";

const today = "2026-09-06";

describe("shortDate", () => {
  it("today と同じ年の日付は月日だけにする", () => {
    expect(shortDate("2026-03-04", today)).toBe("3/4");
    expect(shortDate("2026-12-25", today)).toBe("12/25");
  });

  it("年をまたぐものには年を出す", () => {
    expect(shortDate("2019-01-02", today)).toBe("2019/01/02");
    expect(shortDate("2027-01-02", today)).toBe("2027/01/02");
  });
});

describe("dueBadge", () => {
  const due = "2026-09-04";

  it("4 つの状態を、同じ「印 + 日付 + 補足」の形で出す", () => {
    expect(dueBadge({ kind: "overdue", days: 2 }, due, today)).toEqual({
      tone: "danger",
      text: "⚠ 9/4（2日超過）",
    });
    expect(dueBadge({ kind: "today" }, due, today)).toEqual({ tone: "warning", text: "◷ 今日" });
    expect(dueBadge({ kind: "soon", days: 1 }, due, today)).toEqual({
      tone: "info",
      text: "9/4（あと1日）",
    });
    expect(dueBadge({ kind: "upcoming", days: 8 }, due, today)).toEqual({
      tone: "muted",
      text: "9/4",
    });
  });

  it("期限が無いカードには何も出さない", () => {
    expect(dueBadge({ kind: "none" }, due, today)).toBeNull();
  });

  /// 色だけに意味を持たせない（`docs/DESIGN.md`）。記号でも語でも読めること。
  it("どの状態も、色を見なくても文言だけで区別できる", () => {
    const texts = [
      dueBadge({ kind: "overdue", days: 2 }, due, today)?.text,
      dueBadge({ kind: "today" }, due, today)?.text,
      dueBadge({ kind: "soon", days: 1 }, due, today)?.text,
      dueBadge({ kind: "upcoming", days: 8 }, due, today)?.text,
    ];
    expect(new Set(texts).size).toBe(texts.length);
  });
});
