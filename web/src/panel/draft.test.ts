// 下書きの上の純粋な操作のテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// 操作から SQLite までを通した振る舞いは Playwright ＋ ハーネスの担当です。

import { describe, expect, it } from "vitest";

import {
  checklistToSend,
  deleteChecklistItem,
  draftIsSavable,
  emptyDraft,
  moveChecklistItem,
  newChecklistItem,
  quickDueDates,
  reorderChecklist,
  setChecklistText,
  toggleChecklistItem,
  toggleTag,
  type DraftChecklistItem,
} from "./draft";

function items(...texts: string[]): DraftChecklistItem[] {
  return texts.map((text, index) => ({
    key: `saved-${index + 1}`,
    id: index + 1,
    text,
    checked: false,
  }));
}

describe("draftIsSavable", () => {
  it("タイトルが空白だけなら保存できない", () => {
    expect(draftIsSavable({ ...emptyDraft(), title: "  " })).toBe(false);
    expect(draftIsSavable({ ...emptyDraft(), title: "書く" })).toBe(true);
  });

  /// 空の項目は保存のときに Rust が落とす（#114）。判定をこちらにも置かない。
  it("中身の無いチェックリスト項目があっても止めない", () => {
    const draft = { ...emptyDraft(), title: "書く", checklist: items("下書き", "  ") };
    expect(draftIsSavable(draft)).toBe(true);
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
    reorderChecklist(two, "saved-1", "saved-2");
    expect(two.map((item) => item.text)).toEqual(["あ", "い"]);
  });

  /// 掴んで落としたときの並べ替え（#113）。
  it("掴んだ項目を、落とし先のいた位置へ入れる", () => {
    const three = items("あ", "い", "う");
    expect(reorderChecklist(three, "saved-3", "saved-1").map((item) => item.text)).toEqual([
      "う",
      "あ",
      "い",
    ]);
    expect(reorderChecklist(three, "saved-1", "saved-3").map((item) => item.text)).toEqual([
      "い",
      "う",
      "あ",
    ]);
  });

  it("落とし先が掴んだものと同じ、または鍵が無ければ並びは変わらない", () => {
    const three = items("あ", "い", "う");
    expect(reorderChecklist(three, "saved-2", "saved-2").map((item) => item.text)).toEqual([
      "あ",
      "い",
      "う",
    ]);
    expect(reorderChecklist(three, "saved-2", "new-9").map((item) => item.text)).toEqual([
      "あ",
      "い",
      "う",
    ]);
  });
});

describe("下書きの鍵", () => {
  it("新しい項目は呼ぶたびに違う鍵を持ち、まだ ID は無い", () => {
    const first = newChecklistItem();
    const second = newChecklistItem();
    expect(first.key).not.toBe(second.key);
    expect(first.id).toBeNull();
    expect(first.text).toBe("");
    expect(first.checked).toBe(false);
  });

  /// `key` は画面の鍵なので、Rust には渡さない。
  it("Rust に渡す形では鍵が落ちて、並びはそのまま", () => {
    const checklist = [...items("あ", "い"), newChecklistItem()];
    expect(checklistToSend(checklist)).toEqual([
      { id: 1, text: "あ", checked: false },
      { id: 2, text: "い", checked: false },
      { id: null, text: "", checked: false },
    ]);
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
