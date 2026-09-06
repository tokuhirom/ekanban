// キーボードでの選択と移動のテスト。
// （`docs/DESIGN.md`「ドラッグ＆ドロップ」の受け入れ条件、「テスト」の「部品」）
//
// gpui 版の `next_card_id` / `move_selected_card_between_columns` と同じ動きに
// なっていることを確かめます。**手触りが変わらないことが移行の条件**なので、
// 「もっと素直な動き」に直したくなったら、まずこのテストを見てください。

import { describe, expect, it } from "vitest";

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { Column } from "../ipc/types/Column";
import { keyboardMove, movesSelectedCard, nextSelection } from "./keyboard";

function card(id: number): Card {
  return {
    id,
    columnId: 0,
    title: `カード ${id}`,
    description: "",
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    dueDate: null,
    tagIds: [],
    checklistItems: [],
    archivedAt: null,
  };
}

function column(id: number, cardIds: number[]): Column {
  return {
    id,
    boardId: 1,
    name: `カラム ${id}`,
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    wipLimit: null,
    cards: cardIds.map(card),
  };
}

function board(...columns: Column[]): Board {
  return { id: 1, name: "盤面", createdAt: 0, updatedAt: 0, tags: [], archivedCards: [], columns };
}

describe("nextSelection", () => {
  const b = board(column(10, [1, 2, 3]), column(20, []), column(30, [4, 5]));

  it("何も選んでいなければ先頭のカード", () => {
    expect(nextSelection(b, null, "down")).toBe(1);
    expect(nextSelection(b, null, "right")).toBe(1);
  });

  it("上下は同じカラムの中を動き、端では止まる", () => {
    expect(nextSelection(b, 1, "down")).toBe(2);
    expect(nextSelection(b, 2, "up")).toBe(1);
    expect(nextSelection(b, 1, "up")).toBeNull();
    expect(nextSelection(b, 3, "down")).toBeNull();
  });

  /// 空のカラムで止まらない。止まると、そこから先へ行く手段が無くなる。
  it("左右は、カードのある次のカラムまで飛び越す", () => {
    expect(nextSelection(b, 1, "right")).toBe(4);
    expect(nextSelection(b, 4, "left")).toBe(1);
    expect(nextSelection(b, 4, "right")).toBeNull();
  });

  it("行が足りないカラムへ移るときは、そのカラムの最後のカード", () => {
    expect(nextSelection(b, 3, "right")).toBe(5);
  });

  it("消えたカードを選んだままなら、先頭に戻す", () => {
    expect(nextSelection(b, 999, "down")).toBe(1);
  });
});

describe("keyboardMove", () => {
  const b = board(column(10, [1, 2, 3]), column(20, [4]));

  /// index は「動かす前の列における挿入位置」（`dnd.ts` と同じ約束）。
  /// 下へ 1 つで `+2` になるのはそのためで、書き間違いではない。
  it("下は +2、上は -1", () => {
    expect(keyboardMove(b, 1, "down")).toEqual({ toColumnId: 10, toIndex: 2 });
    expect(keyboardMove(b, 2, "up")).toEqual({ toColumnId: 10, toIndex: 0 });
  });

  it("左右は隣のカラムの、同じ行（足りなければ末尾）", () => {
    expect(keyboardMove(b, 3, "right")).toEqual({ toColumnId: 20, toIndex: 1 });
    expect(keyboardMove(b, 4, "left")).toEqual({ toColumnId: 10, toIndex: 0 });
  });

  it("端のカラムから外へは動かさない", () => {
    expect(keyboardMove(b, 1, "left")).toBeNull();
    expect(keyboardMove(b, 4, "right")).toBeNull();
  });

  it("盤面にないカードは動かさない", () => {
    expect(keyboardMove(b, 999, "down")).toBeNull();
  });
});

describe("movesSelectedCard", () => {
  /// どの修飾キーが `secondary` かは、Rust が返す platform で決める。
  /// UA を見ていたころ、Playwright の Safari 模擬（Linux 上で `Macintosh` を
  /// 名乗る）で割り当てが丸ごと効かなくなった。
  function press(init: Partial<KeyboardEvent>): KeyboardEvent {
    return { ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...init } as KeyboardEvent;
  }

  it("macOS は Cmd + Alt", () => {
    expect(movesSelectedCard(press({ metaKey: true, altKey: true }), "macos")).toBe(true);
    expect(movesSelectedCard(press({ ctrlKey: true, altKey: true }), "macos")).toBe(false);
  });

  it("ほかの OS は Ctrl + Alt", () => {
    expect(movesSelectedCard(press({ ctrlKey: true, altKey: true }), "linux")).toBe(true);
    expect(movesSelectedCard(press({ ctrlKey: true, altKey: true }), "windows")).toBe(true);
    expect(movesSelectedCard(press({ metaKey: true, altKey: true }), "linux")).toBe(false);
  });

  it("ほかの修飾キーが混ざっていたら、別の割り当てに譲る", () => {
    expect(
      movesSelectedCard(press({ ctrlKey: true, altKey: true, shiftKey: true }), "linux"),
    ).toBe(false);
    expect(
      movesSelectedCard(press({ ctrlKey: true, altKey: true, metaKey: true }), "linux"),
    ).toBe(false);
  });
});
