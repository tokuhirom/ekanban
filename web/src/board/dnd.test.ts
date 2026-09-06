// 挿入位置の計算のテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// **ライブラリを外しても残る部分**なので、dnd-kit を通さずに直に確かめます。
// ここで確かめたいのは 2 つ——ドラッグ中の見た目が正しく組み替わること、
// そして Rust の `move_card` に渡す index の**約束**を守っていることです。

import { describe, expect, it } from "vitest";

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { Column } from "../ipc/types/Column";
import { handleId, locateCard, moveCardArgs, moveColumnArgs, parseHandle, previewMove } from "./dnd";

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
  return {
    id: 1,
    name: "盤面",
    createdAt: 0,
    updatedAt: 0,
    tags: [],
    archivedCards: [],
    columns,
  };
}

/// 動いたはずのものを取り出す。`!` を書かずに、落ちたときに理由が読めるように。
function must(value: Board | null): Board {
  if (value === null) throw new Error("動くはずのものが動かなかった");
  return value;
}

/** 読みやすい形。`[[カラム id, [カード id...]], ...]` */
function shape(value: Board): [number, number[]][] {
  return value.columns.map((c) => [c.id, c.cards.map((k) => k.id)]);
}

describe("handle", () => {
  it("カードとカラムの ID を取り違えない", () => {
    // 数値の ID だけだと、id 1 のカードと id 1 のカラムが同じものになる。
    expect(handleId({ kind: "card", id: 1 })).not.toBe(handleId({ kind: "column", id: 1 }));
    expect(parseHandle("card:12")).toEqual({ kind: "card", id: 12 });
    expect(parseHandle("column:3")).toEqual({ kind: "column", id: 3 });
    expect(parseHandle("card:x")).toBeNull();
    expect(parseHandle("mystery:1")).toBeNull();
  });
});

describe("previewMove", () => {
  const b = board(column(10, [1, 2, 3]), column(20, [4]), column(30, []));

  it("同じカラムの中で入れ替える", () => {
    expect(shape(must(previewMove(b, "card:1", "card:3")))).toEqual([
      [10, [2, 3, 1]],
      [20, [4]],
      [30, []],
    ]);
  });

  it("上へ動かすときも、乗っているカードの位置に入る", () => {
    expect(shape(must(previewMove(b, "card:3", "card:1")))).toEqual([
      [10, [3, 1, 2]],
      [20, [4]],
      [30, []],
    ]);
  });

  it("別のカラムのカードの上なら、その位置に割り込む", () => {
    expect(shape(must(previewMove(b, "card:1", "card:4")))).toEqual([
      [10, [2, 3]],
      [20, [1, 4]],
      [30, []],
    ]);
  });

  it("空のカラムの上なら、そのカラムに入る", () => {
    expect(shape(must(previewMove(b, "card:1", "column:30")))).toEqual([
      [10, [2, 3]],
      [20, [4]],
      [30, [1]],
    ]);
  });

  it("いま自分がいるカラムの余白では動かさない", () => {
    // 掴んだだけで末尾に飛ぶと、元の位置に戻せない。
    expect(previewMove(b, "card:1", "column:10")).toBeNull();
  });

  it("カラムはカラムの上でだけ動く", () => {
    expect(shape(must(previewMove(b, "column:30", "column:10")))).toEqual([
      [30, []],
      [10, [1, 2, 3]],
      [20, [4]],
    ]);
    // カードの上に落ちてきても、カラムは動かさない。
    expect(previewMove(b, "column:30", "card:1")).toBeNull();
  });

  it("何も変わらないなら null を返す", () => {
    expect(previewMove(b, "card:1", "card:1")).toBeNull();
    expect(previewMove(b, "column:10", "column:10")).toBeNull();
  });
});

describe("moveCardArgs", () => {
  const b = board(column(10, [1, 2, 3]), column(20, [4]));

  /// Rust の `move_card` は「**動かす前**の列における挿入位置」を受け取り、
  /// 同じカラムの中で後ろへ動かすときだけ自分で 1 引く（`model.rs`）。
  /// ここが 1 ずれると、下へ 1 つ動かしたつもりが動かない。
  it("同じカラムで後ろへ動かすときは 1 足す", () => {
    const after = must(previewMove(b, "card:1", "card:2"));
    expect(locateCard(after, 1)).toEqual({ columnIndex: 0, cardIndex: 1 });
    expect(moveCardArgs(b, after, 1)).toEqual({ toColumnId: 10, toIndex: 2 });
  });

  it("同じカラムで前へ動かすときはそのまま", () => {
    const after = must(previewMove(b, "card:3", "card:1"));
    expect(moveCardArgs(b, after, 3)).toEqual({ toColumnId: 10, toIndex: 0 });
  });

  it("別のカラムへ動かすときはそのまま", () => {
    const after = must(previewMove(b, "card:1", "card:4"));
    expect(moveCardArgs(b, after, 1)).toEqual({ toColumnId: 20, toIndex: 0 });
  });

  it("動いていないなら null（Rust を呼ばない）", () => {
    expect(moveCardArgs(b, b, 1)).toBeNull();
  });
});

describe("moveColumnArgs", () => {
  const b = board(column(10, []), column(20, []), column(30, []));

  it("カラムも同じ約束。後ろへ動かすときだけ 1 足す", () => {
    expect(moveColumnArgs(b, must(previewMove(b, "column:10", "column:30")), 10)).toEqual({
      toIndex: 3,
    });
    expect(moveColumnArgs(b, must(previewMove(b, "column:30", "column:10")), 30)).toEqual({
      toIndex: 0,
    });
    expect(moveColumnArgs(b, b, 10)).toBeNull();
  });
});
