// 下書きの上の純粋な操作のテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// 操作から SQLite までを通した振る舞いは Playwright ＋ ハーネスの担当です。

import { describe, expect, it } from "vitest";

import type { ChecklistItemDraft } from "../ipc/types/ChecklistItemDraft";
import {
  deleteChecklistItem,
  draftIsSavable,
  emptyDraft,
  moveChecklistItem,
  quickDueDates,
  setChecklistText,
  toggleChecklistItem,
  toggleTag,
} from "./draft";

function items(...texts: string[]): ChecklistItemDraft[] {
  return texts.map((text, index) => ({ id: index + 1, text, checked: false }));
}

describe("draftIsSavable", () => {
  it("タイトルが空白だけなら保存できない", () => {
    expect(draftIsSavable({ ...emptyDraft(), title: "  " })).toBe(false);
    expect(draftIsSavable({ ...emptyDraft(), title: "書く" })).toBe(true);
  });

  it("中身の無いチェックリスト項目があれば保存できない", () => {
    const draft = { ...emptyDraft(), title: "書く", checklist: items("下書き", "  ") };
    expect(draftIsSavable(draft)).toBe(false);
  });

  /// 期限の書式は Rust が読む（`docs/DESIGN.md`「絞り込みと検索」）。ここで判定を持つと 2 つがずれる。
  it("読めない期限では止めない。断るのは Rust の仕事", () => {
    expect(draftIsSavable({ ...emptyDraft(), title: "書く", dueDate: "きのう" })).toBe(true);
  });
});

describe("toggleTag", () => {
  it("付いていなければ足し、付いていれば外す", () => {
    expect(toggleTag([1, 2], 3)).toEqual([1, 2, 3]);
    expect(toggleTag([1, 2, 3], 2)).toEqual([1, 3]);
  });
});

describe("チェックリストの編集", () => {
  it("項目の文字を書き換える", () => {
    expect(setChecklistText(items("あ", "い"), 1, "う")[1]?.text).toBe("う");
  });

  it("チェックを反転する", () => {
    expect(toggleChecklistItem(items("あ"), 0)[0]?.checked).toBe(true);
  });

  it("項目を消す", () => {
    expect(deleteChecklistItem(items("あ", "い"), 0).map((item) => item.text)).toEqual(["い"]);
  });

  it("上下に 1 つぶん動かす", () => {
    const three = items("あ", "い", "う");
    expect(moveChecklistItem(three, 1, "up").map((item) => item.text)).toEqual(["い", "あ", "う"]);
    expect(moveChecklistItem(three, 1, "down").map((item) => item.text)).toEqual([
      "あ",
      "う",
      "い",
    ]);
  });

  it("端では動かさない", () => {
    const two = items("あ", "い");
    expect(moveChecklistItem(two, 0, "up").map((item) => item.text)).toEqual(["あ", "い"]);
    expect(moveChecklistItem(two, 1, "down").map((item) => item.text)).toEqual(["あ", "い"]);
  });

  it("元の配列を書き換えない", () => {
    const two = items("あ", "い");
    moveChecklistItem(two, 0, "down");
    deleteChecklistItem(two, 0);
    expect(two.map((item) => item.text)).toEqual(["あ", "い"]);
  });
});

describe("quickDueDates", () => {
  /// 2026-09-06 は日曜。週の起点は月曜なので、今週末は次の土曜、来週は翌日の月曜。
  it("日曜からは、今週末が次の土曜で、来週が翌日の月曜", () => {
    expect(quickDueDates("2026-09-06")).toEqual([
      { label: "今日", date: "2026-09-06" },
      { label: "明日", date: "2026-09-07" },
      { label: "今週末", date: "2026-09-12" },
      { label: "来週", date: "2026-09-07" },
    ]);
  });

  it("土曜の「今週末」はその日", () => {
    expect(quickDueDates("2026-09-05")[2]).toEqual({ label: "今週末", date: "2026-09-05" });
  });

  it("月曜からは、来週が 7 日後", () => {
    expect(quickDueDates("2026-09-07")[3]).toEqual({ label: "来週", date: "2026-09-14" });
  });

  it("月をまたいでも日付が繰り上がる", () => {
    expect(quickDueDates("2026-09-30")[1]).toEqual({ label: "明日", date: "2026-10-01" });
  });

  it("読めない日付には近道を出さない", () => {
    expect(quickDueDates("きょう")).toEqual([]);
  });
});
