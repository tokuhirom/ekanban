// 置いてある形の写しとしての JSON（[ADR 0045]）。
//
// **人が読む Markdown とは別物です。** こちらにはカードの履歴（`cardEvents`）
// まで入り、履歴は置き場所にしかありません。だから組み立てるのも置き場所の側
// です——ブラウザで動いているときの置き場所はここなので、ここに置きます。
//
// **出す文字列は `crates/core/src/export.rs` と 1 バイト違わないこと。** 同じ
// ボードから違うファイルが出ると、書き出したものが置き場所によって別物になり
// ます。`export.test.ts` が、Rust が出した実物と突き合わせます。
//
// [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md

import type { BoardDocument } from "../ipc/types/BoardDocument";
import type { Card } from "../ipc/types/Card";
import type { Recurrence } from "../ipc/types/Recurrence";
import type { StoredCardEvent } from "./types";

/// `serde_json` が出す形に合わせて書く。
///
/// **鍵は名前順**です。Rust 側は `serde_json::Map`（`BTreeMap`）なので、
/// `json!` に並べた順ではなく綴り順で出ます。字下げは 2 つ。
function render(value: unknown, indent: string): string {
  if (value === null) return "null";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (typeof value === "string") return JSON.stringify(value);
  const inner = `${indent}  `;
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    const items = value.map((item) => `${inner}${render(item, inner)}`);
    return `[\n${items.join(",\n")}\n${indent}]`;
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) =>
    a < b ? -1 : a > b ? 1 : 0,
  );
  if (entries.length === 0) return "{}";
  const items = entries.map(([key, item]) => `${inner}${JSON.stringify(key)}: ${render(item, inner)}`);
  return `{\n${items.join(",\n")}\n${indent}}`;
}

function cardJson(card: Card): Record<string, unknown> {
  return {
    id: card.id,
    column_id: card.columnId,
    title: card.title,
    description: card.description,
    position: card.position,
    created_at: card.createdAt,
    updated_at: card.updatedAt,
    due_date: card.dueDate,
    tag_ids: card.tagIds,
    archived_at: card.archivedAt,
    recurrence_id: card.recurrenceId,
    occurrence_date: card.occurrenceDate,
    checklist_items: card.checklistItems.map((item) => ({
      id: item.id,
      card_id: item.cardId,
      text: item.text,
      checked: item.checked,
      position: item.position,
      created_at: item.createdAt,
      updated_at: item.updatedAt,
    })),
  };
}

/// 繰り返しの定義 1 つ。周期は置いてある形ではなく、**形のまま**書きます。
function recurrenceJson(recurrence: Recurrence): Record<string, unknown> {
  const { schedule } = recurrence;
  const rendered =
    schedule.kind === "weekly"
      ? { kind: "weekly", days: schedule.days }
      : schedule.kind === "monthly"
        ? { kind: "monthly", day: schedule.day }
        : { kind: schedule.kind };
  return {
    id: recurrence.id,
    board_id: recurrence.boardId,
    title: recurrence.title,
    description: recurrence.description,
    column_id: recurrence.columnId,
    tag_ids: recurrence.tagIds,
    checklist: recurrence.checklist,
    schedule: rendered,
    lead_days: recurrence.leadDays,
    previous: recurrence.previous,
    enabled: recurrence.enabled,
    last_generated_on: recurrence.lastGeneratedOn,
    created_at: recurrence.createdAt,
    updated_at: recurrence.updatedAt,
  };
}

/// 書き出す JSON。`events` は置き場所が持っているカードの履歴。
export function renderBoardJson(
  document: BoardDocument,
  events: readonly StoredCardEvent[],
): string {
  const { board } = document;
  return render(
    {
      format: "ekanban-board",
      version: 1,
      board: {
        id: board.id,
        name: board.name,
        created_at: board.createdAt,
        updated_at: board.updatedAt,
        next_card_id: document.nextCardId,
        next_column_id: document.nextColumnId,
        next_tag_id: document.nextTagId,
        next_checklist_item_id: document.nextChecklistItemId,
      },
      columns: board.columns.map((column) => ({
        id: column.id,
        board_id: column.boardId,
        name: column.name,
        position: column.position,
        created_at: column.createdAt,
        updated_at: column.updatedAt,
        cards: column.cards.map(cardJson),
      })),
      tags: board.tags.map((tag) => ({
        id: tag.id,
        board_id: tag.boardId,
        name: tag.name,
        color: tag.color,
        created_at: tag.createdAt,
        updated_at: tag.updatedAt,
      })),
      archived_cards: board.archivedCards.map(cardJson),
      recurrences: board.recurrences.map(recurrenceJson),
      card_events: events.map((event) => ({
        id: event.id,
        board_id: board.id,
        card_id: event.cardId,
        kind: event.kind,
        from_column_id: event.fromColumnId,
        to_column_id: event.toColumnId,
        at: event.at,
      })),
    },
    "",
  );
}
