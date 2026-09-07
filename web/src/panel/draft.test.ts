// 下書きの上の純粋な操作のテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// 操作から SQLite までを通した振る舞いは Playwright ＋ ハーネスの担当です。

import { describe, expect, it } from "vitest";

import {
  checklistToSend,
  deleteChecklistItem,
  insertChecklistItemAfter,
  syncChecklistIds,
  draftIsSavable,
  emptyDraft,
  moveChecklistItem,
  newChecklistItem,
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

describe("insertChecklistItemAfter", () => {
  const list = [
    { key: "a", id: 1, text: "あ", checked: false },
    { key: "b", id: 2, text: "い", checked: false },
  ];

  it("指した行の直後に空の項目を入れる", () => {
    const { checklist, key } = insertChecklistItemAfter(list, 0);
    expect(checklist.map((item) => item.text)).toEqual(["あ", "", "い"]);
    expect(checklist[1]?.key).toBe(key);
    expect(checklist[1]?.id).toBeNull();
  });

  it("末尾の行の直後は末尾に足す", () => {
    const { checklist } = insertChecklistItemAfter(list, 1);
    expect(checklist.map((item) => item.text)).toEqual(["あ", "い", ""]);
  });

  it("範囲の外を指されたら末尾に足す", () => {
    expect(insertChecklistItemAfter(list, 9).checklist).toHaveLength(3);
    expect(insertChecklistItemAfter([], 0).checklist).toHaveLength(1);
  });
});

describe("syncChecklistIds", () => {
  /// ID を写さないと、次の確定で同じ項目がもう 1 つ増える（#141）。
  it("まだ ID を持たない項目に、保存された ID を写す", () => {
    const drafted = [
      { key: "a", id: 1, text: "あ", checked: false },
      { key: "b", id: null, text: "い", checked: false },
    ];
    expect(syncChecklistIds(drafted, [{ id: 1 }, { id: 7 }])).toEqual([
      { key: "a", id: 1, text: "あ", checked: false },
      { key: "b", id: 7, text: "い", checked: false },
    ]);
  });

  it("既に ID を持つ項目は触らない", () => {
    const drafted = [{ key: "a", id: 3, text: "あ", checked: false }];
    expect(syncChecklistIds(drafted, [{ id: 9 }])).toEqual(drafted);
  });

  it("返ってきた並びが短くても落ちない", () => {
    const drafted = [{ key: "a", id: null, text: "あ", checked: false }];
    expect(syncChecklistIds(drafted, [])).toEqual(drafted);
  });

  /// 空の行は保存のときに落ちる（#114）ので、返ってきた並びには入っていない。
  /// 数に入れると、そこから下の行に 1 つずれた ID が付く。
  it("空の行を数に入れず、そこから下の ID をずらさない", () => {
    const drafted = [
      { key: "a", id: null, text: "あ", checked: false },
      { key: "b", id: null, text: "  ", checked: false },
      { key: "c", id: null, text: "い", checked: false },
    ];
    expect(syncChecklistIds(drafted, [{ id: 4 }, { id: 5 }])).toEqual([
      { key: "a", id: 4, text: "あ", checked: false },
      { key: "b", id: null, text: "  ", checked: false },
      { key: "c", id: 5, text: "い", checked: false },
    ]);
  });

  /// 文字を消した行は、保存の側ではもう無い。ID を持たせたままにすると、
  /// 打ち直したときにもう無い項目を指す。
  it("文字を消した行からは ID を外す", () => {
    const drafted = [{ key: "a", id: 7, text: "", checked: false }];
    expect(syncChecklistIds(drafted, [])).toEqual([
      { key: "a", id: null, text: "", checked: false },
    ]);
  });
});
