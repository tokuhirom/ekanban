// 複製・削除・アーカイブと、チェックリストのテスト（[ADR 0039]）。
//
// 名前は `model.rs` のものを引き継いでいます。**ID を使い回さないこと**を
// 何度も確かめているのは、使い回すと `card_events` の履歴が別のカードの
// ものと混ざるためです（`docs/DESIGN.md`「盤面とカード」）。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import { describe, expect, it } from "vitest";

import {
  addCard,
  addChecklistItem,
  archiveCard,
  archiveColumn,
  copyCard,
  deleteCard,
  deleteChecklistItem,
  discardAddedCard,
  moveCard,
  moveChecklistItem,
  restoreCard,
  setCardDueDate,
  setChecklistItemChecked,
  updateChecklistItem,
} from "./board";
import { cardAt, cardById, columnAt, fixture, must, only } from "./fixture";

describe("deleteCard", () => {
  it("removes a card and reindexes the remaining cards", () => {
    const document = fixture();
    must(deleteCard(document, cardAt(document, 0, 0).id));

    expect(columnAt(document, 0).cards).toHaveLength(1);
    expect(cardAt(document, 0, 0).position).toBe(0);
    expect(cardAt(document, 0, 0).title).toBe("D&D の操作を試す");
  });

  it("does not reuse deleted card ids", () => {
    const document = fixture();
    const first = must(addCard(document, 1, "1", ""));
    const second = must(addCard(document, 1, "2", ""));
    const third = must(addCard(document, 1, "3", ""));

    must(deleteCard(document, third));

    expect(must(addCard(document, 1, "4", ""))).toBe(third + 1);
    expect(second).toBe(first + 1);
  });

  it("records a deleted event", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    must(deleteCard(document, cardId));
    expect(only(document.pendingEvents)).toMatchObject({
      cardId,
      kind: "deleted",
      fromColumnId: 1,
      toColumnId: null,
    });
  });
});

describe("discardAddedCard", () => {
  it("leaves no trace of a card that was discarded", () => {
    const document = fixture();
    const columns = structuredClone(document.board.columns);

    const cardId = must(addCard(document, 1, "", ""));
    expect(columnAt(document, 0).cards).toHaveLength(3);

    must(discardAddedCard(document, cardId));

    expect(columnAt(document, 0).cards).toHaveLength(2);
    // 追加も削除も履歴に残らない。使う人から見れば一度も存在していない。
    expect(document.pendingEvents).toEqual([]);
    expect(document.undoStack).toEqual([]);
    expect(document.board.columns).toEqual(columns);
  });

  it("keeps the operations that came before it", () => {
    const document = fixture();
    const moved = cardAt(document, 0, 0).id;
    must(moveCard(document, moved, 2, 0));

    const cardId = must(addCard(document, 1, "", ""));
    must(discardAddedCard(document, cardId));

    // 取り下げるのは追加した 1 手だけ。その前の移動は残っている。
    expect(document.undoStack.map((operation) => operation.kind)).toEqual(["moveCard"]);
  });

  it("does not reuse the id of a discarded card", () => {
    const document = fixture();
    const discarded = must(addCard(document, 1, "", ""));
    must(discardAddedCard(document, discarded));
    expect(must(addCard(document, 1, "次のカード", ""))).not.toBe(discarded);
  });

  it("rejects discarding a card that is not there", () => {
    const document = fixture();
    expect(discardAddedCard(document, 999)).toEqual({
      ok: false,
      error: { kind: "cardNotFound", cardId: 999 },
    });
  });
});

describe("archiveCard", () => {
  it("archives and restores cards without reusing ids", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    expect(must(archiveCard(document, cardId))).toBe(true);
    expect(columnAt(document, 0).cards.some((card) => card.id === cardId)).toBe(false);
    expect(only(document.board.archivedCards).archivedAt).not.toBeNull();

    expect(must(restoreCard(document, cardId))).toBe(true);
    expect(columnAt(document, 0).cards.some((card) => card.id === cardId)).toBe(true);
    expect(document.board.archivedCards).toEqual([]);
    expect(must(addCard(document, 1, "新規", ""))).toBe(document.nextCardId - 1);
  });

  /// 戻り先は元のカラムの末尾。カラムが消えていたら先頭のカラムへ。
  it("restores a card to the first column when its own column is gone", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    must(archiveCard(document, cardId));
    document.board.columns = document.board.columns.filter((column) => column.id !== 1);

    must(restoreCard(document, cardId));

    expect(columnAt(document, 0).cards.map((card) => card.id)).toContain(cardId);
  });

  it("records the lifecycle events but not intra column reorders", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    must(moveCard(document, cardId, 2, 1));
    must(moveCard(document, cardId, 2, 0));
    must(archiveCard(document, cardId));

    expect(document.pendingEvents.map((event) => event.kind)).toEqual(["moved", "archived"]);
    expect(document.pendingEvents[0]).toMatchObject({ fromColumnId: 1, toColumnId: 2 });
    expect(document.pendingEvents[1]).toMatchObject({ fromColumnId: 2, toColumnId: null });
  });
});

describe("archiveColumn", () => {
  it("records one archived event for each card in the column", () => {
    const document = fixture();

    expect(must(archiveColumn(document, 1))).toBe(2);
    expect(document.pendingEvents).toHaveLength(2);
    expect(document.pendingEvents.every((event) => event.kind === "archived")).toBe(true);
    // 積まれる操作は 1 件。まとめて戻せる。
    expect(document.undoStack.map((operation) => operation.kind)).toEqual(["archiveColumn"]);
  });

  it("does nothing to an empty column", () => {
    const document = fixture();
    must(archiveColumn(document, 1));
    expect(must(archiveColumn(document, 1))).toBe(0);
    expect(document.undoStack).toHaveLength(1);
  });
});

describe("copyCard", () => {
  it("copies card content and resets the due date and the checks", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    must(setCardDueDate(document, cardId, "2026-09-12"));
    const itemId = must(addChecklistItem(document, cardId, "済ませること"));
    must(setChecklistItemChecked(document, cardId, itemId, true));

    const copyId = must(copyCard(document, cardId));
    const copy = cardById(document, copyId);
    const source = cardById(document, cardId);

    expect(copy.title).toBe(source.title);
    expect(copy.description).toBe(source.description);
    expect(copy.tagIds).toEqual(source.tagIds);
    // 期限と済んだ印は引き継がない。複製は「これからやること」を作る操作。
    expect(copy.dueDate).toBeNull();
    expect(only(copy.checklistItems).checked).toBe(false);
    expect(only(copy.checklistItems).text).toBe("済ませること");
    expect(only(copy.checklistItems).id).not.toBe(itemId);
    // すぐ下に置く。
    expect(cardAt(document, 0, 1).id).toBe(copyId);
  });
});

describe("checklist", () => {
  it("manages checklist items and reindexes them", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    const first = must(addChecklistItem(document, cardId, "1 つめ"));
    const second = must(addChecklistItem(document, cardId, "2 つめ"));
    const third = must(addChecklistItem(document, cardId, "3 つめ"));

    expect(must(updateChecklistItem(document, cardId, first, "書き換えた"))).toBe(true);
    expect(must(updateChecklistItem(document, cardId, first, "書き換えた"))).toBe(false);
    expect(must(setChecklistItemChecked(document, cardId, second, true))).toBe(true);

    must(moveChecklistItem(document, cardId, third, 0));
    expect(cardById(document, cardId).checklistItems.map((item) => item.id)).toEqual([
      third,
      first,
      second,
    ]);
    expect(cardById(document, cardId).checklistItems.map((item) => item.position)).toEqual([0, 1, 2]);

    must(deleteChecklistItem(document, cardId, first));
    expect(cardById(document, cardId).checklistItems.map((item) => item.id)).toEqual([third, second]);
    expect(cardById(document, cardId).checklistItems.map((item) => item.position)).toEqual([0, 1]);
  });

  /// 1 項目ずつの操作は「空の項目を作れ」という指示なので断る（#114）。
  it("rejects empty checklist items", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    expect(addChecklistItem(document, cardId, "  ")).toEqual({
      ok: false,
      error: { kind: "emptyChecklistItemText" },
    });
    const itemId = must(addChecklistItem(document, cardId, "項目"));
    expect(updateChecklistItem(document, cardId, itemId, " ")).toEqual({
      ok: false,
      error: { kind: "emptyChecklistItemText" },
    });
  });

  it("rejects an item that is not on the card", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    expect(setChecklistItemChecked(document, cardId, 999, true)).toEqual({
      ok: false,
      error: { kind: "checklistItemNotFound", itemId: 999, cardId },
    });
  });
});

describe("setCardDueDate", () => {
  it("sets a due date and skips unchanged values", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    expect(must(setCardDueDate(document, cardId, "2026-09-30"))).toBe(true);
    expect(must(setCardDueDate(document, cardId, "2026-09-30"))).toBe(false);
    expect(cardById(document, cardId).dueDate).toBe("2026-09-30");
    expect(must(setCardDueDate(document, cardId, null))).toBe(true);
  });
});
