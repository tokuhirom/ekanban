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

// ---------------------------------------------------------------- チェックリスト

/// カードのチェックリストの `position` を、並びのとおりに振り直す。
function reindexChecklist(card: Card): void {
  for (const [index, item] of card.checklistItems.entries()) item.position = index;
}

/// カードの `updatedAt` を進める。チェックリストを触ったときも、カードは動いた。
function touchCard(board: Board, cardId: number, at: number): Card | null {
  const card = findActiveCard(board, cardId);
  if (card === null) return null;
  card.updatedAt = at;
  return card;
}

/// 項目を 1 つ足す。**空の項目は断ります**——空の行を作れという指示なので。
export function addChecklistItem(
  document: BoardDocument,
  cardId: number,
  text: string,
): Outcome<number> {
  if (text.trim() === "") return fail({ kind: "emptyChecklistItemText" });
  const card = findActiveCard(document.board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });

  const at = now();
  const item: ChecklistItem = {
    id: document.nextChecklistItemId,
    cardId,
    text,
    checked: false,
    position: card.checklistItems.length,
    createdAt: at,
    updatedAt: at,
  };
  document.nextChecklistItemId += 1;
  card.checklistItems.push(item);
  reindexChecklist(card);
  card.updatedAt = at;
  document.board.updatedAt = at;
  pushOperation(document, { kind: "addChecklistItem", cardId, item: structuredClone(item) });
  return ok(item.id);
}

export function updateChecklistItem(
  document: BoardDocument,
  cardId: number,
  itemId: number,
  text: string,
): Outcome<boolean> {
  if (text.trim() === "") return fail({ kind: "emptyChecklistItemText" });
  const found = findChecklistItem(document.board, cardId, itemId);
  if (!found.ok) return found;
  const { item } = found.value;
  if (item.text === text) return ok(false);

  const beforeText = item.text;
  const at = now();
  item.text = text;
  item.updatedAt = at;
  touchCard(document.board, cardId, at);
  document.board.updatedAt = at;
  pushOperation(document, { kind: "updateChecklistItem", cardId, itemId, beforeText, afterText: text });
  return ok(true);
}

export function setChecklistItemChecked(
  document: BoardDocument,
  cardId: number,
  itemId: number,
  checked: boolean,
): Outcome<boolean> {
  const found = findChecklistItem(document.board, cardId, itemId);
  if (!found.ok) return found;
  const { item } = found.value;
  if (item.checked === checked) return ok(false);

  const before = item.checked;
  const at = now();
  item.checked = checked;
  item.updatedAt = at;
  touchCard(document.board, cardId, at);
  document.board.updatedAt = at;
  pushOperation(document, { kind: "setChecklistItemChecked", cardId, itemId, before, after: checked });
  return ok(true);
}

export function deleteChecklistItem(
  document: BoardDocument,
  cardId: number,
  itemId: number,
): Outcome<void> {
  const found = findChecklistItem(document.board, cardId, itemId);
  if (!found.ok) return found;
  const { card, index } = found.value;
  const [item] = card.checklistItems.splice(index, 1);
  if (item === undefined) return fail({ kind: "checklistItemNotFound", itemId, cardId });
  reindexChecklist(card);
  const at = now();
  card.updatedAt = at;
  document.board.updatedAt = at;
  pushOperation(document, { kind: "deleteChecklistItem", cardId, item, index });
  return ok(undefined);
}

export function moveChecklistItem(
  document: BoardDocument,
  cardId: number,
  itemId: number,
  targetIndex: number,
): Outcome<boolean> {
  const found = findChecklistItem(document.board, cardId, itemId);
  if (!found.ok) return found;
  const { card, index } = found.value;

  let insertIndex = Math.min(targetIndex, card.checklistItems.length);
  if (index < insertIndex) insertIndex -= 1;
  if (index === insertIndex) return ok(false);

  const [item] = card.checklistItems.splice(index, 1);
  if (item === undefined) return fail({ kind: "checklistItemNotFound", itemId, cardId });
  card.checklistItems.splice(insertIndex, 0, item);
  reindexChecklist(card);
  const at = now();
  card.updatedAt = at;
  document.board.updatedAt = at;
  pushOperation(document, {
    kind: "moveChecklistItem",
    cardId,
    itemId,
    fromIndex: index,
    toIndex: insertIndex,
  });
  return ok(true);
}

function findChecklistItem(
  board: Board,
  cardId: number,
  itemId: number,
): Outcome<{ card: Card; item: ChecklistItem; index: number }> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  const index = card.checklistItems.findIndex((item) => item.id === itemId);
  const item = card.checklistItems[index];
  if (item === undefined) return fail({ kind: "checklistItemNotFound", itemId, cardId });
  return ok({ card, item, index });
}

// ---------------------------------------------------------------- 複製・削除・アーカイブ

/// カードを複製して、すぐ下に置く。
///
/// **期限とチェックは引き継ぎません。** 複製は「同じ形の、これからやること」を
/// 作る操作なので、済んだ印と、過ぎているかもしれない期限は持ち越しません。
export function copyCard(document: BoardDocument, cardId: number): Outcome<number> {
  const { board } = document;
  const location = locateCard(board, cardId);
  if (location === null) return fail({ kind: "cardNotFound", cardId });
  const column = board.columns[location.column];
  const source = column?.cards[location.card];
  if (column === undefined || source === undefined) return fail({ kind: "cardNotFound", cardId });

  const newCardId = document.nextCardId;
  const at = now();
  const checklistItems: ChecklistItem[] = source.checklistItems.map((item, position) => {
    const id = document.nextChecklistItemId;
    document.nextChecklistItemId += 1;
    return {
      id,
      cardId: newCardId,
      text: item.text,
      checked: false,
      position,
      createdAt: at,
      updatedAt: at,
    };
  });
  const card: Card = {
    id: newCardId,
    columnId: source.columnId,
    title: source.title,
    description: source.description,
    position: location.card + 1,
    createdAt: at,
    updatedAt: at,
    dueDate: null,
    tagIds: [...source.tagIds],
    checklistItems,
    archivedAt: null,
  };
  document.nextCardId += 1;
  column.cards.splice(location.card + 1, 0, card);
  reindex(board);
  board.updatedAt = at;
  recordEvent(document, newCardId, "created", null, source.columnId, at);
  pushOperation(document, { kind: "copyCard", card: structuredClone(card), index: location.card + 1 });
  return ok(newCardId);
}

/// 足した直後の、まだ一度も保存していないカードを無かったことにする。
///
/// `deleteCard` とは別のものです。あちらは「あったカードを消す」操作で、履歴に
/// `deleted` を残し、Undo にも積みます。こちらは**追加そのものを取りやめる**ので、
/// `created` の記録ごと取り下げ、Undo にも何も残しません。使う人から見れば、
/// そのカードは一度も存在していません。
export function discardAddedCard(document: BoardDocument, cardId: number): Outcome<void> {
  const { board } = document;
  const location = locateCard(board, cardId);
  if (location === null) return fail({ kind: "cardNotFound", cardId });
  board.columns[location.column]?.cards.splice(location.card, 1);
  reindex(board);

  // 追加を積んだ操作を取り下げる。残すと、取りやめたあとの Undo が
  // 「消えているカードをもう一度消す」ことになって失敗する。
  for (let at = document.undoStack.length - 1; at >= 0; at -= 1) {
    const operation = document.undoStack[at];
    if (operation?.kind === "addCard" && operation.card.id === cardId) {
      document.undoStack.splice(at, 1);
      break;
    }
  }
  // 保存していないので `created` もまだ書かれていない。残すと、次の保存で
  // 存在しないカードの履歴が 1 件だけ書かれる。
  document.pendingEvents = document.pendingEvents.filter(
    (event) => !(event.cardId === cardId && event.kind === "created"),
  );

  // ID は詰めない。採番は単調増加のままにする。
  board.updatedAt = now();
  return ok(undefined);
}

export function deleteCard(document: BoardDocument, cardId: number): Outcome<void> {
  const { board } = document;
  const location = locateCard(board, cardId);
  if (location === null) return fail({ kind: "cardNotFound", cardId });
  const column = board.columns[location.column];
  if (column === undefined) return fail({ kind: "cardNotFound", cardId });

  const columnId = column.id;
  const [card] = column.cards.splice(location.card, 1);
  if (card === undefined) return fail({ kind: "cardNotFound", cardId });
  reindex(board);
  const at = now();
  board.updatedAt = at;
  recordEvent(document, cardId, "deleted", columnId, null, at);
  pushOperation(document, { kind: "deleteCard", card, index: location.card });
  return ok(undefined);
}

export function archiveCard(document: BoardDocument, cardId: number): Outcome<boolean> {
  const { board } = document;
  const location = locateCard(board, cardId);
  if (location === null) return fail({ kind: "cardNotFound", cardId });
  const column = board.columns[location.column];
  if (column === undefined) return fail({ kind: "cardNotFound", cardId });

  const sourceColumnId = column.id;
  const at = now();
  const [card] = column.cards.splice(location.card, 1);
  if (card === undefined) return fail({ kind: "cardNotFound", cardId });
  const original = structuredClone(card);
  card.archivedAt = at;
  card.updatedAt = at;
  board.archivedCards.push(card);
  reindex(board);
  board.updatedAt = at;
  recordEvent(document, cardId, "archived", sourceColumnId, null, at);
  pushOperation(document, {
    kind: "archiveCard",
    card: original,
    archivedCard: structuredClone(card),
    index: location.card,
    archivedIndex: board.archivedCards.length - 1,
  });
  return ok(true);
}

/// カラムのカードをまとめてアーカイブする。積まれる操作は 1 件。
export function archiveColumn(document: BoardDocument, columnId: number): Outcome<number> {
  const { board } = document;
  const column = findColumn(board, columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId });
  if (column.cards.length === 0) return ok(0);

  const at = now();
  const archivedStart = board.archivedCards.length;
  const originals = structuredClone(column.cards);
  const cards = column.cards;
  column.cards = [];
  for (const card of cards) {
    card.archivedAt = at;
    card.updatedAt = at;
    recordEvent(document, card.id, "archived", columnId, null, at);
  }
  board.archivedCards.push(...cards);
  reindex(board);
  board.updatedAt = at;
  pushOperation(document, {
    kind: "archiveColumn",
    columnId,
    cards: originals.map((card, index) => ({
      card,
      archivedCard: structuredClone(cards[index] ?? card),
      index,
    })),
    archivedStart,
  });
  return ok(cards.length);
}

/// アーカイブから盤面へ戻す。戻り先は元のカラムの末尾。
///
/// 元のカラムが消えていたら先頭のカラムへ。**戻せる先が 1 つも無いときだけ**
/// 断ります。
export function restoreCard(document: BoardDocument, cardId: number): Outcome<boolean> {
  const { board } = document;
  const archiveIndex = board.archivedCards.findIndex((card) => card.id === cardId);
  const archived = board.archivedCards[archiveIndex];
  if (archived === undefined) return fail({ kind: "cardNotFound", cardId });

  const target =
    board.columns.find((column) => column.id === archived.columnId) ?? board.columns[0];
  if (target === undefined) return fail({ kind: "lastColumn" });

  const at = now();
  const archivedCard = structuredClone(archived);
  board.archivedCards.splice(archiveIndex, 1);
  archived.columnId = target.id;
  archived.position = target.cards.length;
  archived.archivedAt = null;
  archived.updatedAt = at;
  target.cards.push(archived);
  reindex(board);
  board.updatedAt = at;
  recordEvent(document, cardId, "restored", null, target.id, at);
  pushOperation(document, {
    kind: "restoreCard",
    archivedCard,
    restoredCard: structuredClone(archived),
    index: target.cards.length - 1,
    archiveIndex,
  });
  return ok(true);
}

export function setCardDueDate(
  document: BoardDocument,
  cardId: number,
  dueDate: string | null,
): Outcome<boolean> {
  const card = findActiveCard(document.board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  if (card.dueDate === dueDate) return ok(false);

  const before = card.dueDate;
  const at = now();
  card.dueDate = dueDate;
  card.updatedAt = at;
  document.board.updatedAt = at;
  pushOperation(document, { kind: "setDueDate", cardId, before, after: dueDate });
  return ok(true);
}

// ---------------------------------------------------------------- タグ

/// タグを 1 つ作る。**同じ名前は 2 つ作れません。**
export function addTag(document: BoardDocument, name: string, color: string): Outcome<number> {
  const { board } = document;
  if (name.trim() === "") return fail({ kind: "emptyTagName" });
  if (board.tags.some((tag) => tag.name === name)) return fail({ kind: "duplicateTagName", name });

  const at = now();
  const tag: Tag = {
    id: document.nextTagId,
    boardId: board.id,
    name,
    color,
    createdAt: at,
    updatedAt: at,
  };
  board.tags.push(tag);
  document.nextTagId += 1;
  board.updatedAt = at;
  pushOperation(document, { kind: "addTag", tag: structuredClone(tag) });
  return ok(tag.id);
}

export function renameTag(document: BoardDocument, tagId: number, name: string): Outcome<boolean> {
  const { board } = document;
  if (name.trim() === "") return fail({ kind: "emptyTagName" });
  if (board.tags.some((tag) => tag.id !== tagId && tag.name === name)) {
    return fail({ kind: "duplicateTagName", name });
  }
  const tag = board.tags.find((tag) => tag.id === tagId);
  if (tag === undefined) return fail({ kind: "tagNotFound", tagId });
  if (tag.name === name) return ok(false);

  const before = tag.name;
  const at = now();
  tag.name = name;
  tag.updatedAt = at;
  board.updatedAt = at;
  pushOperation(document, { kind: "renameTag", tagId, before, after: name });
  return ok(true);
}

export function setTagColor(document: BoardDocument, tagId: number, color: string): Outcome<boolean> {
  const { board } = document;
  const tag = board.tags.find((tag) => tag.id === tagId);
  if (tag === undefined) return fail({ kind: "tagNotFound", tagId });
  if (tag.color === color) return ok(false);

  const before = tag.color;
  const at = now();
  tag.color = color;
  tag.updatedAt = at;
  board.updatedAt = at;
  pushOperation(document, { kind: "setTagColor", tagId, before, after: color });
  return ok(true);
}

/// タグを消し、付いていたカードからも外す。
///
/// **外したことを覚えておきます**——Undo で戻すときに、付いていたカードにだけ
/// 付け直すためです。
export function removeTag(document: BoardDocument, tagId: number): Outcome<void> {
  const { board } = document;
  const index = board.tags.findIndex((tag) => tag.id === tagId);
  const tag = board.tags[index];
  if (tag === undefined) return fail({ kind: "tagNotFound", tagId });

  const activeCardTags = allCards(board)
    .filter((card) => card.tagIds.includes(tagId))
    .map((card): [number, number[]] => [card.id, [...card.tagIds]]);
  const archivedCardTags = board.archivedCards
    .filter((card) => card.tagIds.includes(tagId))
    .map((card): [number, number[]] => [card.id, [...card.tagIds]]);

  board.tags.splice(index, 1);
  for (const card of [...allCards(board), ...board.archivedCards]) {
    card.tagIds = card.tagIds.filter((id) => id !== tagId);
  }
  board.updatedAt = now();
  pushOperation(document, { kind: "removeTag", tag, index, activeCardTags, archivedCardTags });
  return ok(undefined);
}

/// カードに付いているタグを、渡された一式に置き換える。
export function setCardTags(
  document: BoardDocument,
  cardId: number,
  tagIds: number[],
): Outcome<boolean> {
  const { board } = document;
  for (const tagId of tagIds) {
    if (!board.tags.some((tag) => tag.id === tagId)) return fail({ kind: "tagNotFound", tagId });
  }
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });

  const after = normalizeTagIds(tagIds);
  if (sameIds(card.tagIds, after)) return ok(false);

  const before = [...card.tagIds];
  const at = now();
  card.tagIds = after;
  card.updatedAt = at;
  board.updatedAt = at;
  pushOperation(document, { kind: "setCardTags", cardId, before, after: [...after] });
  return ok(true);
}

// ---------------------------------------------------------------- カラム

export function addColumn(document: BoardDocument, name: string): Outcome<number> {
  const { board } = document;
  if (name.trim() === "") return fail({ kind: "emptyColumnName" });

  const at = now();
  const column: Column = {
    id: document.nextColumnId,
    boardId: board.id,
    name,
    position: board.columns.length,
    createdAt: at,
    updatedAt: at,
    done: false,
    cards: [],
  };
  board.columns.push(column);
  document.nextColumnId += 1;
  board.updatedAt = at;
  pushOperation(document, {
    kind: "addColumn",
    column: structuredClone(column),
    index: board.columns.length - 1,
  });
  return ok(column.id);
}

export function renameColumn(
  document: BoardDocument,
  columnId: number,
  name: string,
): Outcome<boolean> {
  const { board } = document;
  if (name.trim() === "") return fail({ kind: "emptyColumnName" });
  const column = findColumn(board, columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId });
  if (column.name === name) return ok(false);

  const before = column.name;
  const at = now();
  column.name = name;
  column.updatedAt = at;
  board.updatedAt = at;
  pushOperation(document, { kind: "renameColumn", columnId, before, after: name });
  return ok(true);
}

/// 終わったものの置き場かどうかを切り替える（[ADR 0038]）。
///
/// **何本立てても構いません。** 「完了」と「キャンセル済み」を並べて両方立てる
/// のが、ボードの属性ではなくカラムの属性にした理由です。
///
/// [ADR 0038]: ../../../docs/adr/0038-a-column-that-means-done.md
export function setColumnDone(
  document: BoardDocument,
  columnId: number,
  done: boolean,
): Outcome<boolean> {
  const { board } = document;
  const column = findColumn(board, columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId });
  if (column.done === done) return ok(false);

  const before = column.done;
  const at = now();
  column.done = done;
  column.updatedAt = at;
  board.updatedAt = at;
  pushOperation(document, { kind: "setColumnDone", columnId, before, after: done });
  return ok(true);
}

/// カラムを消す。中のカードも一緒に消える。
///
/// **最後の 1 本は消せません。** 0 カラムのボードは、次にやることが「カラムを
/// 作る」になり、Kanban の形が画面から消えます。
///
/// アーカイブ済みのカードは消しません——このカラムを指していたものは、残った
/// カラムを指すように付け替えます。消すと、盤面に無いだけのカードが道連れに
/// なります。
export function removeColumn(document: BoardDocument, columnId: number): Outcome<void> {
  const { board } = document;
  const index = board.columns.findIndex((column) => column.id === columnId);
  const column = board.columns[index];
  if (column === undefined) return fail({ kind: "columnNotFound", columnId });
  if (board.columns.length === 1) return fail({ kind: "lastColumn" });

  const fallback = board.columns.find((other) => other.id !== columnId);
  if (fallback === undefined) return fail({ kind: "lastColumn" });
  const fallbackColumnId = fallback.id;

  const archivedCardColumnIds = board.archivedCards
    .filter((card) => card.columnId === columnId)
    .map((card): [number, number] => [card.id, card.columnId]);
  const deletedCardIds = column.cards.map((card) => card.id);

  const removed = structuredClone(column);
  board.columns.splice(index, 1);
  const at = now();
  for (const card of board.archivedCards) {
    if (card.columnId === columnId) {
      card.columnId = fallbackColumnId;
      card.updatedAt = at;
    }
  }
  reindex(board);
  board.updatedAt = at;
  for (const cardId of deletedCardIds) {
    recordEvent(document, cardId, "deleted", columnId, null, at);
  }
  pushOperation(document, {
    kind: "removeColumn",
    column: removed,
    index,
    fallbackColumnId,
    archivedCardColumnIds,
  });
  return ok(undefined);
}

// ---------------------------------------------------------------- ボード

/// ボードの名前を変える。**空の名前は断ります。**
export function renameBoard(document: BoardDocument, name: string): Outcome<boolean> {
  if (name.trim() === "") return fail({ kind: "emptyBoardName" });
  if (document.board.name === name) return ok(false);
  document.board.name = name;
  document.board.updatedAt = now();
  return ok(true);
}

/// 次の保存で書く履歴を捨てる。保存に失敗したときに、盤面と一緒に巻き戻す。
export function discardPendingEvents(document: BoardDocument): void {
  document.pendingEvents.length = 0;
}

// ---------------------------------------------------------------- 取り消しとやり直し

export function canUndo(document: BoardDocument): boolean {
  return document.undoStack.length > 0;
}

export function canRedo(document: BoardDocument): boolean {
  return document.redoStack.length > 0;
}

/// 1 手戻す。戻せなければ `false`（断りではない）。
///
/// **戻せなかったときは積み直します。** 失敗した操作をスタックから落とすと、
/// そこから先の履歴が二度と辿れなくなります。
export function undo(document: BoardDocument): Outcome<boolean> {
  const operation = document.undoStack.pop();
  if (operation === undefined) return ok(false);
  const applied = applyOperation(document, operation, true);
  if (!applied.ok) {
    document.undoStack.push(operation);
    return applied;
  }
  document.redoStack.push(operation);
  return ok(true);
}

export function redo(document: BoardDocument): Outcome<boolean> {
  const operation = document.redoStack.pop();
  if (operation === undefined) return ok(false);
  const applied = applyOperation(document, operation, false);
  if (!applied.ok) {
    document.redoStack.push(operation);
    return applied;
  }
  document.undoStack.push(operation);
  return ok(true);
}

/// 積んだ操作を、戻す向き（`undo`）か進める向きで当てる。
///
/// **履歴（`pendingEvents`）は積みません。** 取り消しはフローの出来事ではなく、
/// 打ち間違いの取り消しです（`docs/DESIGN.md`「盤面とカード」）。
///
/// 採番は戻しません——`nextCardId` は戻したカードの ID より必ず先へ進めます。
/// 詰めてしまうと、やり直したときに同じ ID が 2 枚のカードに付きます。
function applyOperation(
  document: BoardDocument,
  operation: BoardOperation,
  undoing: boolean,
): Outcome<void> {
  const { board } = document;
  let step: Outcome<unknown> = ok(undefined);

  switch (operation.kind) {
    case "moveCard": {
      step = undoing
        ? moveCardRaw(board, operation.cardId, operation.fromColumnId, operation.fromIndex)
        : moveCardRaw(board, operation.cardId, operation.toColumnId, operation.toIndex);
      break;
    }
    case "moveColumn": {
      step = moveColumnRaw(
        board,
        operation.columnId,
        undoing ? operation.fromIndex : operation.toIndex,
      );
      break;
    }
    case "addCard": {
      if (undoing) {
        step = removeActiveCard(board, operation.card.id);
      } else {
        step = insertActiveCard(board, structuredClone(operation.card), operation.card.position);
        document.nextCardId = Math.max(document.nextCardId, operation.card.id + 1);
      }
      break;
    }
    case "updateCard": {
      step = updateCardRaw(
        board,
        operation.cardId,
        undoing ? operation.beforeTitle : operation.afterTitle,
        undoing ? operation.beforeDescription : operation.afterDescription,
      );
      break;
    }
    case "editCard": {
      const content = undoing ? operation.before : operation.after;
      step = updateCardRaw(board, operation.cardId, content.title, content.description);
      if (step.ok) step = setDueDateRaw(board, operation.cardId, content.dueDate);
      if (step.ok) step = setCardTagsRaw(board, operation.cardId, content.tagIds);
      if (step.ok) {
        step = setChecklistItemsRaw(board, operation.cardId, structuredClone(content.checklistItems));
      }
      break;
    }
    case "copyCard": {
      if (undoing) {
        step = removeActiveCard(board, operation.card.id);
      } else {
        step = insertActiveCard(board, structuredClone(operation.card), operation.index);
        document.nextCardId = Math.max(document.nextCardId, operation.card.id + 1);
        for (const item of operation.card.checklistItems) {
          document.nextChecklistItemId = Math.max(document.nextChecklistItemId, item.id + 1);
        }
      }
      break;
    }
    case "addChecklistItem": {
      if (undoing) {
        step = removeChecklistItemRaw(board, operation.item.cardId, operation.item.id);
      } else {
        step = insertChecklistItemRaw(
          board,
          operation.item.cardId,
          structuredClone(operation.item),
          Number.MAX_SAFE_INTEGER,
        );
        document.nextChecklistItemId = Math.max(
          document.nextChecklistItemId,
          operation.item.id + 1,
        );
      }
      break;
    }
    case "updateChecklistItem": {
      step = updateChecklistItemRaw(
        board,
        operation.cardId,
        operation.itemId,
        undoing ? operation.beforeText : operation.afterText,
      );
      break;
    }
    case "setChecklistItemChecked": {
      step = setChecklistItemCheckedRaw(
        board,
        operation.cardId,
        operation.itemId,
        undoing ? operation.before : operation.after,
      );
      break;
    }
    case "deleteChecklistItem": {
      step = undoing
        ? insertChecklistItemRaw(
            board,
            operation.cardId,
            structuredClone(operation.item),
            operation.index,
          )
        : removeChecklistItemRaw(board, operation.cardId, operation.item.id);
      break;
    }
    case "moveChecklistItem": {
      step = moveChecklistItemRaw(
        board,
        operation.cardId,
        undoing ? operation.toIndex : operation.fromIndex,
        undoing ? operation.fromIndex : operation.toIndex,
      );
      break;
    }
    case "deleteCard": {
      step = undoing
        ? insertActiveCard(board, structuredClone(operation.card), operation.index)
        : removeActiveCard(board, operation.card.id);
      break;
    }
    case "archiveCard": {
      if (undoing) {
        step = removeArchivedCard(board, operation.card.id);
        if (step.ok) step = insertActiveCard(board, structuredClone(operation.card), operation.index);
      } else {
        step = removeActiveCard(board, operation.card.id);
        if (step.ok) {
          const index = Math.min(operation.archivedIndex, board.archivedCards.length);
          board.archivedCards.splice(index, 0, structuredClone(operation.archivedCard));
        }
      }
      reindex(board);
      break;
    }
    case "archiveColumn": {
      for (const entry of operation.cards) {
        if (!step.ok) break;
        if (undoing) {
          step = removeArchivedCard(board, entry.card.id);
          if (step.ok) step = insertActiveCard(board, structuredClone(entry.card), entry.index);
        } else {
          step = removeActiveCard(board, entry.card.id);
          if (step.ok) {
            const index = Math.min(
              operation.archivedStart + entry.index,
              board.archivedCards.length,
            );
            board.archivedCards.splice(index, 0, structuredClone(entry.archivedCard));
          }
        }
      }
      reindex(board);
      break;
    }
    case "restoreCard": {
      if (undoing) {
        step = removeActiveCard(board, operation.restoredCard.id);
        if (step.ok) {
          const index = Math.min(operation.archiveIndex, board.archivedCards.length);
          board.archivedCards.splice(index, 0, structuredClone(operation.archivedCard));
        }
      } else {
        step = removeArchivedCard(board, operation.archivedCard.id);
        if (step.ok) {
          step = insertActiveCard(board, structuredClone(operation.restoredCard), operation.index);
        }
      }
      reindex(board);
      break;
    }
    case "setDueDate": {
      step = setDueDateRaw(board, operation.cardId, undoing ? operation.before : operation.after);
      break;
    }
    case "addTag": {
      if (undoing) {
        step = removeTagRaw(board, operation.tag.id);
      } else {
        board.tags.push(structuredClone(operation.tag));
        document.nextTagId = Math.max(document.nextTagId, operation.tag.id + 1);
      }
      break;
    }
    case "renameTag": {
      step = renameTagRaw(board, operation.tagId, undoing ? operation.before : operation.after);
      break;
    }
    case "setTagColor": {
      step = setTagColorRaw(board, operation.tagId, undoing ? operation.before : operation.after);
      break;
    }
    case "removeTag": {
      if (undoing) {
        board.tags.splice(operation.index, 0, structuredClone(operation.tag));
        restoreCardTags(board, operation.activeCardTags);
        restoreCardTags(board, operation.archivedCardTags);
      } else {
        step = removeTagRaw(board, operation.tag.id);
      }
      break;
    }
    case "setCardTags": {
      step = setCardTagsRaw(board, operation.cardId, undoing ? operation.before : operation.after);
      break;
    }
    case "addColumn": {
      if (undoing) {
        step = removeColumnRaw(board, operation.column.id);
      } else {
        board.columns.splice(operation.index, 0, structuredClone(operation.column));
        document.nextColumnId = Math.max(document.nextColumnId, operation.column.id + 1);
        reindex(board);
      }
      break;
    }
    case "renameColumn": {
      step = renameColumnRaw(
        board,
        operation.columnId,
        undoing ? operation.before : operation.after,
      );
      break;
    }
    case "setColumnDone": {
      step = setColumnDoneRaw(
        board,
        operation.columnId,
        undoing ? operation.before : operation.after,
      );
      break;
    }
    case "removeColumn": {
      if (undoing) {
        board.columns.splice(operation.index, 0, structuredClone(operation.column));
        for (const [cardId, columnId] of operation.archivedCardColumnIds) {
          const card = board.archivedCards.find((card) => card.id === cardId);
          if (card !== undefined) card.columnId = columnId;
        }
      } else {
        step = removeColumnRaw(board, operation.column.id);
        for (const [cardId] of operation.archivedCardColumnIds) {
          const card = board.archivedCards.find((card) => card.id === cardId);
          if (card !== undefined) card.columnId = operation.fallbackColumnId;
        }
      }
      reindex(board);
      break;
    }
  }

  if (!step.ok) return step;
  board.updatedAt = now();
  return ok(undefined);
}

// 以下は「積んだ操作を当てる」ためだけの書き換えです。**断りの検査も、操作の
// 積み直しも、履歴も付けません**——それは一度通った判断で、二度目に通す必要が
// ないからです。

function removeActiveCard(board: Board, cardId: number): Outcome<Card> {
  const location = locateCard(board, cardId);
  if (location === null) return fail({ kind: "cardNotFound", cardId });
  const [card] = board.columns[location.column]?.cards.splice(location.card, 1) ?? [];
  if (card === undefined) return fail({ kind: "cardNotFound", cardId });
  reindex(board);
  return ok(card);
}

function insertActiveCard(board: Board, card: Card, index: number): Outcome<void> {
  const column = findColumn(board, card.columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId: card.columnId });
  card.archivedAt = null;
  column.cards.splice(Math.min(index, column.cards.length), 0, card);
  reindex(board);
  return ok(undefined);
}

function removeArchivedCard(board: Board, cardId: number): Outcome<Card> {
  const index = board.archivedCards.findIndex((card) => card.id === cardId);
  const [card] = board.archivedCards.splice(index === -1 ? board.archivedCards.length : index, 1);
  if (card === undefined) return fail({ kind: "cardNotFound", cardId });
  return ok(card);
}

function moveCardRaw(
  board: Board,
  cardId: number,
  targetColumnId: number,
  targetIndex: number,
): Outcome<void> {
  const removed = removeActiveCard(board, cardId);
  if (!removed.ok) return removed;
  removed.value.columnId = targetColumnId;
  return insertActiveCard(board, removed.value, targetIndex);
}

function moveColumnRaw(board: Board, columnId: number, targetIndex: number): Outcome<void> {
  const sourceIndex = board.columns.findIndex((column) => column.id === columnId);
  if (sourceIndex === -1) return fail({ kind: "columnNotFound", columnId });
  const [column] = board.columns.splice(sourceIndex, 1);
  if (column === undefined) return fail({ kind: "columnNotFound", columnId });
  board.columns.splice(Math.min(targetIndex, board.columns.length), 0, column);
  reindex(board);
  return ok(undefined);
}

function updateCardRaw(
  board: Board,
  cardId: number,
  title: string,
  description: string,
): Outcome<void> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  card.title = title;
  card.description = description;
  card.updatedAt = now();
  return ok(undefined);
}

function setDueDateRaw(board: Board, cardId: number, dueDate: string | null): Outcome<void> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  card.dueDate = dueDate;
  card.updatedAt = now();
  return ok(undefined);
}

function setCardTagsRaw(board: Board, cardId: number, tagIds: number[]): Outcome<void> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  card.tagIds = [...tagIds];
  card.updatedAt = now();
  return ok(undefined);
}

function setChecklistItemsRaw(
  board: Board,
  cardId: number,
  items: ChecklistItem[],
): Outcome<void> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  card.checklistItems = items;
  for (const item of card.checklistItems) item.cardId = card.id;
  reindexChecklist(card);
  return ok(undefined);
}

function insertChecklistItemRaw(
  board: Board,
  cardId: number,
  item: ChecklistItem,
  index: number,
): Outcome<void> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  item.cardId = cardId;
  card.checklistItems.splice(Math.min(index, card.checklistItems.length), 0, item);
  reindexChecklist(card);
  return ok(undefined);
}

function removeChecklistItemRaw(board: Board, cardId: number, itemId: number): Outcome<void> {
  const found = findChecklistItem(board, cardId, itemId);
  if (!found.ok) return found;
  found.value.card.checklistItems.splice(found.value.index, 1);
  reindexChecklist(found.value.card);
  return ok(undefined);
}

function moveChecklistItemRaw(
  board: Board,
  cardId: number,
  fromIndex: number,
  targetIndex: number,
): Outcome<void> {
  const card = findActiveCard(board, cardId);
  if (card === null) return fail({ kind: "cardNotFound", cardId });
  if (fromIndex >= card.checklistItems.length) {
    return fail({ kind: "checklistItemNotFound", itemId: fromIndex, cardId });
  }
  const [item] = card.checklistItems.splice(fromIndex, 1);
  if (item === undefined) return fail({ kind: "checklistItemNotFound", itemId: fromIndex, cardId });
  card.checklistItems.splice(Math.min(targetIndex, card.checklistItems.length), 0, item);
  reindexChecklist(card);
  return ok(undefined);
}

function updateChecklistItemRaw(
  board: Board,
  cardId: number,
  itemId: number,
  text: string,
): Outcome<void> {
  const found = findChecklistItem(board, cardId, itemId);
  if (!found.ok) return found;
  found.value.item.text = text;
  found.value.item.updatedAt = now();
  return ok(undefined);
}

function setChecklistItemCheckedRaw(
  board: Board,
  cardId: number,
  itemId: number,
  checked: boolean,
): Outcome<void> {
  const found = findChecklistItem(board, cardId, itemId);
  if (!found.ok) return found;
  found.value.item.checked = checked;
  found.value.item.updatedAt = now();
  return ok(undefined);
}

function removeTagRaw(board: Board, tagId: number): Outcome<void> {
  const index = board.tags.findIndex((tag) => tag.id === tagId);
  if (index === -1) return fail({ kind: "tagNotFound", tagId });
  board.tags.splice(index, 1);
  for (const card of [...allCards(board), ...board.archivedCards]) {
    card.tagIds = card.tagIds.filter((id) => id !== tagId);
  }
  return ok(undefined);
}

function restoreCardTags(board: Board, assignments: [number, number[]][]): void {
  for (const [cardId, tagIds] of assignments) {
    const card =
      findActiveCard(board, cardId) ?? board.archivedCards.find((card) => card.id === cardId) ?? null;
    if (card !== null) card.tagIds = [...tagIds];
  }
}

function renameTagRaw(board: Board, tagId: number, name: string): Outcome<void> {
  const tag = board.tags.find((tag) => tag.id === tagId);
  if (tag === undefined) return fail({ kind: "tagNotFound", tagId });
  tag.name = name;
  tag.updatedAt = now();
  return ok(undefined);
}

function setTagColorRaw(board: Board, tagId: number, color: string): Outcome<void> {
  const tag = board.tags.find((tag) => tag.id === tagId);
  if (tag === undefined) return fail({ kind: "tagNotFound", tagId });
  tag.color = color;
  tag.updatedAt = now();
  return ok(undefined);
}

function renameColumnRaw(board: Board, columnId: number, name: string): Outcome<void> {
  const column = findColumn(board, columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId });
  column.name = name;
  column.updatedAt = now();
  return ok(undefined);
}

function setColumnDoneRaw(board: Board, columnId: number, done: boolean): Outcome<void> {
  const column = findColumn(board, columnId);
  if (column === null) return fail({ kind: "columnNotFound", columnId });
  column.done = done;
  column.updatedAt = now();
  return ok(undefined);
}

function removeColumnRaw(board: Board, columnId: number): Outcome<void> {
  const index = board.columns.findIndex((column) => column.id === columnId);
  if (index === -1) return fail({ kind: "columnNotFound", columnId });
  board.columns.splice(index, 1);
  reindex(board);
  return ok(undefined);
}
