//! ボードをファイルに書き出す形。
//!
//! JSON も Markdown もここです。どちらも UI に依らないので、Tauri のアプリも
//! 開発用のハーネスもブラウザ版も同じものを使います（`docs/DESIGN.md`
//! 「層の分け方」）。
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

pub fn render_board_markdown(board: &Board) -> String {
    let mut markdown = format!("# {}\n\n", markdown_inline(&board.name));
    for column in &board.columns {
        markdown.push_str(&format!("## {}\n\n", markdown_inline(&column.name)));
        if column.cards.is_empty() {
            markdown.push_str("カードはありません。\n\n");
            continue;
        }
        for card in &column.cards {
            append_markdown_card(&mut markdown, card, board, None);
        }
    }

    if !board.archived_cards.is_empty() {
        markdown.push_str("## アーカイブ\n\n");
        for card in &board.archived_cards {
            let column_name = board
                .columns
                .iter()
                .find(|column| column.id == card.column_id)
                .map(|column| column.name.as_str());
            append_markdown_card(&mut markdown, card, board, column_name);
        }
    }

    markdown
}

pub fn suggested_export_name(board_name: &str, extension: &str) -> String {
    let stem = board_name
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    let stem = stem.trim().trim_matches('.');
    let stem = if stem.is_empty() { "board" } else { stem };
    format!("{stem}.{extension}")
}

fn append_markdown_card(
    markdown: &mut String,
    card: &Card,
    board: &Board,
    column_name: Option<&str>,
) {
    markdown.push_str(&format!("- **{}**\n", markdown_inline(&card.title)));

    let mut metadata = Vec::new();
    if let Some(column_name) = column_name {
        metadata.push(format!("カラム: {}", markdown_inline(column_name)));
    }
    if let Some(due_date) = card.due_date {
        metadata.push(format!("期限: {due_date}"));
    }
    let tag_names = card
        .tag_ids
        .iter()
        .filter_map(|tag_id| board.tags.iter().find(|tag| tag.id == *tag_id))
        .map(|tag| markdown_inline(&tag.name))
        .collect::<Vec<_>>();
    if !tag_names.is_empty() {
        metadata.push(format!("タグ: {}", tag_names.join(", ")));
    }
    if card.archived_at.is_some() {
        metadata.push("アーカイブ済み".to_string());
    }
    for line in metadata {
        markdown.push_str(&format!("  - {line}\n"));
    }

    if !card.description.trim().is_empty() {
        for line in card.description.lines() {
            markdown.push_str(&format!("  > {}\n", markdown_inline(line)));
        }
    }
    for item in &card.checklist_items {
        let marker = if item.checked { 'x' } else { ' ' };
        markdown.push_str(&format!("  - [{marker}] {}\n", markdown_inline(&item.text)));
    }
    markdown.push('\n');
}

fn markdown_inline(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(['\r', '\n'], " ")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

#[cfg(test)]
mod tests {
    use super::{markdown_inline, render_board_markdown, suggested_export_name};
    use crate::model::Board;

    #[test]
    fn escapes_markdown_syntax_inside_the_text_the_user_typed() {
        // 書き出した Markdown を読む道具が、カードの中身を見出しや強調として
        // 解釈しないこと。改行も潰す（1 行の中に収めるための関数なので）。
        assert_eq!(
            markdown_inline("*強調* _と_ `コード`"),
            "\\*強調\\* \\_と\\_ \\`コード\\`"
        );
        assert_eq!(markdown_inline("1 行目\n2 行目"), "1 行目 2 行目");
        assert_eq!(markdown_inline("[link]"), "\\[link\\]");
    }

    #[test]
    fn builds_a_file_name_that_the_file_system_accepts() {
        assert_eq!(suggested_export_name("個人 Kanban", "md"), "個人 Kanban.md");
        assert_eq!(suggested_export_name("a/b\\c", "json"), "a_b_c.json");
        assert_eq!(suggested_export_name("  ...  ", "md"), "board.md");
        assert_eq!(suggested_export_name("", "json"), "board.json");
    }

    #[test]
    fn writes_every_column_and_the_archive() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;
        board.archive_card(card_id).expect("the card is archived");

        let markdown = render_board_markdown(&board);

        assert!(markdown.starts_with("# 個人 Kanban\n\n"));
        for column in &board.columns {
            assert!(
                markdown.contains(&format!("## {}\n", column.name)),
                "カラム「{}」が出ていない",
                column.name
            );
        }
        assert!(markdown.contains("## アーカイブ\n"));
        assert!(
            markdown.contains("アーカイブ済み"),
            "アーカイブしたカードには、そうと分かる印が要る"
        );
    }

    #[test]
    fn says_so_when_a_column_has_no_cards() {
        let mut board = Board::first_run();
        board.add_column("空のカラム").expect("a column is added");
        assert!(render_board_markdown(&board).contains("カードはありません。"));
    }
}
