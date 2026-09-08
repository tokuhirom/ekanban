// 空の置き場所を開いたときに置く、最初の盤面。
//
// **`Board::first_run` と同じもの**です（`crates/core/src/model.rs`）。置き方が
// 2 つある以上（SQLite とここ）、蒔く側も 2 つになります——盤面の判断ではない
// ので `model/board.ts` には入りません（[ADR 0036]）。
//
// カードは入れません。読み終わったら消す前提のものを最初に置くと、消す手間を
// 全員に配ることになり、消したあともアーカイブか履歴に残ります。カラムだけは
// 置きます。0 カラムだと、最初にやることが「カラムを作る」になって Kanban の
// 形が伝わりません。
//
// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

import type { StoredBoard } from "./types";

export function firstRunBoard(): StoredBoard {
  const at = Date.now();
  return {
    id: 1,
    name: "個人 Kanban",
    createdAt: at,
    updatedAt: at,
    nextCardId: 1,
    nextColumnId: 4,
    nextTagId: 1,
    nextChecklistItemId: 1,
    tags: [],
    archivedCards: [],
    columns: [
      { id: 1, boardId: 1, name: "やること", position: 0, createdAt: at, updatedAt: at, done: false, cards: [] },
      { id: 2, boardId: 1, name: "進行中", position: 1, createdAt: at, updatedAt: at, done: false, cards: [] },
      { id: 3, boardId: 1, name: "完了", position: 2, createdAt: at, updatedAt: at, done: true, cards: [] },
    ],
    events: [],
    rev: 0,
  };
}
