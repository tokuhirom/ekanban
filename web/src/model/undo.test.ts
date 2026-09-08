// 取り消しとやり直しのテスト（[ADR 0039]）。
//
// **移植でいちばん危ないところです。** 取りこぼしても画面には何も出ず、
// 「戻せない」という形でしか現れません。だから操作の種類ごとに、戻して・
// やり直して・元の形に戻ることを 1 つずつ確かめます。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import { describe, expect, it } from "vitest";

import type { BoardDocument } from "./board";
import {
  addCard,
  addChecklistItem,
  addColumn,
  addTag,
  archiveCard,
  archiveColumn,
  canRedo,
  canUndo,
  copyCard,
  deleteCard,
  deleteChecklistItem,
  moveCard,
  moveChecklistItem,
  moveColumn,
  redo,
  removeColumn,
  removeTag,
  renameColumn,
  renameTag,
  restoreCard,
  setCardDueDate,
  setCardTags,
  setChecklistItemChecked,
  setColumnDone,
  setTagColor,
  undo,
  updateCard,
  updateCardDetails,
  updateChecklistItem,
} from "./board";
import { cardAt, cardById, columnAt, fixture, must, only } from "./fixture";

/// 盤面だけを比べる形。時刻は動くので落とす——**戻したときに同じ盤面に
/// なること**が見たいのであって、同じ瞬間に戻ることではない。
function snapshot(document: BoardDocument): unknown {
  return JSON.parse(
    JSON.stringify(document.board, (key, value: unknown) =>
      key === "createdAt" || key === "updatedAt" ? 0 : value,
    ),
  );
}

/// 1 つの操作について、**やって・戻して・やり直して**を確かめる。
///
/// 戻したら元の盤面に、やり直したら操作後の盤面に、それぞれ一致すること。
function roundTrip(
  document: BoardDocument,
  act: (document: BoardDocument) => void,
): void {
  const before = snapshot(document);
  act(document);
  const after = snapshot(document);
  expect(after).not.toEqual(before);

  expect(must(undo(document))).toBe(true);
  expect(snapshot(document)).toEqual(before);

  expect(must(redo(document))).toBe(true);
  expect(snapshot(document)).toEqual(after);
}

describe("undo and redo", () => {
  it("undoes and redoes card operations without reusing snapshots", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    const originalTitle = cardAt(document, 0, 0).title;

    expect(must(moveCard(document, cardId, 2, 0))).toBe(true);
    expect(canUndo(document)).toBe(true);
    expect(canRedo(document)).toBe(false);
    expect(cardAt(document, 1, 0).id).toBe(cardId);

    expect(must(undo(document))).toBe(true);
    expect(cardAt(document, 0, 0).id).toBe(cardId);
    expect(canUndo(document)).toBe(false);
    expect(canRedo(document)).toBe(true);

    expect(must(redo(document))).toBe(true);
    expect(cardAt(document, 1, 0).id).toBe(cardId);

    expect(must(updateCard(document, cardId, "更新", "説明"))).toBe(true);
    expect(must(undo(document))).toBe(true);
    expect(cardAt(document, 1, 0).title).toBe(originalTitle);
    expect(must(redo(document))).toBe(true);
    expect(cardAt(document, 1, 0).title).toBe("更新");
  });

  it("clears the redo history when a new operation is done", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;

    must(setCardDueDate(document, cardId, "2026-09-30"));
    must(undo(document));
    expect(canRedo(document)).toBe(true);

    must(setCardDueDate(document, cardId, "2026-10-01"));
    expect(canRedo(document)).toBe(false);
  });

  it("says false when there is nothing to undo or redo", () => {
    const document = fixture();
    expect(must(undo(document))).toBe(false);
    expect(must(redo(document))).toBe(false);
  });

  /// 取り消しはフローの出来事ではない（`docs/DESIGN.md`「盤面とカード」）。
  it("does not record card events", () => {
    const document = fixture();
    must(moveCard(document, cardAt(document, 0, 0).id, 2, 0));
    document.pendingEvents.length = 0;

    must(undo(document));
    must(redo(document));

    expect(document.pendingEvents).toEqual([]);
  });
});

describe("every operation round trips", () => {
  it("moveCard", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(moveCard(document, cardId, 2, 0));
    });
  });

  it("moveColumn", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(moveColumn(document, 1, 3));
    });
  });

  it("addCard", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(addCard(document, 1, "足したカード", "説明"));
    });
  });

  it("updateCard", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(updateCard(document, cardId, "更新", "説明"));
    });
  });

  /// 編集パネルの 1 回の確定は、Undo でも 1 回（ADR 0032）。
  it("updateCardDetails with its checklist", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    const tagId = must(addTag(document, "手順", "#60a5fa"));
    roundTrip(document, (document) => {
      must(
        updateCardDetails(
          document,
          cardId,
          "PR を出す",
          "手順を確認する",
          "2026-09-30",
          [tagId],
          [
            { id: null, text: "テストを書く", checked: false },
            { id: null, text: "fmt を通す", checked: true },
          ],
        ),
      );
    });
  });

  it("copyCard", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(copyCard(document, cardId));
    });
  });

  it("deleteCard", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(deleteCard(document, cardId));
    });
  });

  it("archiveCard", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(archiveCard(document, cardId));
    });
  });

  it("archiveColumn", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(archiveColumn(document, 1));
    });
  });

  it("restoreCard", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    must(archiveCard(document, cardId));
    roundTrip(document, (document) => {
      must(restoreCard(document, cardId));
    });
  });

  it("setCardDueDate", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(setCardDueDate(document, cardId, "2026-09-30"));
    });
  });

  it("addChecklistItem", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(addChecklistItem(document, cardId, "項目"));
    });
  });

  it("updateChecklistItem", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    const itemId = must(addChecklistItem(document, cardId, "項目"));
    roundTrip(document, (document) => {
      must(updateChecklistItem(document, cardId, itemId, "書き換えた"));
    });
  });

  it("setChecklistItemChecked", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    const itemId = must(addChecklistItem(document, cardId, "項目"));
    roundTrip(document, (document) => {
      must(setChecklistItemChecked(document, cardId, itemId, true));
    });
  });

  it("deleteChecklistItem", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    must(addChecklistItem(document, cardId, "1 つめ"));
    const itemId = must(addChecklistItem(document, cardId, "2 つめ"));
    roundTrip(document, (document) => {
      must(deleteChecklistItem(document, cardId, itemId));
    });
  });

  it("moveChecklistItem", () => {
    const document = fixture();
    const cardId = cardAt(document, 0, 0).id;
    must(addChecklistItem(document, cardId, "1 つめ"));
    const itemId = must(addChecklistItem(document, cardId, "2 つめ"));
    roundTrip(document, (document) => {
      must(moveChecklistItem(document, cardId, itemId, 0));
    });
  });

  it("addTag", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(addTag(document, "重要", "#ef4444"));
    });
  });

  it("renameTag", () => {
    const document = fixture();
    const tagId = must(addTag(document, "重要", "#ef4444"));
    roundTrip(document, (document) => {
      must(renameTag(document, tagId, "至急"));
    });
  });

  it("setTagColor", () => {
    const document = fixture();
    const tagId = must(addTag(document, "重要", "#ef4444"));
    roundTrip(document, (document) => {
      must(setTagColor(document, tagId, "#60a5fa"));
    });
  });

  /// 消したタグを戻すときは、**付いていたカードにだけ**付け直す。
  it("removeTag puts the tag back on the cards that had it", () => {
    const document = fixture();
    const tagId = must(addTag(document, "重要", "#ef4444"));
    const cardId = cardAt(document, 0, 0).id;
    must(setCardTags(document, cardId, [tagId]));
    const otherId = cardAt(document, 0, 1).id;

    roundTrip(document, (document) => {
      must(removeTag(document, tagId));
    });

    must(undo(document));
    expect(cardById(document, cardId).tagIds).toEqual([tagId]);
    expect(cardById(document, otherId).tagIds).toEqual([]);
  });

  it("setCardTags", () => {
    const document = fixture();
    const tagId = must(addTag(document, "重要", "#ef4444"));
    const cardId = cardAt(document, 0, 0).id;
    roundTrip(document, (document) => {
      must(setCardTags(document, cardId, [tagId]));
    });
  });

  it("addColumn", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(addColumn(document, "追加したカラム"));
    });
  });

  it("renameColumn", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(renameColumn(document, 1, "書き換えた"));
    });
  });

  it("setColumnDone", () => {
    const document = fixture();
    roundTrip(document, (document) => {
      must(setColumnDone(document, 3, true));
    });
  });

  /// 消したカラムを戻すと、中のカードも、アーカイブの行き先も元どおり。
  it("removeColumn brings back its cards and the archived cards that pointed at it", () => {
    const document = fixture();
    const archivedId = cardAt(document, 0, 0).id;
    must(archiveCard(document, archivedId));

    roundTrip(document, (document) => {
      must(removeColumn(document, 1));
    });

    must(undo(document));
    expect(columnAt(document, 0).id).toBe(1);
    expect(only(document.board.archivedCards).columnId).toBe(1);
  });
});

describe("ids after undo", () => {
  /// **採番は戻しません。** 詰めると、やり直したときに同じ ID が 2 枚のカードに
  /// 付きます。
  it("does not reuse the id of a card that was undone", () => {
    const document = fixture();
    const first = must(addCard(document, 1, "1 枚目", ""));
    must(undo(document));
    const second = must(addCard(document, 1, "2 枚目", ""));
    expect(second).not.toBe(first);
  });

  /// やり直しで戻ってきたカードの ID より、次の採番は必ず先へ進む。
  it("keeps the counter ahead of a card that came back with redo", () => {
    const document = fixture();
    const cardId = must(addCard(document, 1, "1 枚目", ""));
    must(undo(document));
    must(redo(document));
    expect(document.nextCardId).toBeGreaterThan(cardId);
  });
});
