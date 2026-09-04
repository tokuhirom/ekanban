use thiserror::Error;

pub type BoardId = i64;
pub type ColumnId = i64;
pub type CardId = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: CardId,
    pub column_id: ColumnId,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub id: ColumnId,
    pub board_id: BoardId,
    pub name: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub columns: Vec<Column>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BoardError {
    #[error("column {0} was not found")]
    ColumnNotFound(ColumnId),
    #[error("card {0} was not found")]
    CardNotFound(CardId),
    #[error("a card title cannot be empty")]
    EmptyCardTitle,
    #[error("a board must have at least one column")]
    LastColumn,
}

impl Board {
    pub fn demo() -> Self {
        let now = timestamp();
        let mut board = Self {
            id: 1,
            name: "個人 Kanban".to_string(),
            created_at: now,
            updated_at: now,
            columns: vec![
                Column::new(1, 1, "やること", 0, now),
                Column::new(2, 1, "進行中", 1, now),
                Column::new(3, 1, "完了", 2, now),
            ],
        };

        board
            .add_card(1, "GPUI の画面を作る", "カラムとカードを表示する")
            .expect("demo column exists");
        board
            .add_card(1, "D&D の操作を試す", "カードを掴んで移動する")
            .expect("demo column exists");
        board
            .add_card(2, "SQLite の設計", "マイグレーションを用意する")
            .expect("demo column exists");
        board
            .add_card(3, "README を書く", "プロジェクトの方針をまとめる")
            .expect("demo column exists");
        board
    }

    pub fn move_card(
        &mut self,
        card_id: CardId,
        target_column_id: ColumnId,
        target_index: usize,
    ) -> Result<bool, BoardError> {
        let (source_column_index, source_card_index) = self
            .columns
            .iter()
            .enumerate()
            .find_map(|(column_index, column)| {
                column
                    .cards
                    .iter()
                    .position(|card| card.id == card_id)
                    .map(|card_index| (column_index, card_index))
            })
            .ok_or(BoardError::CardNotFound(card_id))?;

        let target_column_index = self
            .columns
            .iter()
            .position(|column| column.id == target_column_id)
            .ok_or(BoardError::ColumnNotFound(target_column_id))?;

        let same_column = source_column_index == target_column_index;
        let mut insert_index = target_index.min(self.columns[target_column_index].cards.len());
        if same_column && source_card_index < insert_index {
            insert_index -= 1;
        }

        if same_column && source_card_index == insert_index {
            return Ok(false);
        }

        let card = self.columns[source_column_index]
            .cards
            .remove(source_card_index);
        let card = Card {
            column_id: target_column_id,
            updated_at: timestamp(),
            ..card
        };

        insert_index = insert_index.min(self.columns[target_column_index].cards.len());
        self.columns[target_column_index]
            .cards
            .insert(insert_index, card);
        self.reindex();
        self.updated_at = timestamp();
        Ok(true)
    }

    pub fn move_column(
        &mut self,
        column_id: ColumnId,
        target_index: usize,
    ) -> Result<bool, BoardError> {
        let source_index = self
            .columns
            .iter()
            .position(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;

        let mut insert_index = target_index.min(self.columns.len());
        if source_index < insert_index {
            insert_index -= 1;
        }

        if source_index == insert_index {
            return Ok(false);
        }

        let mut column = self.columns.remove(source_index);
        column.updated_at = timestamp();
        insert_index = insert_index.min(self.columns.len());
        self.columns.insert(insert_index, column);
        self.reindex();
        self.updated_at = timestamp();
        Ok(true)
    }

    pub fn add_card(
        &mut self,
        column_id: ColumnId,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<CardId, BoardError> {
        let id = self
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .map(|card| card.id)
            .max()
            .unwrap_or(0)
            + 1;
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        let now = timestamp();
        column.cards.push(Card {
            id,
            column_id,
            title: title.into(),
            description: description.into(),
            position: column.cards.len() as i64,
            created_at: now,
            updated_at: now,
        });
        self.updated_at = now;
        Ok(id)
    }

    pub fn update_card(
        &mut self,
        card_id: CardId,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<bool, BoardError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(BoardError::EmptyCardTitle);
        }
        let description = description.into();
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;

        if card.title == title && card.description == description {
            return Ok(false);
        }

        let now = timestamp();
        card.title = title;
        card.description = description;
        card.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn remove_card(&mut self, card_id: CardId) -> Result<(), BoardError> {
        let (column_index, card_index) = self
            .columns
            .iter()
            .enumerate()
            .find_map(|(column_index, column)| {
                column
                    .cards
                    .iter()
                    .position(|card| card.id == card_id)
                    .map(|card_index| (column_index, card_index))
            })
            .ok_or(BoardError::CardNotFound(card_id))?;

        self.columns[column_index].cards.remove(card_index);
        self.reindex();
        self.updated_at = timestamp();
        Ok(())
    }

    pub fn add_column(&mut self, name: impl Into<String>) -> ColumnId {
        let id = self
            .columns
            .iter()
            .map(|column| column.id)
            .max()
            .unwrap_or(0)
            + 1;
        let now = timestamp();
        self.columns.push(Column::new(
            id,
            self.id,
            name,
            self.columns.len() as i64,
            now,
        ));
        self.updated_at = now;
        id
    }

    pub fn remove_column(&mut self, column_id: ColumnId) -> Result<(), BoardError> {
        if self.columns.len() == 1 {
            return Err(BoardError::LastColumn);
        }
        let index = self
            .columns
            .iter()
            .position(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        self.columns.remove(index);
        self.reindex();
        self.updated_at = timestamp();
        Ok(())
    }

    fn reindex(&mut self) {
        for (column_index, column) in self.columns.iter_mut().enumerate() {
            column.position = column_index as i64;
            for (card_index, card) in column.cards.iter_mut().enumerate() {
                card.position = card_index as i64;
                card.column_id = column.id;
            }
        }
    }
}

impl Column {
    fn new(
        id: ColumnId,
        board_id: BoardId,
        name: impl Into<String>,
        position: i64,
        now: i64,
    ) -> Self {
        Self {
            id,
            board_id,
            name: name.into(),
            position,
            created_at: now,
            updated_at: now,
            cards: Vec::new(),
        }
    }
}

fn timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::{Board, BoardError};

    #[test]
    fn moves_card_to_another_column() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;

        assert!(board.move_card(card_id, 2, 0).unwrap());
        assert_eq!(board.columns[0].cards.len(), 1);
        assert_eq!(board.columns[1].cards[0].id, card_id);
        assert_eq!(board.columns[1].cards[0].column_id, 2);
    }

    #[test]
    fn reorders_card_inside_a_column() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;

        assert!(board.move_card(card_id, 1, 2).unwrap());
        assert_eq!(board.columns[0].cards[0].title, "D&D の操作を試す");
        assert_eq!(board.columns[0].cards[1].id, card_id);
    }

    #[test]
    fn moving_a_card_to_its_current_position_is_a_noop() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;

        assert!(!board.move_card(card_id, 1, 0).unwrap());
        assert_eq!(board.columns[0].cards[0].id, card_id);
    }

    #[test]
    fn reorders_columns() {
        let mut board = Board::demo();

        assert!(board.move_column(1, board.columns.len()).unwrap());
        assert_eq!(
            board
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["進行中", "完了", "やること"]
        );
        assert_eq!(board.columns[2].position, 2);
    }

    #[test]
    fn moving_a_column_to_its_current_position_is_a_noop() {
        let mut board = Board::demo();

        assert!(!board.move_column(2, 1).unwrap());
        assert_eq!(board.columns[1].id, 2);
    }

    #[test]
    fn rejects_unknown_card_and_column() {
        let mut board = Board::demo();
        assert_eq!(
            board.move_card(999, 1, 0),
            Err(BoardError::CardNotFound(999))
        );
        let card_id = board.columns[0].cards[0].id;
        assert_eq!(
            board.move_card(card_id, 999, 0),
            Err(BoardError::ColumnNotFound(999))
        );
    }

    #[test]
    fn updates_card_content() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;

        assert!(board
            .update_card(card_id, "更新したタイトル", "更新した説明")
            .unwrap());
        assert_eq!(board.columns[0].cards[0].title, "更新したタイトル");
        assert_eq!(board.columns[0].cards[0].description, "更新した説明");
    }

    #[test]
    fn rejects_empty_card_title() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;

        assert_eq!(
            board.update_card(card_id, "  ", "説明"),
            Err(BoardError::EmptyCardTitle)
        );
    }

    #[test]
    fn removes_card_and_reindexes_remaining_cards() {
        let mut board = Board::demo();
        let removed_id = board.columns[0].cards[0].id;

        board.remove_card(removed_id).unwrap();

        assert_eq!(board.columns[0].cards.len(), 1);
        assert_eq!(board.columns[0].cards[0].position, 0);
        assert_eq!(board.columns[0].cards[0].title, "D&D の操作を試す");
    }
}
