// 掴んだものがどこに落ちるかを決める、純粋な計算。
//
// **ライブラリの外に置いてあります。** ADR 0020 が「外しても自前に落とせる形で
// 使う」と決めたところで、dnd-kit が渡してくるのは「いま何の上にいるか」だけ、
// 挿入位置をどう読むかはこちらの責任です。ライブラリを外す日が来ても、
// 動かす先の計算は残ります。
//
// 盤面の論理は Rust が持っています（ADR 0018）。ここが作る盤面は**ドラッグ中の
// 見た目だけ**で、離した瞬間に `move_card` / `move_column` を 1 回呼び、返って
// きたスナップショットで置き換えます（`docs/DESIGN.md`「ドラッグ＆ドロップ」）。

import type { Board } from "../ipc/types/Board";

/** dnd-kit に渡す ID。数値の ID だけだとカードとカラムが衝突する。 */
export type Handle = { kind: "card"; id: number } | { kind: "column"; id: number };

export function handleId(handle: Handle): string {
  return `${handle.kind}:${handle.id}`;
}

export function parseHandle(id: string | number): Handle | null {
  const [kind = "", rest = ""] = String(id).split(":");
  const value = Number(rest);
  if (!Number.isInteger(value)) return null;
  if (kind === "card") return { kind: "card", id: value };
  if (kind === "column") return { kind: "column", id: value };
  return null;
}

export interface CardLocation {
  columnIndex: number;
  cardIndex: number;
}

export function locateCard(board: Board, cardId: number): CardLocation | null {
  for (const [columnIndex, column] of board.columns.entries()) {
    const cardIndex = column.cards.findIndex((card) => card.id === cardId);
    if (cardIndex !== -1) return { columnIndex, cardIndex };
  }
  return null;
}

function withCardMoved(
  board: Board,
  cardId: number,
  toColumnIndex: number,
  toIndex: number,
): Board {
  const at = locateCard(board, cardId);
  const source = at ? board.columns[at.columnIndex] : undefined;
  const card = source?.cards[at?.cardIndex ?? -1];
  const target = board.columns[toColumnIndex];
  if (at === null || card === undefined || target === undefined) return board;

  const columns = board.columns.map((column) => ({ ...column, cards: [...column.cards] }));
  columns[at.columnIndex]?.cards.splice(at.cardIndex, 1);
  const cards = columns[toColumnIndex]?.cards;
  if (cards === undefined) return board;
  cards.splice(Math.min(toIndex, cards.length), 0, card);
  return { ...board, columns };
}

function withColumnMoved(board: Board, from: number, to: number): Board {
  const columns = [...board.columns];
  const [column] = columns.splice(from, 1);
  if (column === undefined) return board;
  columns.splice(to, 0, column);
  return { ...board, columns };
}

/// 掴んでいるものを、いまポインタが乗っているものの位置へ動かした盤面。
///
/// ドラッグ中の見た目のためだけのものです。何も変わらないなら `null` を返し、
/// 呼ぶ側は描き直しません。
export function previewMove(board: Board, activeId: string, overId: string): Board | null {
  const active = parseHandle(activeId);
  const over = parseHandle(overId);
  if (active === null || over === null) return null;

  if (active.kind === "column") {
    if (over.kind !== "column") return null;
    const from = board.columns.findIndex((column) => column.id === active.id);
    const to = board.columns.findIndex((column) => column.id === over.id);
    if (from === -1 || to === -1 || from === to) return null;
    return withColumnMoved(board, from, to);
  }

  const at = locateCard(board, active.id);
  if (at === null) return null;

  // カラムそのものの上にいるとき（空のカラム、カードの下の余白）は末尾へ。
  // いまいるカラムの余白なら動かさない——掴んだだけで末尾に飛ぶと、戻せない。
  if (over.kind === "column") {
    const toColumnIndex = board.columns.findIndex((column) => column.id === over.id);
    if (toColumnIndex === -1 || toColumnIndex === at.columnIndex) return null;
    return withCardMoved(board, active.id, toColumnIndex, board.columns[toColumnIndex]?.cards.length ?? 0);
  }

  const overAt = locateCard(board, over.id);
  if (overAt === null) return null;
  if (overAt.columnIndex === at.columnIndex && overAt.cardIndex === at.cardIndex) return null;
  return withCardMoved(board, active.id, overAt.columnIndex, overAt.cardIndex);
}

export interface MoveCardArgs {
  toColumnId: number;
  toIndex: number;
}

/// 落ちた先を、Rust の `move_card` が受け取る index に直す。
///
/// **あちらは「動かす前の列における挿入位置」を受け取ります**（`model.rs`）。
/// 同じカラムの中で後ろへ動かすときだけ、抜いたぶんを自分で 1 引きます。
/// こちらが持っているのは動かした**後**の位置なので、その 1 を足し戻します。
/// ここを取り違えると、下へ 1 つ動かしたつもりが動かない。
export function moveCardArgs(
  original: Board,
  moved: Board,
  cardId: number,
): MoveCardArgs | null {
  const before = locateCard(original, cardId);
  const after = locateCard(moved, cardId);
  if (before === null || after === null) return null;

  const toColumn = moved.columns[after.columnIndex];
  if (toColumn === undefined) return null;
  const sameColumn = original.columns[before.columnIndex]?.id === toColumn.id;
  if (sameColumn && before.cardIndex === after.cardIndex) return null;

  const toIndex =
    sameColumn && after.cardIndex > before.cardIndex ? after.cardIndex + 1 : after.cardIndex;
  return { toColumnId: toColumn.id, toIndex };
}

/// カラムの並べ替えも同じ約束。`move_column` も動かす前の index を受け取る。
export function moveColumnArgs(
  original: Board,
  moved: Board,
  columnId: number,
): { toIndex: number } | null {
  const before = original.columns.findIndex((column) => column.id === columnId);
  const after = moved.columns.findIndex((column) => column.id === columnId);
  if (before === -1 || after === -1 || before === after) return null;
  return { toIndex: after > before ? after + 1 : after };
}

/// 掴んでいるものに合わせて、落とし先の候補を絞る。
///
/// カラムの中にカードの `SortableContext` が入れ子になっているので、素のまま
/// だと**カラムを掴んでいるのにカードが落とし先に選ばれます**。そうなると
/// `previewMove` は「カラムをカードの上へ」を拒み、掴んでも何も起きません。
/// dnd-kit を外して自前に書く日が来ても、この絞り込みは要ります。
export function droppableKind(activeId: string): Handle["kind"] | null {
  return parseHandle(activeId)?.kind ?? null;
}
