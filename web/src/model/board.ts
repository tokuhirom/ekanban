// 盤面のモデル（[ADR 0039]）。
//
// **ここが盤面の唯一の実装になります。** 採番、`position` の振り直し、
// アーカイブ、タグ、チェックリスト、カードの履歴の生成——盤面についての判断は
// すべてここにあり、置き場所（Rust）は受け取ったものを検めて書くだけです
// （[ADR 0040]）。
//
// **操作は文書を書き換えます。** 呼ぶ側が写しを取ってから渡し、成功したもの
// だけを画面に載せてください。断られた操作は文書に触れません——検めるものを
// 先に全部見てから書き換える、という順で書いてあります。
//
// 時刻は `Date.now()`（epoch ミリ秒）。**期限だけは別**で、`"YYYY-MM-DD"` の
// 文字列のまま扱い、基準日は引数で受けます（`docs/DESIGN.md`「境界を越える値」、
// `web/src/model/due.ts`）。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { ChecklistItem } from "../ipc/types/ChecklistItem";
import type { Column } from "../ipc/types/Column";
import type { Tag } from "../ipc/types/Tag";

/// 盤面と、それに付いて回るもの。
///
/// **`board` だけが画面へ出ます。** 採番の続きは置き場所と一緒に運ばれ
/// （[ADR 0040]）、Undo のスタックと積まれた履歴はここにしかありません。
///
/// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
export interface BoardDocument {
  board: Board;
  /** 次に振るカードの番号。ボードごとの名前空間の中で進む。 */
  nextCardId: number;
  nextColumnId: number;
  nextTagId: number;
  nextChecklistItemId: number;
  /** 次の保存で書き、書けたら捨てるカードの履歴。 */
  pendingEvents: CardEvent[];
  /** 取り消せる操作。**セッションの間だけ**で、保存しない。 */
  undoStack: BoardOperation[];
  redoStack: BoardOperation[];
}

/** カードのライフサイクル。カラム内の並べ替えと属性変更は残さない。 */
export type CardEventKind = "created" | "moved" | "archived" | "restored" | "deleted";

export interface CardEvent {
  cardId: number;
  kind: CardEventKind;
  fromColumnId: number | null;
  toColumnId: number | null;
  at: number;
}

/// 断る理由。**例外にしません**——入力欄の脇に出すものと、画面を更新すれば
/// 直るものが混ざっており、呼ぶ側が行き先を選びます（[ADR 0016]）。
///
/// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md
export type BoardError =
  | { kind: "emptyBoardName" }
  | { kind: "columnNotFound"; columnId: number }
  | { kind: "cardNotFound"; cardId: number }
  | { kind: "emptyCardTitle" }
  | { kind: "emptyColumnName" }
  | { kind: "emptyTagName" }
  | { kind: "tagNotFound"; tagId: number }
  | { kind: "duplicateTagName"; name: string }
  | { kind: "emptyChecklistItemText" }
  | { kind: "checklistItemNotFound"; itemId: number; cardId: number }
  | { kind: "lastColumn" };

/** 操作の結果。`false` は「変わらなかった」——断られたのとは違う。 */
export type Outcome<T> = { ok: true; value: T } | { ok: false; error: BoardError };

export function ok<T>(value: T): Outcome<T> {
  return { ok: true, value };
}

export function fail<T>(error: BoardError): Outcome<T> {
  return { ok: false, error };
}

/** チェックリストの下書き。`id` は保存済みの項目を指すときだけ入る。 */
export interface ChecklistItemDraft {
  id: number | null;
  text: string;
  checked: boolean;
}

/// 取り消せる操作。**やり直しのために、変える前と後の両方を持ちます。**
///
/// 消したカードは `Card` を丸ごと抱えます——ID だけだと、戻すときに作り直す
/// ことになり、作り直したものは元と同じではありません（履歴も採番も動く）。
export type BoardOperation =
  | {
      kind: "moveCard";
      cardId: number;
      fromColumnId: number;
      fromIndex: number;
      toColumnId: number;
      toIndex: number;
    }
  | { kind: "moveColumn"; columnId: number; fromIndex: number; toIndex: number }
  | { kind: "addCard"; card: Card }
  | {
      kind: "updateCard";
      cardId: number;
      beforeTitle: string;
      beforeDescription: string;
      afterTitle: string;
      afterDescription: string;
    }
  | {
      kind: "editCard";
      cardId: number;
      before: CardContent;
      after: CardContent;
    }
  | { kind: "copyCard"; card: Card; index: number }
  | { kind: "addChecklistItem"; cardId: number; item: ChecklistItem }
  | {
      kind: "updateChecklistItem";
      cardId: number;
      itemId: number;
      beforeText: string;
      afterText: string;
    }
  | {
      kind: "setChecklistItemChecked";
      cardId: number;
      itemId: number;
      before: boolean;
      after: boolean;
    }
  | { kind: "deleteChecklistItem"; cardId: number; item: ChecklistItem; index: number }
  | {
      kind: "moveChecklistItem";
      cardId: number;
      itemId: number;
      fromIndex: number;
      toIndex: number;
    }
  | { kind: "deleteCard"; card: Card; index: number }
  | {
      kind: "archiveCard";
      card: Card;
      archivedCard: Card;
      index: number;
      archivedIndex: number;
    }
  | {
      kind: "archiveColumn";
      columnId: number;
      cards: ArchivedCardOperation[];
      archivedStart: number;
    }
  | {
      kind: "restoreCard";
      archivedCard: Card;
      restoredCard: Card;
      index: number;
      archiveIndex: number;
    }
  | { kind: "setDueDate"; cardId: number; before: string | null; after: string | null }
  | { kind: "addTag"; tag: Tag }
  | { kind: "renameTag"; tagId: number; before: string; after: string }
  | { kind: "setTagColor"; tagId: number; before: string; after: string }
  | {
      kind: "removeTag";
      tag: Tag;
      index: number;
      activeCardTags: [number, number[]][];
      archivedCardTags: [number, number[]][];
    }
  | { kind: "setCardTags"; cardId: number; before: number[]; after: number[] }
  | { kind: "addColumn"; column: Column; index: number }
  | { kind: "renameColumn"; columnId: number; before: string; after: string }
  | { kind: "setColumnDone"; columnId: number; before: boolean; after: boolean }
  | {
      kind: "removeColumn";
      column: Column;
      index: number;
      fallbackColumnId: number;
      archivedCardColumnIds: [number, number][];
    };

/** 編集パネルの 1 回の確定で変わるもの。`editCard` が前後の両方を抱える。 */
export interface CardContent {
  title: string;
  description: string;
  dueDate: string | null;
  tagIds: number[];
  checklistItems: ChecklistItem[];
}

export interface ArchivedCardOperation {
  card: Card;
  archivedCard: Card;
  index: number;
}

// ---------------------------------------------------------------- 土台

/** epoch ミリ秒。`db` の `now()` と同じ単位（`docs/DESIGN.md`「境界を越える値」）。 */
function now(): number {
  return Date.now();
}

/** 深い写し。操作は文書を書き換えるので、呼ぶ側はこれを通してから渡す。 */
export function cloneDocument(document: BoardDocument): BoardDocument {
  return structuredClone(document);
}

function allCards(board: Board): Card[] {
  return board.columns.flatMap((column) => column.cards);
}

function locateCard(board: Board, cardId: number): { column: number; card: number } | null {
  for (const [column, entry] of board.columns.entries()) {
    const card = entry.cards.findIndex((card) => card.id === cardId);
    if (card !== -1) return { column, card };
  }
  return null;
}

function findActiveCard(board: Board, cardId: number): Card | null {
  return allCards(board).find((card) => card.id === cardId) ?? null;
}

function findColumn(board: Board, columnId: number): Column | null {
  return board.columns.find((column) => column.id === columnId) ?? null;
}

/// カラムとカードの `position` を、並びのとおりに振り直す。
///
/// **見た目の位置と `position` を食い違わせない**ための 1 か所（`docs/DESIGN.md`
/// 「盤面とカード」）。並べ替えたあとに必ず通します。
function reindex(board: Board): void {
  for (const [columnIndex, column] of board.columns.entries()) {
    column.position = columnIndex;
    for (const [cardIndex, card] of column.cards.entries()) {
      card.position = cardIndex;
      card.columnId = column.id;
    }
  }
}

function recordEvent(
  document: BoardDocument,
  cardId: number,
  kind: CardEventKind,
  fromColumnId: number | null,
  toColumnId: number | null,
  at: number,
): void {
  document.pendingEvents.push({ cardId, kind, fromColumnId, toColumnId, at });
}

/// 操作を積む。**新しい操作はやり直しの履歴を捨てます**——枝分かれした履歴を
/// 持つと、「やり直す」が何を指すのか画面から読めなくなります。
function pushOperation(document: BoardDocument, operation: BoardOperation): void {
  document.undoStack.push(operation);
  document.redoStack.length = 0;
}

/** 同じ ID を 2 つ持たない、昇順のタグの並びにする。 */
function normalizeTagIds(tagIds: number[]): number[] {
  return [...new Set(tagIds)].sort((left, right) => left - right);
}

// ---------------------------------------------------------------- カード

/// カードを、別のカラムか同じカラムの別の位置へ動かす。
///
/// **落とす位置を決めるのは呼ぶ側**です（`web/src/board/dnd.ts`、[ADR 0020]）。
/// ここは受け取った位置に入れ、`position` を振り直します。
///
/// `targetIndex` は「動かす前の並びで数えた、入れたい位置」です。同じカラムの
/// 中で下へ動かすときは、抜いたぶん 1 つ手前になります。
///
/// [ADR 0020]: ../../../docs/adr/0020-pointer-based-drag-and-drop.md
export function moveCard(
  document: BoardDocument,
  cardId: number,
  targetColumnId: number,
  targetIndex: number,
): Outcome<boolean> {
  const { board } = document;
  const source = locateCard(board, cardId);
  if (source === null) return fail({ kind: "cardNotFound", cardId });
  const targetColumnIndex = board.columns.findIndex((column) => column.id === targetColumnId);
  if (targetColumnIndex === -1) {
    return fail({ kind: "columnNotFound", columnId: targetColumnId });
  }

  const sameColumn = source.column === targetColumnIndex;
  const sourceColumn = board.columns[source.column];
  const targetColumn = board.columns[targetColumnIndex];
  if (sourceColumn === undefined || targetColumn === undefined) {
    return fail({ kind: "columnNotFound", columnId: targetColumnId });
  }
  const sourceColumnId = sourceColumn.id;

  let insertIndex = Math.min(targetIndex, targetColumn.cards.length);
  if (sameColumn && source.card < insertIndex) insertIndex -= 1;
  if (sameColumn && source.card === insertIndex) return ok(false);

  const at = now();
  const [card] = sourceColumn.cards.splice(source.card, 1);
  if (card === undefined) return fail({ kind: "cardNotFound", cardId });
  card.columnId = targetColumnId;
  card.updatedAt = at;

  insertIndex = Math.min(insertIndex, targetColumn.cards.length);
  targetColumn.cards.splice(insertIndex, 0, card);
  reindex(board);
  board.updatedAt = at;
  pushOperation(document, {
    kind: "moveCard",
    cardId,
    fromColumnId: sourceColumnId,
    fromIndex: source.card,
    toColumnId: targetColumnId,
    toIndex: insertIndex,
  });
  // **カラムをまたいだときだけ履歴に残します**（`docs/DESIGN.md`「盤面とカード」）。
  // カラム内の並べ替えはライフサイクルの変化ではありません。
  if (!sameColumn) {
    recordEvent(document, cardId, "moved", sourceColumnId, targetColumnId, at);
  }
  return ok(true);
}

/// カラムそのものを並べ替える。
export function moveColumn(
  document: BoardDocument,
  columnId: number,
  targetIndex: number,
): Outcome<boolean> {
  const { board } = document;
  const sourceIndex = board.columns.findIndex((column) => column.id === columnId);
  if (sourceIndex === -1) return fail({ kind: "columnNotFound", columnId });

  let insertIndex = Math.min(targetIndex, board.columns.length);
  if (sourceIndex < insertIndex) insertIndex -= 1;
  if (sourceIndex === insertIndex) return ok(false);

  const [column] = board.columns.splice(sourceIndex, 1);
  if (column === undefined) return fail({ kind: "columnNotFound", columnId });
  const at = now();
  column.updatedAt = at;
  insertIndex = Math.min(insertIndex, board.columns.length);
  board.columns.splice(insertIndex, 0, column);
  reindex(board);
  board.updatedAt = at;
  pushOperation(document, { kind: "moveColumn", columnId, fromIndex: sourceIndex, toIndex: insertIndex });
  return ok(true);
}

/// カードを 1 枚足す。中身は空のまま。
export function addCard(
  document: BoardDocument,
  columnId: number,
  title: string,
  description: string,
): Outcome<number> {
  return addCardWithDetails(document, columnId, title, description, null, [], []);
}

/// 期限・タグ・チェックリストまで備えたカードを 1 回で足す（#127）。
///
/// **足してから書き換えるのではなく、備えた状態で作ります。** 2 回に分けると
/// 操作が 2 件積まれ、足したばかりのカードを取り消すのに Undo が 2 回要ります。
///
/// 名前が空のチェックリスト項目は落とします。渡された下書きの `id` は見ません
/// ——**まだ存在しないカードに既存の項目はありえない**ので、すべて新しい項目
/// として採番します。
export function addCardWithDetails(
  document: BoardDocument,
  columnId: number,
  title: string,
  description: string,
  dueDate: string | null,
  tagIds: number[],
  checklistDrafts: ChecklistItemDraft[],
): Outcome<number> {
  const { board } = document;
  // カラムが無いとき、タグが無いときは、何も変えずに断る。採番も進めない。
  const column = findColumn(board, columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId });
  for (const tagId of tagIds) {
    if (!board.tags.some((tag) => tag.id === tagId)) return fail({ kind: "tagNotFound", tagId });
  }

  const id = document.nextCardId;
  const at = now();
  const checklistItems: ChecklistItem[] = [];
  for (const draft of checklistDrafts) {
    if (draft.text.trim() === "") continue;
    checklistItems.push({
      id: document.nextChecklistItemId,
      cardId: id,
      text: draft.text,
      checked: draft.checked,
      position: checklistItems.length,
      createdAt: at,
      updatedAt: at,
    });
    document.nextChecklistItemId += 1;
  }

  const card: Card = {
    id,
    columnId,
    title,
    description,
    position: column.cards.length,
    createdAt: at,
    updatedAt: at,
    dueDate,
    tagIds: normalizeTagIds(tagIds),
    checklistItems,
    archivedAt: null,
  };
  column.cards.push(card);
  document.nextCardId += 1;
  recordEvent(document, id, "created", null, columnId, at);
  pushOperation(document, { kind: "addCard", card: structuredClone(card) });
  board.updatedAt = at;
  return ok(id);
}

/// タイトルと説明だけを書き換える。
export function updateCard(
  document: BoardDocument,
  cardId: number,
  title: string,
  description: string,
): Outcome<boolean> {
  if (title.trim() === "") return fail({ kind: "emptyCardTitle" });
  const card = findActiveCard(document.board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  if (card.title === title && card.description === description) return ok(false);

  const beforeTitle = card.title;
  const beforeDescription = card.description;
  const at = now();
  card.title = title;
  card.description = description;
  card.updatedAt = at;
  document.board.updatedAt = at;
  pushOperation(document, {
    kind: "updateCard",
    cardId,
    beforeTitle,
    beforeDescription,
    afterTitle: title,
    afterDescription: description,
  });
  return ok(true);
}

/// 編集パネルの 1 回の確定（[ADR 0032]）。**積まれる操作は 1 件**です。
///
/// チェックリストは「このカードのすべて」を受け取ります。**項目名が空のものは
/// 落とします**（#114）——`＋ 項目を追加` で作った行を消すまでカードごと保存
/// できない、という状態を作らないためです。1 項目ずつの操作は、空の項目を
/// 作れという指示なので今までどおり断ります。
///
/// [ADR 0032]: ../../../docs/adr/0032-committing-a-card-field-by-field.md
export function updateCardDetails(
  document: BoardDocument,
  cardId: number,
  title: string,
  description: string,
  dueDate: string | null,
  tagIds: number[],
  checklistDrafts: ChecklistItemDraft[],
): Outcome<boolean> {
  const { board } = document;
  if (title.trim() === "") return fail({ kind: "emptyCardTitle" });
  for (const tagId of tagIds) {
    if (!board.tags.some((tag) => tag.id === tagId)) return fail({ kind: "tagNotFound", tagId });
  }
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });

  const drafts = checklistDrafts.filter((draft) => draft.text.trim() !== "");
  for (const draft of drafts) {
    if (draft.id !== null && !card.checklistItems.some((item) => item.id === draft.id)) {
      return fail({ kind: "checklistItemNotFound", itemId: draft.id, cardId });
    }
  }

  const at = now();
  const before: CardContent = {
    title: card.title,
    description: card.description,
    dueDate: card.dueDate,
    tagIds: [...card.tagIds],
    checklistItems: structuredClone(card.checklistItems),
  };

  const checklistItems: ChecklistItem[] = drafts.map((draft, position) => {
    const existing =
      draft.id === null ? undefined : card.checklistItems.find((item) => item.id === draft.id);
    let id: number;
    if (existing === undefined) {
      id = document.nextChecklistItemId;
      document.nextChecklistItemId += 1;
    } else {
      id = existing.id;
    }
    // 触っていない項目の `updatedAt` は動かしません。並べ替えただけの項目が
    // 「いま直したもの」に見えないように。
    const changed = existing?.text !== draft.text || existing.checked !== draft.checked;
    return {
      id,
      cardId,
      text: draft.text,
      checked: draft.checked,
      position,
      createdAt: existing?.createdAt ?? at,
      updatedAt: changed ? at : existing.updatedAt,
    };
  });

  const normalizedTagIds = normalizeTagIds(tagIds);
  if (
    card.title === title &&
    card.description === description &&
    card.dueDate === dueDate &&
    sameIds(card.tagIds, normalizedTagIds) &&
    sameChecklist(card.checklistItems, checklistItems)
  ) {
    return ok(false);
  }

  card.title = title;
  card.description = description;
  card.dueDate = dueDate;
  card.tagIds = normalizedTagIds;
  card.checklistItems = checklistItems;
  card.updatedAt = at;
  board.updatedAt = at;
  pushOperation(document, {
    kind: "editCard",
    cardId,
    before,
    after: {
      title,
      description,
      dueDate,
      tagIds: normalizedTagIds,
      checklistItems: structuredClone(checklistItems),
    },
  });
  return ok(true);
}

function sameIds(left: number[], right: number[]): boolean {
  return left.length === right.length && left.every((id, at) => id === right[at]);
}

function sameChecklist(left: ChecklistItem[], right: ChecklistItem[]): boolean {
  return (
    left.length === right.length &&
    left.every((item, at) => {
      const other = right[at];
      return (
        other?.id === item.id &&
        other.cardId === item.cardId &&
        other.text === item.text &&
        other.checked === item.checked &&
        other.position === item.position &&
        other.createdAt === item.createdAt &&
        other.updatedAt === item.updatedAt
      );
    })
  );
}
