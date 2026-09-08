// タグとカラムのテスト（[ADR 0039]）。名前は `model.rs` から引き継いでいます。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import { describe, expect, it } from "vitest";

import {
  addColumn,
  addTag,
  archiveCard,
  archiveColumn,
  removeColumn,
  removeTag,
  renameBoard,
  renameColumn,
  renameTag,
  setCardTags,
  setColumnDone,
} from "./board";
import { cardAt, cardById, columnAt, fixture, must, only } from "./fixture";

describe("tags", () => {
  it("manages tags and card assignments", () => {
    const document = fixture();
    const tagId = must(addTag(document, "重要", "#ef4444"));
    const otherTagId = must(addTag(document, "個人", "#60a5fa"));
    const cardId = cardAt(document, 0, 0).id;

    // 同じタグを 2 回渡しても 1 つ。並びは ID の昇順にそろえる。
    expect(must(setCardTags(document, cardId, [otherTagId, tagId, tagId]))).toBe(true);
    expect(cardById(document, cardId).tagIds).toEqual([tagId, otherTagId]);
    expect(must(setCardTags(document, cardId, [tagId, otherTagId]))).toBe(false);

    expect(must(renameTag(document, tagId, "最重要"))).toBe(true);
    must(removeTag(document, otherTagId));
    expect(cardById(document, cardId).tagIds).toEqual([tagId]);
  });

  it("rejects empty and duplicate tag names", () => {
    const document = fixture();

    expect(addTag(document, " ", "#000000")).toEqual({
      ok: false,
      error: { kind: "emptyTagName" },
    });
    must(addTag(document, "仕事", "#000000"));
    expect(addTag(document, "仕事", "#ffffff")).toEqual({
      ok: false,
      error: { kind: "duplicateTagName", name: "仕事" },
    });
  });

  it("rejects renaming a tag onto a name another tag already has", () => {
    const document = fixture();
    must(addTag(document, "仕事", "#000000"));
    const other = must(addTag(document, "個人", "#ffffff"));
    expect(renameTag(document, other, "仕事")).toEqual({
      ok: false,
      error: { kind: "duplicateTagName", name: "仕事" },
    });
  });

  it("takes a removed tag off the archived cards too", () => {
    const document = fixture();
    const tagId = must(addTag(document, "重要", "#ef4444"));
    const cardId = cardAt(document, 0, 0).id;
    must(setCardTags(document, cardId, [tagId]));
    must(archiveCard(document, cardId));

    must(removeTag(document, tagId));

    expect(only(document.board.archivedCards).tagIds).toEqual([]);
  });
});

describe("columns", () => {
  it("renames a column and skips unchanged values", () => {
    const document = fixture();
    expect(must(renameColumn(document, 1, "やること"))).toBe(false);
    expect(must(renameColumn(document, 1, "積んであるもの"))).toBe(true);
    expect(columnAt(document, 0).name).toBe("積んであるもの");
  });

  it("rejects empty column names", () => {
    const document = fixture();
    expect(addColumn(document, "  ")).toEqual({
      ok: false,
      error: { kind: "emptyColumnName" },
    });
    expect(renameColumn(document, 1, "\n")).toEqual({
      ok: false,
      error: { kind: "emptyColumnName" },
    });
  });

  it("does not reuse deleted column ids", () => {
    const document = fixture();
    const first = must(addColumn(document, "追加 1"));
    const second = must(addColumn(document, "追加 2"));

    must(removeColumn(document, second));

    expect(must(addColumn(document, "追加 3"))).toBe(second + 1);
    expect(second).toBe(first + 1);
  });

  /// 0 カラムのボードを作らせない。最初にやることが「カラムを作る」になる。
  it("refuses to remove the last column", () => {
    const document = fixture();
    must(removeColumn(document, 1));
    must(removeColumn(document, 2));
    expect(removeColumn(document, 3)).toEqual({ ok: false, error: { kind: "lastColumn" } });
  });

  it("archives a column and keeps the archived cards when the column is deleted", () => {
    const document = fixture();
    const archivedId = cardAt(document, 0, 0).id;
    expect(must(archiveColumn(document, 1))).toBe(2);
    expect(document.board.archivedCards[0]?.id).toBe(archivedId);

    must(removeColumn(document, 1));

    // アーカイブ済みのカードは道連れにしない。残ったカラムを指す。
    expect(document.board.archivedCards[0]?.columnId).toBe(columnAt(document, 0).id);
  });

  /// 何本でも立てられるのが、ボードではなくカラムの属性にした理由（ADR 0038）。
  it("marks columns as the place finished work goes", () => {
    const document = fixture();
    const doneColumn = columnAt(document, 2).id;
    const cancelledColumn = must(addColumn(document, "キャンセル済み"));

    expect(columnAt(document, 2).done).toBe(false);
    expect(must(setColumnDone(document, doneColumn, false))).toBe(false);
    expect(document.undoStack).toHaveLength(1);

    expect(must(setColumnDone(document, doneColumn, true))).toBe(true);
    expect(must(setColumnDone(document, cancelledColumn, true))).toBe(true);
    expect(columnAt(document, 2).done).toBe(true);
    expect(columnAt(document, 3).done).toBe(true);

    expect(setColumnDone(document, 999, true)).toEqual({
      ok: false,
      error: { kind: "columnNotFound", columnId: 999 },
    });
  });
});

describe("renameBoard", () => {
  it("renames a board and rejects empty names", () => {
    const document = fixture();
    expect(must(renameBoard(document, "個人 Kanban"))).toBe(false);
    expect(must(renameBoard(document, "仕事"))).toBe(true);
    expect(document.board.name).toBe("仕事");
    expect(renameBoard(document, "  ")).toEqual({
      ok: false,
      error: { kind: "emptyBoardName" },
    });
  });
});
