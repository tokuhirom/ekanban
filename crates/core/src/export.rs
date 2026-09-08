//! ボードを JSON に書き出す形。
//!
//! **ここにあるのは JSON だけです**（[ADR 0045]）。JSON は**置いてある形の
//! 写し**で、採番の続き（`next_card_id` ほか）のように、境界を越えて画面へ
//! 出さない値まで入ります。だから組み立てられるのは置き場所の側だけです。
//! 人が読む Markdown のほうは webview にあります（`web/src/model/export.ts`）。
//!
//! [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
//!
//! **JSON の組み立てが置き場所の側にありません**（[ADR 0036]）。置き場所は
//! 2 つあり（SQLite と JSON、`store.rs`）、組み立てをそれぞれが持つと、同じ
//! ボードから違うファイルが出ます。置き場所から受け取るのはカードの履歴だけで、
//! 残りは `Board` から出します。
//!
//! [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

use serde_json::json;

use crate::model::{Board, Card};
use crate::store::{StoreError, StoredCardEvent};

/// 書き出す JSON。`events` は置き場所が持っているカードの履歴。
pub fn render_board_json(board: &Board, events: &[StoredCardEvent]) -> Result<String, StoreError> {
    let events = events
        .iter()
        .map(|event| {
            json!({
                "id": event.id,
                "board_id": board.id,
                "card_id": event.card_id,
                "kind": event.kind,
                "from_column_id": event.from_column_id,
                "to_column_id": event.to_column_id,
                "at": event.at,
            })
        })
        .collect::<Vec<_>>();

    let card_json = |card: &Card| {
        json!({
            "id": card.id,
            "column_id": card.column_id,
            "title": card.title,
            "description": card.description,
            "position": card.position,
            "created_at": card.created_at,
            "updated_at": card.updated_at,
            "due_date": card.due_date.map(|date| date.format("%Y-%m-%d").to_string()),
            "tag_ids": card.tag_ids,
            "archived_at": card.archived_at,
            "checklist_items": card.checklist_items.iter().map(|item| json!({
                "id": item.id,
                "card_id": item.card_id,
                "text": item.text,
                "checked": item.checked,
                "position": item.position,
                "created_at": item.created_at,
                "updated_at": item.updated_at,
            })).collect::<Vec<_>>(),
        })
    };
    let columns = board
        .columns
        .iter()
        .map(|column| {
            json!({
                "id": column.id,
                "board_id": column.board_id,
                "name": column.name,
                "position": column.position,
                "created_at": column.created_at,
                "updated_at": column.updated_at,
                "cards": column.cards.iter().map(card_json).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let tags = board
        .tags
        .iter()
        .map(|tag| {
            json!({
                "id": tag.id,
                "board_id": tag.board_id,
                "name": tag.name,
                "color": tag.color,
                "created_at": tag.created_at,
                "updated_at": tag.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let archived_cards = board
        .archived_cards
        .iter()
        .map(card_json)
        .collect::<Vec<_>>();

    serde_json::to_string_pretty(&json!({
        "format": "ekanban-board",
        "version": 1,
        "board": {
            "id": board.id,
            "name": board.name,
            "created_at": board.created_at,
            "updated_at": board.updated_at,
            "next_card_id": board.next_card_id,
            "next_column_id": board.next_column_id,
            "next_tag_id": board.next_tag_id,
            "next_checklist_item_id": board.next_checklist_item_id,
        },
        "columns": columns,
        "tags": tags,
        "archived_cards": archived_cards,
        "card_events": events,
    }))
    .map_err(StoreError::from)
}
