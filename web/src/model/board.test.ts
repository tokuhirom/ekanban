// 盤面のモデルのテスト（[ADR 0039]）。
//
// **名前は Rust のものをそのまま引き継いでいます。** 移す前に `model.rs` が
// `#[cfg(test)]` で押さえていた振る舞いを、1 つずつ同じ観点で確かめるためです。
// 移植の正しさを見るのはこのファイルで、実装を読み比べることではありません。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import { describe, expect, it } from "vitest";

import {
  addCard,
  addCardWithDetails,
  moveCard,
  moveColumn,
  updateCard,
  updateCardDetails,
} from "./board";
import { cardAt, cardById, columnAt, fixture, must, only, shape } from "./fixture";

describe("moveCard", () => {
  it("moves a card to another column", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    expect(must(moveCard(document, cardId, 2, 0))).toBe(true);
    expect(columnAt(document, 0).cards).toHaveLength(1);
    expect(cardAt(document, 1, 0).id).toBe(cardId);
    expect(cardAt(document, 1, 0).columnId).toBe(2);
  });

  it("reorders a card inside a column", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    expect(must(moveCard(document, cardId, 1, 2))).toBe(true);
    expect(cardAt(document, 0, 0).title).toBe("D&D の操作を試す");
    expect(cardAt(document, 0, 1).id).toBe(cardId);
  });

  it("treats moving a card to its current position as a noop", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    expect(must(moveCard(document, cardId, 1, 0))).toBe(false);
    expect(cardAt(document, 0, 0).id).toBe(cardId);
  });

  /// カラムをまたいだ移動だけが履歴に残る（`docs/DESIGN.md`「盤面とカード」）。
  it("records a moved event only when the card changes column", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    must(moveCard(document, cardId, 1, 2));
    expect(document.pendingEvents).toHaveLength(0);

    must(moveCard(document, cardId, 2, 0));
    const event = only(document.pendingEvents);
    expect(event).toMatchObject({ cardId, kind: "moved", fromColumnId: 1, toColumnId: 2 });
    expect(event.at).toBeGreaterThan(0);
  });

  it("reindexes the positions of both columns", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    must(moveCard(document, cardId, 2, 0));

    for (const column of document.board.columns) {
      expect(column.cards.map((card) => card.position)).toEqual(
        column.cards.map((_, index) => index),
      );
    }
  });

  it("rejects an unknown card and an unknown column", () => {
    const document = fixture();
    expect(moveCard(document, 999, 1, 0)).toEqual({
      ok: false,
      error: { kind: "cardNotFound", cardId: 999 },
    });
    const cardId = cardAt(document, 0, 0).id;
    expect(moveCard(document, cardId, 999, 0)).toEqual({
      ok: false,
      error: { kind: "columnNotFound", columnId: 999 },
    });
    // 断られた操作は盤面に触れていない。
    expect(shape(document)).toEqual([
      [1, [1, 2]],
      [2, [3]],
      [3, [4]],
    ]);
  });
});

describe("moveColumn", () => {
  it("reorders columns", () => {
    const document = fixture();

    expect(must(moveColumn(document, 1, document.board.columns.length))).toBe(true);
    expect(document.board.columns.map((column) => column.name)).toEqual([
      "進行中",
      "完了",
      "やること",
    ]);
    expect(columnAt(document, 2).position).toBe(2);
  });

  it("treats moving a column to its current position as a noop", () => {
    const document = fixture();
    expect(must(moveColumn(document, 2, 1))).toBe(false);
    expect(columnAt(document, 1).id).toBe(2);
  });
});

describe("addCard", () => {
  it("adds a card with a due date, tags and a checklist", () => {
    const document = fixture();
    document.board.tags = [
      { id: 1, boardId: 1, name: "重要", color: "#60a5fa", createdAt: 0, updatedAt: 0 },
    ];

    const cardId = must(
      addCardWithDetails(document, 1, "支度", "説明", "2026-09-12", [1], [
        { id: null, text: "済んだこと", checked: true },
        { id: null, text: "まだのこと", checked: false },
      ]),
    );

    const card = cardById(document, cardId);
    expect(card.dueDate).toBe("2026-09-12");
    expect(card.tagIds).toEqual([1]);
    expect(card.checklistItems.map((item) => [item.text, item.checked, item.position])).toEqual([
      ["済んだこと", true, 0],
      ["まだのこと", false, 1],
    ]);
    // 足すのは 1 操作。備えた状態で作るので、取り消しも 1 回で済む。
    expect(document.undoStack).toHaveLength(1);
  });

  it("drops blank checklist items when adding a card", () => {
    const document = fixture();
    const cardId = must(
      addCardWithDetails(document, 1, "支度", "", null, [], [
        { id: null, text: "  ", checked: false },
        { id: null, text: "残るもの", checked: false },
      ]),
    );

    const card = cardById(document, cardId);
    expect(card.checklistItems.map((item) => item.text)).toEqual(["残るもの"]);
  });

  it("rejects an unknown tag and an unknown column without spending an id", () => {
    const document = fixture();
    const before = document.nextCardId;

    expect(addCardWithDetails(document, 1, "支度", "", null, [999], [])).toEqual({
      ok: false,
      error: { kind: "tagNotFound", tagId: 999 },
    });
    expect(addCard(document, 999, "支度", "")).toEqual({
      ok: false,
      error: { kind: "columnNotFound", columnId: 999 },
    });
    expect(document.nextCardId).toBe(before);
  });

  it("records a created event", () => {
    const document = fixture();
    const cardId = must(addCard(document, 2, "新しいカード", ""));
    const event = only(document.pendingEvents);
    expect(event).toMatchObject({ cardId, kind: "created", fromColumnId: null, toColumnId: 2 });
    expect(event.at).toBeGreaterThan(0);
  });
});

describe("updateCard", () => {
  it("updates card content", () => {
    const document = fixture();
    expect(must(updateCard(document, 1, "書き換えた", "説明も"))).toBe(true);
    expect(cardAt(document, 0, 0).title).toBe("書き換えた");
    // 同じ値で呼んでも、何も起きない。
    expect(must(updateCard(document, 1, "書き換えた", "説明も"))).toBe(false);
  });

  it("rejects an empty card title", () => {
    const document = fixture();
    expect(updateCard(document, 1, "   ", "")).toEqual({
      ok: false,
      error: { kind: "emptyCardTitle" },
    });
    expect(cardAt(document, 0, 0).title).toBe("画面の描画をひととおり通す");
  });
});

describe("updateCardDetails", () => {
  /// 編集パネルの 1 回の確定は、Undo でも 1 回（ADR 0032）。
  it("saves a card editor commit as one operation", () => {
    const document = fixture();
    document.board.tags = [
      { id: 1, boardId: 1, name: "重要", color: "#60a5fa", createdAt: 0, updatedAt: 0 },
    ];

    expect(
      must(
        updateCardDetails(document, 1, "題", "説明", "2026-09-12", [1], [
          { id: null, text: "項目", checked: false },
        ]),
      ),
    ).toBe(true);

    expect(document.undoStack).toHaveLength(1);
    expect(only(document.undoStack).kind).toBe("editCard");
  });

  it("removes a saved checklist item whose text was cleared", () => {
    const document = fixture();
    must(updateCardDetails(document, 1, "題", "", null, [], [{ id: null, text: "項目", checked: false }]));
    const itemId = only(cardAt(document, 0, 0).checklistItems).id;

    must(updateCardDetails(document, 1, "題", "", null, [], [{ id: itemId, text: "  ", checked: false }]));

    expect(cardAt(document, 0, 0).checklistItems).toEqual([]);
  });

  it("rejects a checklist item that belongs to no card", () => {
    const document = fixture();
    expect(
      updateCardDetails(document, 1, "題", "", null, [], [{ id: 999, text: "項目", checked: false }]),
    ).toEqual({ ok: false, error: { kind: "checklistItemNotFound", itemId: 999, cardId: 1 } });
  });

  /// 触っていない項目の時刻は動かさない。並べ替えただけの項目が「いま直した
  /// もの」に見えないように。
  it("leaves the timestamp of an untouched item alone", () => {
    const document = fixture();
    must(updateCardDetails(document, 1, "題", "", null, [], [{ id: null, text: "項目", checked: false }]));
    const item = only(cardAt(document, 0, 0).checklistItems);

    must(
      updateCardDetails(document, 1, "別の題", "", null, [], [
        { id: item.id, text: item.text, checked: item.checked },
      ]),
    );

    expect(only(cardAt(document, 0, 0).checklistItems).updatedAt).toBe(item.updatedAt);
  });

  it("does nothing when every field already holds that value", () => {
    const document = fixture();
    must(updateCardDetails(document, 1, "題", "説明", null, [], []));
    expect(must(updateCardDetails(document, 1, "題", "説明", null, [], []))).toBe(false);
  });
});
