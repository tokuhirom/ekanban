//! ボードを JSON に書き出す形。
//!
//! **ここにあるのは JSON だけです**（[ADR 0045]）。JSON は**置いてある形の
//! 写し**で、カードの履歴（`card_events`）まで入ります。履歴は置き場所にしか
//! 無いので、組み立てられるのはこちら側だけです。人が読む Markdown のほうは
//! webview にあります（`web/src/model/export.ts`）。
//!
//! [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
//!
//! **JSON の組み立てが置き場所の側にありません**（[ADR 0042]）。置き場所は
//! 2 つあり（SQLite と JSON、`store.rs`）、組み立てをそれぞれが持つと、同じ
//! ボードから違うファイルが出ます。置き場所から受け取るのはカードの履歴だけで、
//! 残りは `Board` から出します。
//!
//! [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Card, ChecklistItem, Column, Tag};

    /// 書き出しの形を 1 か所に固定する（[ADR 0041]、[ADR 0042]）。
    ///
    /// **置き場所は 2 つあります**——配るアプリの SQLite と、ブラウザで動く
    /// ときの `web/src/store/`。同じボードから違うファイルが出ると、書き出した
    /// ものが置き場所によって別物になります。両側がこの 1 つのファイルと突き
    /// 合わせるので、どちらかがずれた日に落ちます（`web/src/store/export.test.ts`）。
    ///
    /// [ADR 0041]: ../../../docs/adr/0041-one-layer-of-screen-tests.md
    /// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
    #[test]
    fn writes_the_json_both_stores_have_to_agree_on() {
        let at = 1_700_000_000_000;
        let board = Board {
            id: 1,
            name: "個人 Kanban".to_string(),
            created_at: at,
            updated_at: at + 1,
            next_card_id: 4,
            next_column_id: 3,
            next_tag_id: 2,
            next_checklist_item_id: 3,
            tags: vec![Tag {
                id: 1,
                board_id: 1,
                name: "設計".to_string(),
                color: "#8b5cf6".to_string(),
                created_at: at,
                updated_at: at,
            }],
            archived_cards: vec![Card {
                id: 3,
                column_id: 1,
                title: "しまったカード".to_string(),
                description: String::new(),
                position: 0,
                created_at: at,
                updated_at: at,
                due_date: None,
                tag_ids: Vec::new(),
                checklist_items: Vec::new(),
                archived_at: Some(at + 2),
            }],
            columns: vec![
                Column {
                    id: 1,
                    board_id: 1,
                    name: "やること".to_string(),
                    position: 0,
                    created_at: at,
                    updated_at: at,
                    done: false,
                    cards: vec![Card {
                        id: 1,
                        column_id: 1,
                        title: "期限つきのカード".to_string(),
                        description: "説明\nの 2 行目".to_string(),
                        position: 0,
                        created_at: at,
                        updated_at: at,
                        due_date: chrono::NaiveDate::from_ymd_opt(2026, 12, 24),
                        tag_ids: vec![1],
                        checklist_items: vec![
                            ChecklistItem {
                                id: 1,
                                card_id: 1,
                                text: "済んだ項目".to_string(),
                                checked: true,
                                position: 0,
                                created_at: at,
                                updated_at: at,
                            },
                            ChecklistItem {
                                id: 2,
                                card_id: 1,
                                text: "まだの項目".to_string(),
                                checked: false,
                                position: 1,
                                created_at: at,
                                updated_at: at,
                            },
                        ],
                        archived_at: None,
                    }],
                },
                Column {
                    id: 2,
                    board_id: 1,
                    name: "完了".to_string(),
                    position: 1,
                    created_at: at,
                    updated_at: at,
                    done: true,
                    cards: vec![Card {
                        id: 2,
                        column_id: 2,
                        title: "終わったカード".to_string(),
                        description: String::new(),
                        position: 0,
                        created_at: at,
                        updated_at: at,
                        due_date: None,
                        tag_ids: Vec::new(),
                        checklist_items: Vec::new(),
                        archived_at: None,
                    }],
                },
            ],
            pending_events: Vec::new(),
        };
        let events = vec![
            StoredCardEvent {
                id: 1,
                card_id: 1,
                kind: "created".to_string(),
                from_column_id: None,
                to_column_id: Some(1),
                at,
            },
            StoredCardEvent {
                id: 2,
                card_id: 3,
                kind: "archived".to_string(),
                from_column_id: Some(1),
                to_column_id: None,
                at: at + 2,
            },
        ];

        let written = render_board_json(&board, &events).expect("the board renders");
        // 見たいのは中身であって行末ではない。`.gitattributes` が LF に固定して
        // いるが、それが外れた checkout（Windows の既定は CRLF）で落ちると、
        // 突き合わせの差分からは理由が読み取れないので、読んだ時点でそろえる。
        let expected =
            include_str!("../../../web/src/store/export.fixture.json").replace("\r\n", "\n");
        assert_eq!(written, expected.trim_end_matches('\n'));
    }
}
