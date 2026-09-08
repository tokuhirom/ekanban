// テストの土台。カードが 2 / 1 / 1 枚入った 3 カラムのボード。
//
// **初回に見えるボードとは別物にしてあります。** 1 つの関数が両方を兼ねると、
// 初回の見た目を直すつもりで中身を変えたときにテストが壊れます
// （`docs/DESIGN.md`「盤面とカード」）。初回のボードを作るのは置き場所の側で、
// こちらは盤面のモデルを確かめるためだけのものです。

import type { Card } from "../ipc/types/Card";
import type { Column } from "../ipc/types/Column";
import type { BoardDocument } from "./board";
import { addCard } from "./board";

/// 空のボード 1 つ。カラムも入っていない。
export function emptyDocument(): BoardDocument {
  const at = 1_700_000_000_000;
  return {
    board: {
      id: 1,
      name: "個人 Kanban",
      createdAt: at,
      updatedAt: at,
      tags: [],
      archivedCards: [],
      columns: [],
    },
    nextCardId: 1,
    nextColumnId: 1,
    nextTagId: 1,
    nextChecklistItemId: 1,
    pendingEvents: [],
    undoStack: [],
    redoStack: [],
  };
}

/// カードの入った 3 カラムのボード。**積まれた操作と履歴は空にして返します**
/// ——土台を組み立てた手順が、テストの Undo に混ざらないように。
export function fixture(): BoardDocument {
  const at = 1_700_000_000_000;
  const document = emptyDocument();
  document.board.columns = [
    { id: 1, boardId: 1, name: "やること", position: 0, createdAt: at, updatedAt: at, done: false, cards: [] },
    { id: 2, boardId: 1, name: "進行中", position: 1, createdAt: at, updatedAt: at, done: false, cards: [] },
    { id: 3, boardId: 1, name: "完了", position: 2, createdAt: at, updatedAt: at, done: false, cards: [] },
  ];
  document.nextColumnId = 4;

  addCard(document, 1, "画面の描画をひととおり通す", "カラムとカードを表示する");
  addCard(document, 1, "D&D の操作を試す", "カードを掴んで移動する");
  addCard(document, 2, "SQLite の設計", "マイグレーションを用意する");
  addCard(document, 3, "README を書く", "プロジェクトの方針をまとめる");
  document.undoStack.length = 0;
  document.redoStack.length = 0;
  document.pendingEvents.length = 0;
  return document;
}

/// 動いたはずのものを取り出す。`!` を書かずに、落ちたときに理由が読めるように。
export function must<T>(outcome: { ok: true; value: T } | { ok: false; error: unknown }): T {
  if (!outcome.ok) throw new Error(`断られた: ${JSON.stringify(outcome.error)}`);
  return outcome.value;
}

/** 読みやすい形。`[[カラム id, [カード id...]], ...]` */
export function shape(document: BoardDocument): [number, number[]][] {
  return document.board.columns.map((column) => [column.id, column.cards.map((card) => card.id)]);
}

/// カラムを添字で取り出す。**`!` を書かないため**の入口（ESLint が禁じている
/// のは、外したときに何が起きたか読めなくなるからです）。
export function columnAt(document: BoardDocument, index: number): Column {
  const column = document.board.columns[index];
  if (column === undefined) throw new Error(`カラム ${String(index)} が無い`);
  return column;
}

/// カードを、カラムの添字とカードの添字で取り出す。
export function cardAt(document: BoardDocument, column: number, card: number): Card {
  const found = columnAt(document, column).cards[card];
  if (found === undefined) throw new Error(`カラム ${String(column)} の ${String(card)} 枚目が無い`);
  return found;
}

/// ID でカードを探す。盤面の上だけを見る（アーカイブは見ない）。
export function cardById(document: BoardDocument, cardId: number): Card {
  const found = document.board.columns
    .flatMap((column) => column.cards)
    .find((card) => card.id === cardId);
  if (found === undefined) throw new Error(`カード ${String(cardId)} が無い`);
  return found;
}

/// 1 つしか無いはずのものを取り出す。数が違えば、そこで落ちて理由が出る。
export function only<T>(items: T[]): T {
  if (items.length !== 1) throw new Error(`1 つのはずが ${String(items.length)} 個ある`);
  const [first] = items;
  if (first === undefined) throw new Error("1 つ目が無い");
  return first;
}
