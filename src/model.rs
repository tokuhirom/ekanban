use chrono::NaiveDate;
use thiserror::Error;

pub type BoardId = i64;
pub type ColumnId = i64;
pub type CardId = i64;
pub type TagId = i64;

pub const SOON_THRESHOLD_DAYS: i64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardEventKind {
    Created,
    Moved,
    Archived,
    Restored,
    Deleted,
}

impl CardEventKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Moved => "moved",
            Self::Archived => "archived",
            Self::Restored => "restored",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEvent {
    pub card_id: CardId,
    pub kind: CardEventKind,
    pub from_column_id: Option<ColumnId>,
    pub to_column_id: Option<ColumnId>,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: CardId,
    pub column_id: ColumnId,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub due_date: Option<NaiveDate>,
    pub tag_ids: Vec<TagId>,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub id: TagId,
    pub board_id: BoardId,
    pub name: String,
    pub color: String,
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
    pub wip_limit: Option<i64>,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_card_id: CardId,
    pub next_column_id: ColumnId,
    pub next_tag_id: TagId,
    pub tags: Vec<Tag>,
    pub archived_cards: Vec<Card>,
    pub columns: Vec<Column>,
    /// Events that are written by the next save and then cleared.
    pub(crate) pending_events: Vec<CardEvent>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BoardError {
    #[error("column {0} was not found")]
    ColumnNotFound(ColumnId),
    #[error("card {0} was not found")]
    CardNotFound(CardId),
    #[error("a card title cannot be empty")]
    EmptyCardTitle,
    #[error("a column name cannot be empty")]
    EmptyColumnName,
    #[error("invalid due date: {0}")]
    InvalidDueDate(String),
    #[error("invalid WIP limit: {0}")]
    InvalidWipLimit(String),
    #[error("a tag name cannot be empty")]
    EmptyTagName,
    #[error("tag {0} was not found")]
    TagNotFound(TagId),
    #[error("a tag named {0} already exists")]
    DuplicateTagName(String),
    #[error("a board must have at least one column")]
    LastColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DueStatus {
    Overdue(i64),
    Today,
    Soon(i64),
    Upcoming(i64),
    None,
}

pub fn due_status(due_date: Option<NaiveDate>, today: NaiveDate) -> DueStatus {
    let Some(due_date) = due_date else {
        return DueStatus::None;
    };
    let days = due_date.signed_duration_since(today).num_days();
    match days {
        ..=-1 => DueStatus::Overdue(-days),
        0 => DueStatus::Today,
        1..=SOON_THRESHOLD_DAYS => DueStatus::Soon(days),
        _ => DueStatus::Upcoming(days),
    }
}

pub fn parse_due_date(value: &str) -> Result<Option<NaiveDate>, BoardError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| BoardError::InvalidDueDate(value.to_string()))
}

pub fn parse_wip_limit(value: &str) -> Result<Option<i64>, BoardError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let limit = value
        .parse::<i64>()
        .map_err(|_| BoardError::InvalidWipLimit(value.to_string()))?;
    if limit <= 0 {
        return Err(BoardError::InvalidWipLimit(value.to_string()));
    }
    Ok(Some(limit))
}

pub fn normalize_search_text(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\u{3000}' => ' ',
            '\u{ff01}'..='\u{ff5e}' => {
                char::from_u32(character as u32 - 0xfee0).unwrap_or(character)
            }
            character => character,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn card_matches_search(card: &Card, query: &str) -> bool {
    let query = normalize_search_text(query);
    query.is_empty()
        || normalize_search_text(&card.title).contains(&query)
        || normalize_search_text(&card.description).contains(&query)
}

impl Board {
    pub fn demo() -> Self {
        let now = timestamp();
        let mut board = Self {
            id: 1,
            name: "個人 Kanban".to_string(),
            created_at: now,
            updated_at: now,
            next_card_id: 1,
            next_column_id: 4,
            next_tag_id: 1,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![
                Column::new(1, 1, "やること", 0, now),
                Column::new(2, 1, "進行中", 1, now),
                Column::new(3, 1, "完了", 2, now),
            ],
            pending_events: Vec::new(),
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
        let source_column_id = self.columns[source_column_index].id;
        let mut insert_index = target_index.min(self.columns[target_column_index].cards.len());
        if same_column && source_card_index < insert_index {
            insert_index -= 1;
        }

        if same_column && source_card_index == insert_index {
            return Ok(false);
        }

        let now = timestamp();
        let card = self.columns[source_column_index]
            .cards
            .remove(source_card_index);
        let card = Card {
            column_id: target_column_id,
            updated_at: now,
            ..card
        };

        insert_index = insert_index.min(self.columns[target_column_index].cards.len());
        self.columns[target_column_index]
            .cards
            .insert(insert_index, card);
        self.reindex();
        self.updated_at = now;
        if !same_column {
            self.record_event(
                card_id,
                CardEventKind::Moved,
                Some(source_column_id),
                Some(target_column_id),
                now,
            );
        }
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
        let id = self.next_card_id;
        let title = title.into();
        let description = description.into();
        let now = timestamp();
        {
            let column = self
                .columns
                .iter_mut()
                .find(|column| column.id == column_id)
                .ok_or(BoardError::ColumnNotFound(column_id))?;
            column.cards.push(Card {
                id,
                column_id,
                title,
                description,
                position: column.cards.len() as i64,
                created_at: now,
                updated_at: now,
                due_date: None,
                tag_ids: Vec::new(),
                archived_at: None,
            });
        }
        self.next_card_id += 1;
        self.record_event(id, CardEventKind::Created, None, Some(column_id), now);
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
        self.delete_card(card_id)
    }

    pub fn delete_card(&mut self, card_id: CardId) -> Result<(), BoardError> {
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

        let column_id = self.columns[column_index].id;
        self.columns[column_index].cards.remove(card_index);
        self.reindex();
        let now = timestamp();
        self.updated_at = now;
        self.record_event(card_id, CardEventKind::Deleted, Some(column_id), None, now);
        Ok(())
    }

    pub fn archive_card(&mut self, card_id: CardId) -> Result<bool, BoardError> {
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
        let source_column_id = self.columns[column_index].id;
        let now = timestamp();
        let mut card = self.columns[column_index].cards.remove(card_index);
        card.archived_at = Some(now);
        card.updated_at = now;
        self.archived_cards.push(card);
        self.reindex();
        self.updated_at = now;
        self.record_event(
            card_id,
            CardEventKind::Archived,
            Some(source_column_id),
            None,
            now,
        );
        Ok(true)
    }

    pub fn archive_column(&mut self, column_id: ColumnId) -> Result<usize, BoardError> {
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        if column.cards.is_empty() {
            return Ok(0);
        }

        let now = timestamp();
        let mut cards = std::mem::take(&mut column.cards);
        let count = cards.len();
        for card in &mut cards {
            card.archived_at = Some(now);
            card.updated_at = now;
        }
        self.pending_events
            .extend(cards.iter().map(|card| CardEvent {
                card_id: card.id,
                kind: CardEventKind::Archived,
                from_column_id: Some(column_id),
                to_column_id: None,
                at: now,
            }));
        self.archived_cards.append(&mut cards);
        self.reindex();
        self.updated_at = now;
        Ok(count)
    }

    pub fn restore_card(&mut self, card_id: CardId) -> Result<bool, BoardError> {
        let archive_index = self
            .archived_cards
            .iter()
            .position(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        let target_column_id = self
            .columns
            .iter()
            .find(|column| column.id == self.archived_cards[archive_index].column_id)
            .map(|column| column.id)
            .or_else(|| self.columns.first().map(|column| column.id))
            .ok_or(BoardError::LastColumn)?;
        let now = timestamp();
        let mut card = self.archived_cards.remove(archive_index);
        card.column_id = target_column_id;
        card.position = self
            .columns
            .iter()
            .find(|column| column.id == target_column_id)
            .map(|column| column.cards.len() as i64)
            .expect("target column exists");
        card.archived_at = None;
        card.updated_at = now;
        self.columns
            .iter_mut()
            .find(|column| column.id == target_column_id)
            .expect("target column exists")
            .cards
            .push(card);
        self.reindex();
        self.updated_at = now;
        self.record_event(
            card_id,
            CardEventKind::Restored,
            None,
            Some(target_column_id),
            now,
        );
        Ok(true)
    }

    pub fn set_card_due_date(
        &mut self,
        card_id: CardId,
        due_date: Option<NaiveDate>,
    ) -> Result<bool, BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;

        if card.due_date == due_date {
            return Ok(false);
        }

        let now = timestamp();
        card.due_date = due_date;
        card.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn sort_column_by_due_date(&mut self, column_id: ColumnId) -> Result<bool, BoardError> {
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        let original_order = column.cards.iter().map(|card| card.id).collect::<Vec<_>>();
        column.cards.sort_by(|left, right| {
            match (left.due_date, right.due_date) {
                (Some(left), Some(right)) => left.cmp(&right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| left.position.cmp(&right.position))
        });

        if column.cards.iter().map(|card| card.id).eq(original_order) {
            return Ok(false);
        }

        self.reindex();
        self.updated_at = timestamp();
        Ok(true)
    }

    pub fn set_column_wip_limit(
        &mut self,
        column_id: ColumnId,
        wip_limit: Option<i64>,
    ) -> Result<bool, BoardError> {
        if wip_limit.is_some_and(|limit| limit <= 0) {
            return Err(BoardError::InvalidWipLimit(
                "上限は 1 以上で入力してください".to_string(),
            ));
        }
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        if column.wip_limit == wip_limit {
            return Ok(false);
        }

        let now = timestamp();
        column.wip_limit = wip_limit;
        column.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn add_tag(
        &mut self,
        name: impl Into<String>,
        color: impl Into<String>,
    ) -> Result<TagId, BoardError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(BoardError::EmptyTagName);
        }
        if self.tags.iter().any(|tag| tag.name == name) {
            return Err(BoardError::DuplicateTagName(name));
        }
        let id = self.next_tag_id;
        let now = timestamp();
        self.tags.push(Tag {
            id,
            board_id: self.id,
            name,
            color: color.into(),
            created_at: now,
            updated_at: now,
        });
        self.next_tag_id += 1;
        self.updated_at = now;
        Ok(id)
    }

    pub fn rename_tag(
        &mut self,
        tag_id: TagId,
        name: impl Into<String>,
    ) -> Result<bool, BoardError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(BoardError::EmptyTagName);
        }
        if self
            .tags
            .iter()
            .any(|tag| tag.id != tag_id && tag.name == name)
        {
            return Err(BoardError::DuplicateTagName(name));
        }
        let tag = self
            .tags
            .iter_mut()
            .find(|tag| tag.id == tag_id)
            .ok_or(BoardError::TagNotFound(tag_id))?;
        if tag.name == name {
            return Ok(false);
        }
        let now = timestamp();
        tag.name = name;
        tag.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn set_tag_color(
        &mut self,
        tag_id: TagId,
        color: impl Into<String>,
    ) -> Result<bool, BoardError> {
        let color = color.into();
        let tag = self
            .tags
            .iter_mut()
            .find(|tag| tag.id == tag_id)
            .ok_or(BoardError::TagNotFound(tag_id))?;
        if tag.color == color {
            return Ok(false);
        }
        let now = timestamp();
        tag.color = color;
        tag.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn remove_tag(&mut self, tag_id: TagId) -> Result<(), BoardError> {
        let index = self
            .tags
            .iter()
            .position(|tag| tag.id == tag_id)
            .ok_or(BoardError::TagNotFound(tag_id))?;
        self.tags.remove(index);
        for column in &mut self.columns {
            for card in &mut column.cards {
                card.tag_ids.retain(|id| *id != tag_id);
            }
        }
        for card in &mut self.archived_cards {
            card.tag_ids.retain(|id| *id != tag_id);
        }
        self.updated_at = timestamp();
        Ok(())
    }

    pub fn set_card_tags(
        &mut self,
        card_id: CardId,
        tag_ids: Vec<TagId>,
    ) -> Result<bool, BoardError> {
        for tag_id in &tag_ids {
            if !self.tags.iter().any(|tag| tag.id == *tag_id) {
                return Err(BoardError::TagNotFound(*tag_id));
            }
        }
        let mut tag_ids = tag_ids;
        tag_ids.sort_unstable();
        tag_ids.dedup();
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        if card.tag_ids == tag_ids {
            return Ok(false);
        }
        let now = timestamp();
        card.tag_ids = tag_ids;
        card.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn add_column(&mut self, name: impl Into<String>) -> Result<ColumnId, BoardError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(BoardError::EmptyColumnName);
        }
        let id = self.next_column_id;
        let now = timestamp();
        self.columns.push(Column::new(
            id,
            self.id,
            name,
            self.columns.len() as i64,
            now,
        ));
        self.next_column_id += 1;
        self.updated_at = now;
        Ok(id)
    }

    pub fn rename_column(
        &mut self,
        column_id: ColumnId,
        name: impl Into<String>,
    ) -> Result<bool, BoardError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(BoardError::EmptyColumnName);
        }
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;

        if column.name == name {
            return Ok(false);
        }

        let now = timestamp();
        column.name = name;
        column.updated_at = now;
        self.updated_at = now;
        Ok(true)
    }

    pub fn remove_column(&mut self, column_id: ColumnId) -> Result<(), BoardError> {
        let index = self
            .columns
            .iter()
            .position(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        if self.columns.len() == 1 {
            return Err(BoardError::LastColumn);
        }
        let fallback_column_id = self
            .columns
            .iter()
            .find(|column| column.id != column_id)
            .expect("there is another column")
            .id;
        let deleted_card_ids = self.columns[index]
            .cards
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        self.columns.remove(index);
        let now = timestamp();
        for card in &mut self.archived_cards {
            if card.column_id == column_id {
                card.column_id = fallback_column_id;
                card.updated_at = now;
            }
        }
        self.reindex();
        self.updated_at = now;
        self.pending_events
            .extend(deleted_card_ids.into_iter().map(|card_id| CardEvent {
                card_id,
                kind: CardEventKind::Deleted,
                from_column_id: Some(column_id),
                to_column_id: None,
                at: now,
            }));
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

    fn record_event(
        &mut self,
        card_id: CardId,
        kind: CardEventKind,
        from_column_id: Option<ColumnId>,
        to_column_id: Option<ColumnId>,
        at: i64,
    ) {
        self.pending_events.push(CardEvent {
            card_id,
            kind,
            from_column_id,
            to_column_id,
            at,
        });
    }

    pub(crate) fn discard_pending_events(&mut self) {
        self.pending_events.clear();
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
            wip_limit: None,
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
    use chrono::NaiveDate;

    use super::{
        card_matches_search, due_status, normalize_search_text, parse_due_date, Board, BoardError,
        CardEventKind, DueStatus,
    };

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

    #[test]
    fn does_not_reuse_deleted_card_ids() {
        let mut board = Board::demo();
        let first = board.add_card(1, "1", "").unwrap();
        let second = board.add_card(1, "2", "").unwrap();
        let third = board.add_card(1, "3", "").unwrap();

        board.remove_card(third).unwrap();

        assert_eq!(board.add_card(1, "4", "").unwrap(), third + 1);
        assert_eq!(second, first + 1);
    }

    #[test]
    fn does_not_reuse_deleted_column_ids() {
        let mut board = Board::demo();
        let first = board.add_column("追加 1").unwrap();
        let second = board.add_column("追加 2").unwrap();

        board.remove_column(second).unwrap();

        assert_eq!(board.add_column("追加 3").unwrap(), second + 1);
        assert_eq!(first + 1, second);
    }

    #[test]
    fn renames_column_and_skips_unchanged_values() {
        let mut board = Board::demo();

        assert!(!board.rename_column(1, "やること").unwrap());
        assert!(board.rename_column(1, "近日中").unwrap());
        assert_eq!(board.columns[0].name, "近日中");
    }

    #[test]
    fn rejects_empty_column_names() {
        let mut board = Board::demo();

        assert_eq!(board.add_column("  "), Err(BoardError::EmptyColumnName));
        assert_eq!(
            board.rename_column(1, "\n"),
            Err(BoardError::EmptyColumnName)
        );
    }

    #[test]
    fn classifies_due_date_boundaries() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 4).unwrap();

        assert_eq!(due_status(None, today), DueStatus::None);
        assert_eq!(
            due_status(Some(today.pred_opt().unwrap()), today),
            DueStatus::Overdue(1)
        );
        assert_eq!(due_status(Some(today), today), DueStatus::Today);
        assert_eq!(
            due_status(Some(today.succ_opt().unwrap()), today),
            DueStatus::Soon(1)
        );
        assert_eq!(
            due_status(
                Some(today.checked_add_days(chrono::Days::new(3)).unwrap()),
                today
            ),
            DueStatus::Soon(3)
        );
        assert_eq!(
            due_status(
                Some(today.checked_add_days(chrono::Days::new(4)).unwrap()),
                today
            ),
            DueStatus::Upcoming(4)
        );
    }

    #[test]
    fn handles_leap_day_and_year_boundaries() {
        let new_years_eve = NaiveDate::from_ymd_opt(2026, 12, 31).unwrap();
        let new_years_day = NaiveDate::from_ymd_opt(2027, 1, 1).unwrap();
        assert_eq!(
            due_status(Some(new_years_day), new_years_eve),
            DueStatus::Soon(1)
        );

        let leap_day = NaiveDate::from_ymd_opt(2028, 2, 29).unwrap();
        let march_first = NaiveDate::from_ymd_opt(2028, 3, 1).unwrap();
        assert_eq!(
            due_status(Some(leap_day), march_first),
            DueStatus::Overdue(1)
        );
    }

    #[test]
    fn sets_due_date_and_skips_unchanged_values() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;
        let due_date = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();

        assert!(board.set_card_due_date(card_id, Some(due_date)).unwrap());
        assert!(!board.set_card_due_date(card_id, Some(due_date)).unwrap());
        assert_eq!(board.columns[0].cards[0].due_date, Some(due_date));
        assert!(board.set_card_due_date(card_id, None).unwrap());
    }

    #[test]
    fn parses_and_rejects_due_date_strings() {
        assert_eq!(
            parse_due_date("2028-02-29").unwrap(),
            Some(NaiveDate::from_ymd_opt(2028, 2, 29).unwrap())
        );
        assert_eq!(parse_due_date(" ").unwrap(), None);
        assert_eq!(
            parse_due_date("2028-02-30"),
            Err(BoardError::InvalidDueDate("2028-02-30".to_string()))
        );
    }

    #[test]
    fn sorts_cards_by_due_date_with_empty_dates_last() {
        let mut board = Board::demo();
        let first = board.columns[0].cards[0].id;
        let second = board.columns[0].cards[1].id;
        let first_due = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        let second_due = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        board.set_card_due_date(first, Some(first_due)).unwrap();
        board.set_card_due_date(second, Some(second_due)).unwrap();

        assert!(board.sort_column_by_due_date(1).unwrap());
        assert_eq!(board.columns[0].cards[0].id, second);
        assert_eq!(board.columns[0].cards[1].id, first);
        assert!(!board.sort_column_by_due_date(1).unwrap());
        assert_eq!(board.columns[0].cards[0].position, 0);
    }

    #[test]
    fn searches_case_insensitively_and_normalizes_full_width_ascii() {
        let mut board = Board::demo();
        board
            .update_card(1, "Rust Ｋａｎｂａｎ", "ローカル DB")
            .unwrap();
        let card = &board.columns[0].cards[0];

        assert_eq!(normalize_search_text(" ＫＡＮＢＡＮ　"), " kanban ");
        assert!(card_matches_search(card, "kanban"));
        assert!(card_matches_search(card, "ローカル"));
        assert!(!card_matches_search(card, "存在しない"));
    }

    #[test]
    fn sets_wip_limit_and_rejects_non_positive_values() {
        let mut board = Board::demo();

        assert!(board.set_column_wip_limit(1, Some(3)).unwrap());
        assert!(!board.set_column_wip_limit(1, Some(3)).unwrap());
        assert_eq!(board.columns[0].wip_limit, Some(3));
        assert_eq!(
            board.set_column_wip_limit(1, Some(0)),
            Err(BoardError::InvalidWipLimit(
                "上限は 1 以上で入力してください".to_string()
            ))
        );
        assert!(board.set_column_wip_limit(1, None).unwrap());
    }

    #[test]
    fn manages_tags_and_card_assignments() {
        let mut board = Board::demo();
        let tag_id = board.add_tag("重要", "#ef4444").unwrap();
        let other_tag_id = board.add_tag("個人", "#60a5fa").unwrap();
        let card_id = board.columns[0].cards[0].id;

        assert!(board
            .set_card_tags(card_id, vec![other_tag_id, tag_id, tag_id])
            .unwrap());
        assert_eq!(
            board.columns[0].cards[0].tag_ids,
            vec![tag_id, other_tag_id]
        );
        assert!(!board
            .set_card_tags(card_id, vec![tag_id, other_tag_id])
            .unwrap());
        assert!(board.rename_tag(tag_id, "最重要").unwrap());
        board.remove_tag(other_tag_id).unwrap();
        assert_eq!(board.columns[0].cards[0].tag_ids, vec![tag_id]);
    }

    #[test]
    fn archives_and_restores_cards_without_reusing_ids() {
        let mut board = Board::demo();
        let card_id = board.columns[0].cards[0].id;

        assert!(board.archive_card(card_id).unwrap());
        assert!(board.columns[0].cards.iter().all(|card| card.id != card_id));
        assert!(board.archived_cards[0].archived_at.is_some());

        assert!(board.restore_card(card_id).unwrap());
        assert!(board.columns[0].cards.iter().any(|card| card.id == card_id));
        assert!(board.archived_cards.is_empty());
        assert_eq!(
            board.add_card(1, "新規", "").unwrap(),
            board.next_card_id - 1
        );
    }

    #[test]
    fn records_card_lifecycle_events_but_not_intra_column_reorders() {
        let mut board = Board::demo();
        board.pending_events.clear();
        let card_id = board.columns[0].cards[0].id;

        assert!(board.move_card(card_id, 2, 1).unwrap());
        assert!(board.move_card(card_id, 2, 0).unwrap());
        assert!(board.archive_card(card_id).unwrap());

        assert_eq!(
            board
                .pending_events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            [CardEventKind::Moved, CardEventKind::Archived]
        );
        assert_eq!(board.pending_events[0].from_column_id, Some(1));
        assert_eq!(board.pending_events[0].to_column_id, Some(2));
        assert_eq!(board.pending_events[1].from_column_id, Some(2));
        assert_eq!(board.pending_events[1].to_column_id, None);
    }

    #[test]
    fn records_one_archived_event_for_each_card_in_a_column() {
        let mut board = Board::demo();
        board.pending_events.clear();

        assert_eq!(board.archive_column(1).unwrap(), 2);
        assert_eq!(board.pending_events.len(), 2);
        assert!(board
            .pending_events
            .iter()
            .all(|event| event.kind == CardEventKind::Archived));
    }

    #[test]
    fn records_deleted_events_for_cards_removed_directly_or_with_a_column() {
        let mut board = Board::demo();
        board.pending_events.clear();
        let card_id = board.columns[0].cards[0].id;

        board.delete_card(card_id).unwrap();
        assert_eq!(board.pending_events[0].kind, CardEventKind::Deleted);
        assert_eq!(board.pending_events[0].from_column_id, Some(1));

        board.pending_events.clear();
        let remaining_card_id = board.columns[0].cards[0].id;
        board.remove_column(1).unwrap();
        assert_eq!(board.pending_events.len(), 1);
        assert_eq!(board.pending_events[0].card_id, remaining_card_id);
        assert_eq!(board.pending_events[0].kind, CardEventKind::Deleted);
    }

    #[test]
    fn archives_a_column_and_keeps_archived_cards_when_column_is_deleted() {
        let mut board = Board::demo();
        let archived_id = board.columns[0].cards[0].id;
        assert_eq!(board.archive_column(1).unwrap(), 2);
        assert_eq!(board.archived_cards[0].id, archived_id);

        board.remove_column(1).unwrap();

        assert_eq!(board.archived_cards[0].column_id, board.columns[0].id);
    }

    #[test]
    fn rejects_empty_and_duplicate_tag_names() {
        let mut board = Board::demo();

        assert_eq!(board.add_tag(" ", "#000000"), Err(BoardError::EmptyTagName));
        board.add_tag("仕事", "#000000").unwrap();
        assert_eq!(
            board.add_tag("仕事", "#ffffff"),
            Err(BoardError::DuplicateTagName("仕事".to_string()))
        );
    }
}
