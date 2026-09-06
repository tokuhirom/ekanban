//! ボードをファイルに書き出す形。
//!
//! JSON は `db::export_board_json`（保存しているものをそのまま出すので、SQL の
//! 近くにある）。Markdown はここです。どちらも UI に依らないので、Tauri のアプリ
//! も開発用のハーネスも同じものを使います（`docs/DESIGN.md`「層の分け方」）。

use crate::model::{Board, Card};

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
