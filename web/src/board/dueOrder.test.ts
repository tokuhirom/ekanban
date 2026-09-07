import { describe, expect, it } from "vitest";

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { Column } from "../ipc/types/Column";
import type { DueStatus } from "../ipc/types/DueStatus";
import { firstDueCard } from "./dueOrder";

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
    cards: cardIds.map(card),
  };
}

function board(...columns: Column[]): Board {
  return { id: 1, name: "盤面", createdAt: 0, updatedAt: 0, tags: [], archivedCards: [], columns };
}

describe("firstDueCard", () => {
  const b = board(column(10, [1, 2]), column(20, [3, 4]));
  const statuses = new Map<number, DueStatus>([
    [2, { kind: "today" }],
    [3, { kind: "overdue", days: 2 }],
    [4, { kind: "overdue", days: 1 }],
  ]);

  it("カラムの並び、カードの並びで最初の 1 枚を返す", () => {
    expect(firstDueCard(b, statuses, "overdue")).toBe(3);
    expect(firstDueCard(b, statuses, "today")).toBe(2);
  });

  it("期限のないカードは飛ばす", () => {
    expect(firstDueCard(b, new Map([[4, { kind: "overdue", days: 1 }]]), "overdue")).toBe(4);
  });

  it("該当が無ければ何も返さない", () => {
    expect(firstDueCard(b, new Map(), "overdue")).toBeNull();
    expect(firstDueCard(b, statuses, "today")).not.toBeNull();
    expect(firstDueCard(board(column(10, [])), statuses, "overdue")).toBeNull();
  });
});
