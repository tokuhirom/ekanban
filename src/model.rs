use chrono::NaiveDate;
use thiserror::Error;

pub type BoardId = i64;
pub type ColumnId = i64;
pub type CardId = i64;
pub type TagId = i64;
pub type ChecklistItemId = i64;

pub const SOON_THRESHOLD_DAYS: i64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardSummary {
    pub id: BoardId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

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
pub struct ChecklistItem {
    pub id: ChecklistItemId,
    pub card_id: CardId,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistItemDraft {
    pub id: Option<ChecklistItemId>,
    pub text: String,
    pub checked: bool,
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
    pub checklist_items: Vec<ChecklistItem>,
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
pub enum BoardOperation {
    MoveCard {
        card_id: CardId,
        from_column_id: ColumnId,
        from_index: usize,
        to_column_id: ColumnId,
        to_index: usize,
    },
    MoveColumn {
        column_id: ColumnId,
        from_index: usize,
        to_index: usize,
    },
    AddCard {
        card: Card,
    },
    UpdateCard {
        card_id: CardId,
        before_title: String,
        before_description: String,
        after_title: String,
        after_description: String,
    },
    EditCard {
        card_id: CardId,
        before_title: String,
        before_description: String,
        before_due_date: Option<NaiveDate>,
        before_tag_ids: Vec<TagId>,
        after_title: String,
        after_description: String,
        after_due_date: Option<NaiveDate>,
        after_tag_ids: Vec<TagId>,
        before_checklist_items: Vec<ChecklistItem>,
        after_checklist_items: Vec<ChecklistItem>,
    },
    CopyCard {
        card: Card,
        index: usize,
    },
    AddChecklistItem {
        card_id: CardId,
        item: ChecklistItem,
    },
    UpdateChecklistItem {
        card_id: CardId,
        item_id: ChecklistItemId,
        before_text: String,
        after_text: String,
    },
    SetChecklistItemChecked {
        card_id: CardId,
        item_id: ChecklistItemId,
        before: bool,
        after: bool,
    },
    DeleteChecklistItem {
        card_id: CardId,
        item: ChecklistItem,
        index: usize,
    },
    MoveChecklistItem {
        card_id: CardId,
        item_id: ChecklistItemId,
        from_index: usize,
        to_index: usize,
    },
    DeleteCard {
        card: Card,
        index: usize,
    },
    ArchiveCard {
        card: Card,
        archived_card: Card,
        index: usize,
        archived_index: usize,
    },
    ArchiveColumn {
        column_id: ColumnId,
        cards: Vec<ArchivedCardOperation>,
        archived_start: usize,
    },
    RestoreCard {
        archived_card: Card,
        restored_card: Card,
        index: usize,
        archive_index: usize,
    },
    SetDueDate {
        card_id: CardId,
        before: Option<NaiveDate>,
        after: Option<NaiveDate>,
    },
    SortColumnByDueDate {
        column_id: ColumnId,
        before_order: Vec<CardId>,
        after_order: Vec<CardId>,
    },
    SetColumnWipLimit {
        column_id: ColumnId,
        before: Option<i64>,
        after: Option<i64>,
    },
    AddTag {
        tag: Tag,
    },
    RenameTag {
        tag_id: TagId,
        before: String,
        after: String,
    },
    SetTagColor {
        tag_id: TagId,
        before: String,
        after: String,
    },
    RemoveTag {
        tag: Tag,
        index: usize,
        active_card_tags: Vec<(CardId, Vec<TagId>)>,
        archived_card_tags: Vec<(CardId, Vec<TagId>)>,
    },
    SetCardTags {
        card_id: CardId,
        before: Vec<TagId>,
        after: Vec<TagId>,
    },
    AddColumn {
        column: Column,
        index: usize,
    },
    RenameColumn {
        column_id: ColumnId,
        before: String,
        after: String,
    },
    RemoveColumn {
        column: Column,
        index: usize,
        fallback_column_id: ColumnId,
        archived_card_column_ids: Vec<(CardId, ColumnId)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivedCardOperation {
    pub card: Card,
    pub archived_card: Card,
    pub index: usize,
}

#[derive(Debug, Clone)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub next_card_id: CardId,
    pub next_column_id: ColumnId,
    pub next_tag_id: TagId,
    pub next_checklist_item_id: ChecklistItemId,
    pub tags: Vec<Tag>,
    pub archived_cards: Vec<Card>,
    pub columns: Vec<Column>,
    /// Events that are written by the next save and then cleared.
    pub(crate) pending_events: Vec<CardEvent>,
    pub(crate) undo_stack: Vec<BoardOperation>,
    pub(crate) redo_stack: Vec<BoardOperation>,
}

impl PartialEq for Board {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.created_at == other.created_at
            && self.updated_at == other.updated_at
            && self.next_card_id == other.next_card_id
            && self.next_column_id == other.next_column_id
            && self.next_tag_id == other.next_tag_id
            && self.next_checklist_item_id == other.next_checklist_item_id
            && self.tags == other.tags
            && self.archived_cards == other.archived_cards
            && self.columns == other.columns
            && self.pending_events == other.pending_events
    }
}

impl Eq for Board {}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BoardError {
    #[error("a board name cannot be empty")]
    EmptyBoardName,
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
    #[error("a checklist item cannot be empty")]
    EmptyChecklistItemText,
    #[error("checklist item {0} was not found on card {1}")]
    ChecklistItemNotFound(ChecklistItemId, CardId),
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

/// 検索欄に打たれた `#12` を、カード番号として読む。
///
/// 編集パネルはカード番号を出しているのに、その番号から目的のカードへ辿り着く
/// 手段が無かった（#60）。URL スキームは単一インスタンス制御が前提で保留に
/// してあるが、アプリの中で番号から辿るだけならその前提は要らない。
///
/// `#` のうしろが数字だけのときにしか効かない。`#イベント` のような、`#` で
/// 始まるだけの普通の検索語はここでは拾わず、これまでどおりの文字列検索に落ちる。
pub fn parse_card_number_query(query: &str) -> Option<CardId> {
    let query = normalize_search_text(query);
    let digits = query.trim().strip_prefix('#')?;
    if digits.is_empty() || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    digits.parse::<CardId>().ok()
}

pub fn card_matches_search(card: &Card, query: &str) -> bool {
    if let Some(card_id) = parse_card_number_query(query) {
        return card.id == card_id;
    }
    let query = normalize_search_text(query);
    query.is_empty()
        || normalize_search_text(&card.title).contains(&query)
        || normalize_search_text(&card.description).contains(&query)
}

/// 説明の中の `http(s)://` を、出てきた順に取り出す。
///
/// 説明はプレーンテキストのままにする方針（`docs/DESIGN.md` の「やらないこと」に
/// Markdown の描画がある）なので、ここでやるのは URL を見つけることだけ。見出しも
/// 強調も解釈しない。
///
/// 拾うのは `http://` と `https://` だけ。裸の `example.com` や `ftp://` まで
/// 拾うと、URL でない文字列がリンクになる（受け入れ条件「URL 以外の文字列が
/// リンクとして誤検出されない」）。
pub fn find_urls(text: &str) -> Vec<&str> {
    const SCHEMES: [&str; 2] = ["https://", "http://"];

    let mut found: Vec<&str> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let Some((start, scheme)) = SCHEMES
            .iter()
            .filter_map(|scheme| rest.find(scheme).map(|at| (at, *scheme)))
            .min_by_key(|(at, _)| *at)
        else {
            break;
        };

        let candidate = &rest[start..];
        let end = candidate
            .find(char::is_whitespace)
            .unwrap_or(candidate.len());
        let url = trim_url_tail(&candidate[..end]);

        // スキームだけのものは URL として扱わない。
        if url.len() > scheme.len() && !found.contains(&url) {
            found.push(url);
        }
        rest = &candidate[end..];
    }
    found
}

/// URL の末尾に付いてきた句読点や閉じ括弧を落とす。
///
/// 「詳しくは https://example.com/a 。」の `。` は URL ではない。ただし `)` は、
/// 対応する `(` が URL の中にあるなら残す。`https://ja.wikipedia.org/wiki/Rust_(プログラミング言語)`
/// のようなアドレスを壊さないため。
fn trim_url_tail(url: &str) -> &str {
    const TAIL: [char; 20] = [
        '.', ',', ';', ':', '!', '?', '"', '\'', ']', '}', '>', '。', '、', '！', '？', '」', '』',
        '】', '）', ')',
    ];

    let mut url = url;
    while let Some(last) = url.chars().next_back() {
        if !TAIL.contains(&last) {
            break;
        }
        // 対応する開き括弧があるなら、閉じ括弧は URL の一部。
        let opening = match last {
            ')' => Some('('),
            '）' => Some('（'),
            ']' => Some('['),
            '}' => Some('{'),
            _ => None,
        };
        if let Some(opening) = opening {
            let body = &url[..url.len() - last.len_utf8()];
            if body.matches(opening).count() > body.matches(last).count() {
                break;
            }
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    url
}

impl Board {
    pub(crate) fn new_empty(
        id: BoardId,
        name: impl Into<String>,
        next_card_id: CardId,
        first_column_id: ColumnId,
        next_tag_id: TagId,
        next_checklist_item_id: ChecklistItemId,
        now: i64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            created_at: now,
            updated_at: now,
            next_card_id,
            next_column_id: first_column_id + 1,
            next_tag_id,
            next_checklist_item_id,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![Column::new(first_column_id, id, "やること", 0, now)],
            pending_events: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn rename(&mut self, name: impl Into<String>) -> Result<bool, BoardError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(BoardError::EmptyBoardName);
        }
        if self.name == name {
            return Ok(false);
        }
        self.name = name;
        self.updated_at = timestamp();
        Ok(true)
    }

    /// 空のデータベースを開いたときに作る最初のボード。
    ///
    /// カードは入れない。読み終わったら消す前提のものを最初に置くと、消す手間を
    /// 全員に配ることになり、消したあともアーカイブか `card_events` に残る。
    /// カラムだけは置く。0 カラムだと、最初にやることが「カラムを作る」になって
    /// Kanban の形が伝わらない。
    pub fn first_run() -> Self {
        let now = timestamp();
        Self {
            id: 1,
            name: "個人 Kanban".to_string(),
            created_at: now,
            updated_at: now,
            next_card_id: 1,
            next_column_id: 4,
            next_tag_id: 1,
            next_checklist_item_id: 1,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![
                Column::new(1, 1, "やること", 0, now),
                Column::new(2, 1, "進行中", 1, now),
                Column::new(3, 1, "完了", 2, now),
            ],
            pending_events: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// テストの土台。カードが 2 / 1 / 1 枚入った 3 カラムのボード。
    ///
    /// 初回のシード（[`Board::first_run`]）とは別物にしてある。1 つの関数が
    /// 両方を兼ねていたころは、初回の見た目を直すつもりで中身を変えると
    /// テストが壊れた。
    #[cfg(test)]
    pub fn fixture() -> Self {
        let now = timestamp();
        let mut board = Self {
            id: 1,
            name: "個人 Kanban".to_string(),
            created_at: now,
            updated_at: now,
            next_card_id: 1,
            next_column_id: 4,
            next_tag_id: 1,
            next_checklist_item_id: 1,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![
                Column::new(1, 1, "やること", 0, now),
                Column::new(2, 1, "進行中", 1, now),
                Column::new(3, 1, "完了", 2, now),
            ],
            pending_events: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
        board.undo_stack.clear();
        board.redo_stack.clear();
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
        self.push_operation(BoardOperation::MoveCard {
            card_id,
            from_column_id: source_column_id,
            from_index: source_card_index,
            to_column_id: target_column_id,
            to_index: insert_index,
        });
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
        let now = timestamp();
        column.updated_at = now;
        insert_index = insert_index.min(self.columns.len());
        self.columns.insert(insert_index, column);
        self.reindex();
        self.updated_at = now;
        self.push_operation(BoardOperation::MoveColumn {
            column_id,
            from_index: source_index,
            to_index: insert_index,
        });
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
                checklist_items: Vec::new(),
                archived_at: None,
            });
        }
        let card = self
            .columns
            .iter()
            .find(|column| column.id == column_id)
            .and_then(|column| column.cards.last())
            .cloned()
            .expect("new card exists");
        self.next_card_id += 1;
        self.record_event(id, CardEventKind::Created, None, Some(column_id), now);
        self.push_operation(BoardOperation::AddCard { card });
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
        let (before_title, before_description, after_title, after_description) = {
            let card = self
                .columns
                .iter_mut()
                .flat_map(|column| column.cards.iter_mut())
                .find(|card| card.id == card_id)
                .ok_or(BoardError::CardNotFound(card_id))?;

            if card.title == title && card.description == description {
                return Ok(false);
            }

            let before_title = card.title.clone();
            let before_description = card.description.clone();
            let now = timestamp();
            card.title = title;
            card.description = description;
            card.updated_at = now;
            (
                before_title,
                before_description,
                card.title.clone(),
                card.description.clone(),
            )
        };
        self.updated_at = timestamp();
        self.push_operation(BoardOperation::UpdateCard {
            card_id,
            before_title,
            before_description,
            after_title,
            after_description,
        });
        Ok(true)
    }

    pub fn update_card_details(
        &mut self,
        card_id: CardId,
        title: impl Into<String>,
        description: impl Into<String>,
        due_date: Option<NaiveDate>,
        tag_ids: Vec<TagId>,
    ) -> Result<bool, BoardError> {
        let checklist_items = self
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?
            .checklist_items
            .iter()
            .map(|item| ChecklistItemDraft {
                id: Some(item.id),
                text: item.text.clone(),
                checked: item.checked,
            })
            .collect();
        self.update_card_details_with_checklist(
            card_id,
            title,
            description,
            due_date,
            tag_ids,
            checklist_items,
        )
    }

    pub fn update_card_details_with_checklist(
        &mut self,
        card_id: CardId,
        title: impl Into<String>,
        description: impl Into<String>,
        due_date: Option<NaiveDate>,
        tag_ids: Vec<TagId>,
        checklist_drafts: Vec<ChecklistItemDraft>,
    ) -> Result<bool, BoardError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(BoardError::EmptyCardTitle);
        }
        for tag_id in &tag_ids {
            if !self.tags.iter().any(|tag| tag.id == *tag_id) {
                return Err(BoardError::TagNotFound(*tag_id));
            }
        }
        let mut tag_ids = tag_ids;
        tag_ids.sort_unstable();
        tag_ids.dedup();
        let description = description.into();
        let before_card = self
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?
            .clone();
        for draft in &checklist_drafts {
            if draft.text.trim().is_empty() {
                return Err(BoardError::EmptyChecklistItemText);
            }
            if let Some(item_id) = draft.id {
                if !before_card
                    .checklist_items
                    .iter()
                    .any(|item| item.id == item_id)
                {
                    return Err(BoardError::ChecklistItemNotFound(item_id, card_id));
                }
            }
        }

        let now = timestamp();
        let mut checklist_items = Vec::with_capacity(checklist_drafts.len());
        for (position, draft) in checklist_drafts.into_iter().enumerate() {
            let existing = draft.id.and_then(|id| {
                before_card
                    .checklist_items
                    .iter()
                    .find(|item| item.id == id)
            });
            let id = existing
                .map(|item| item.id)
                .unwrap_or(self.next_checklist_item_id);
            if existing.is_none() {
                self.next_checklist_item_id += 1;
            }
            let changed = existing
                .is_none_or(|item| item.text != draft.text || item.checked != draft.checked);
            checklist_items.push(ChecklistItem {
                id,
                card_id,
                text: draft.text,
                checked: draft.checked,
                position: position as i64,
                created_at: existing.map(|item| item.created_at).unwrap_or(now),
                updated_at: if changed {
                    now
                } else {
                    existing.expect("existing item exists").updated_at
                },
            });
        }

        let (
            before_title,
            before_description,
            before_due_date,
            before_tag_ids,
            after_title,
            after_description,
            before_checklist_items,
            after_checklist_items,
        ) = {
            let card = self
                .columns
                .iter_mut()
                .flat_map(|column| column.cards.iter_mut())
                .find(|card| card.id == card_id)
                .ok_or(BoardError::CardNotFound(card_id))?;
            if card.title == title
                && card.description == description
                && card.due_date == due_date
                && card.tag_ids == tag_ids
                && card.checklist_items == checklist_items
            {
                return Ok(false);
            }
            let before_title = card.title.clone();
            let before_description = card.description.clone();
            let before_due_date = card.due_date;
            let before_tag_ids = card.tag_ids.clone();
            let before_checklist_items = card.checklist_items.clone();
            card.title = title.clone();
            card.description = description.clone();
            card.due_date = due_date;
            card.tag_ids = tag_ids.clone();
            card.checklist_items = checklist_items.clone();
            card.updated_at = now;
            (
                before_title,
                before_description,
                before_due_date,
                before_tag_ids,
                card.title.clone(),
                card.description.clone(),
                before_checklist_items,
                card.checklist_items.clone(),
            )
        };
        self.updated_at = now;
        self.push_operation(BoardOperation::EditCard {
            card_id,
            before_title,
            before_description,
            before_due_date,
            before_tag_ids,
            after_title,
            after_description,
            after_due_date: due_date,
            after_tag_ids: tag_ids,
            before_checklist_items,
            after_checklist_items,
        });
        Ok(true)
    }

    pub fn add_checklist_item(
        &mut self,
        card_id: CardId,
        text: impl Into<String>,
    ) -> Result<ChecklistItemId, BoardError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(BoardError::EmptyChecklistItemText);
        }
        let card = self
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        let id = self.next_checklist_item_id;
        let now = timestamp();
        let item = ChecklistItem {
            id,
            card_id,
            text,
            checked: false,
            position: card.checklist_items.len() as i64,
            created_at: now,
            updated_at: now,
        };
        self.next_checklist_item_id += 1;
        self.insert_checklist_item_raw(item.clone())?;
        self.touch_active_card(card_id, now)?;
        self.updated_at = now;
        self.push_operation(BoardOperation::AddChecklistItem { card_id, item });
        Ok(id)
    }

    pub fn update_checklist_item(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
        text: impl Into<String>,
    ) -> Result<bool, BoardError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(BoardError::EmptyChecklistItemText);
        }
        let item = self.find_checklist_item_mut(card_id, item_id)?;
        if item.text == text {
            return Ok(false);
        }
        let before = item.text.clone();
        item.text = text.clone();
        let now = timestamp();
        item.updated_at = now;
        self.touch_active_card(card_id, now)?;
        self.updated_at = now;
        self.push_operation(BoardOperation::UpdateChecklistItem {
            card_id,
            item_id,
            before_text: before,
            after_text: text,
        });
        Ok(true)
    }

    pub fn set_checklist_item_checked(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
        checked: bool,
    ) -> Result<bool, BoardError> {
        let item = self.find_checklist_item_mut(card_id, item_id)?;
        if item.checked == checked {
            return Ok(false);
        }
        let before = item.checked;
        item.checked = checked;
        let now = timestamp();
        item.updated_at = now;
        self.touch_active_card(card_id, now)?;
        self.updated_at = now;
        self.push_operation(BoardOperation::SetChecklistItemChecked {
            card_id,
            item_id,
            before,
            after: checked,
        });
        Ok(true)
    }

    pub fn delete_checklist_item(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
    ) -> Result<(), BoardError> {
        let (index, item) = self.remove_checklist_item_raw(card_id, item_id)?;
        let now = timestamp();
        self.touch_active_card(card_id, now)?;
        self.updated_at = now;
        self.push_operation(BoardOperation::DeleteChecklistItem {
            card_id,
            item,
            index,
        });
        Ok(())
    }

    pub fn move_checklist_item(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
        target_index: usize,
    ) -> Result<bool, BoardError> {
        let items_len = self.find_active_card(card_id)?.checklist_items.len();
        let index = self
            .find_active_card(card_id)?
            .checklist_items
            .iter()
            .position(|item| item.id == item_id)
            .ok_or(BoardError::ChecklistItemNotFound(item_id, card_id))?;
        let mut target_index = target_index.min(items_len);
        if index < target_index {
            target_index -= 1;
        }
        if index == target_index {
            return Ok(false);
        }
        self.move_checklist_item_raw(card_id, index, target_index)?;
        let now = timestamp();
        self.touch_active_card(card_id, now)?;
        self.updated_at = now;
        self.push_operation(BoardOperation::MoveChecklistItem {
            card_id,
            item_id,
            from_index: index,
            to_index: target_index,
        });
        Ok(true)
    }

    pub fn copy_card(&mut self, card_id: CardId) -> Result<CardId, BoardError> {
        let (column_index, card_index) = self
            .active_card_location(card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        let source = self.columns[column_index].cards[card_index].clone();
        let new_card_id = self.next_card_id;
        let now = timestamp();
        let mut checklist_items = Vec::with_capacity(source.checklist_items.len());
        for (position, item) in source.checklist_items.iter().enumerate() {
            let id = self.next_checklist_item_id;
            self.next_checklist_item_id += 1;
            checklist_items.push(ChecklistItem {
                id,
                card_id: new_card_id,
                text: item.text.clone(),
                checked: false,
                position: position as i64,
                created_at: now,
                updated_at: now,
            });
        }
        let card = Card {
            id: new_card_id,
            column_id: source.column_id,
            title: source.title,
            description: source.description,
            position: (card_index + 1) as i64,
            created_at: now,
            updated_at: now,
            due_date: None,
            tag_ids: source.tag_ids,
            checklist_items,
            archived_at: None,
        };
        self.next_card_id += 1;
        self.columns[column_index]
            .cards
            .insert(card_index + 1, card.clone());
        self.reindex();
        self.updated_at = now;
        self.record_event(
            new_card_id,
            CardEventKind::Created,
            None,
            Some(source.column_id),
            now,
        );
        self.push_operation(BoardOperation::CopyCard {
            card,
            index: card_index + 1,
        });
        Ok(new_card_id)
    }

    /// 追加した直後の、まだ一度も保存していないカードを無かったことにする。
    ///
    /// `delete_card` とは別に用意している。あちらは「あったカードを消す」操作で、
    /// `card_events` に `deleted` を残し、Undo にも積む。こちらは「追加そのものを
    /// 取りやめる」ので、`created` の記録ごと取り下げ、Undo にも何も残さない。
    /// 使う人から見ればそのカードは一度も存在していない。
    pub fn discard_added_card(&mut self, card_id: CardId) -> Result<(), BoardError> {
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

        // 追加を積んだ操作を取り下げる。残すと、取りやめたあとの Undo が
        // 「消えているカードをもう一度消す」ことになって失敗する。
        if let Some(position) = self.undo_stack.iter().rposition(
            |operation| matches!(operation, BoardOperation::AddCard { card } if card.id == card_id),
        ) {
            self.undo_stack.remove(position);
        }
        // 保存していないので `created` もまだ書かれていない。残すと、次の保存で
        // 存在しないカードの履歴が 1 件だけ書かれる。
        self.pending_events
            .retain(|event| !(event.card_id == card_id && event.kind == CardEventKind::Created));

        // ID は詰めない。採番は単調増加のままにする。
        self.updated_at = timestamp();
        Ok(())
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
        let card = self.columns[column_index].cards.remove(card_index);
        self.reindex();
        let now = timestamp();
        self.updated_at = now;
        self.record_event(card_id, CardEventKind::Deleted, Some(column_id), None, now);
        self.push_operation(BoardOperation::DeleteCard {
            card,
            index: card_index,
        });
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
        let original_card = card.clone();
        card.archived_at = Some(now);
        card.updated_at = now;
        let archived_card = card.clone();
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
        self.push_operation(BoardOperation::ArchiveCard {
            card: original_card,
            archived_card,
            index: card_index,
            archived_index: self.archived_cards.len() - 1,
        });
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
        let archived_start = self.archived_cards.len();
        let mut cards = std::mem::take(&mut column.cards);
        let count = cards.len();
        let original_cards = cards.clone();
        for card in &mut cards {
            card.archived_at = Some(now);
            card.updated_at = now;
        }
        let archived_cards = cards.clone();
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
        self.push_operation(BoardOperation::ArchiveColumn {
            column_id,
            cards: original_cards
                .into_iter()
                .zip(archived_cards)
                .enumerate()
                .map(|(index, (card, archived_card))| ArchivedCardOperation {
                    card,
                    archived_card,
                    index,
                })
                .collect(),
            archived_start,
        });
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
        let archived_card = self.archived_cards[archive_index].clone();
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
        let restored_card = card.clone();
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
        self.push_operation(BoardOperation::RestoreCard {
            archived_card,
            restored_card,
            index: self
                .columns
                .iter()
                .find(|column| column.id == target_column_id)
                .map(|column| column.cards.len().saturating_sub(1))
                .expect("target column exists"),
            archive_index,
        });
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

        let before = card.due_date;
        let now = timestamp();
        card.due_date = due_date;
        card.updated_at = now;
        self.updated_at = now;
        self.push_operation(BoardOperation::SetDueDate {
            card_id,
            before,
            after: due_date,
        });
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

        if column
            .cards
            .iter()
            .map(|card| card.id)
            .eq(original_order.iter().copied())
        {
            return Ok(false);
        }

        let sorted_order = column.cards.iter().map(|card| card.id).collect();
        self.reindex();
        self.updated_at = timestamp();
        self.push_operation(BoardOperation::SortColumnByDueDate {
            column_id,
            before_order: original_order,
            after_order: sorted_order,
        });
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

        let before = column.wip_limit;
        let now = timestamp();
        column.wip_limit = wip_limit;
        column.updated_at = now;
        self.updated_at = now;
        self.push_operation(BoardOperation::SetColumnWipLimit {
            column_id,
            before,
            after: wip_limit,
        });
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
        let tag = self.tags.last().cloned().expect("new tag exists");
        self.next_tag_id += 1;
        self.updated_at = now;
        self.push_operation(BoardOperation::AddTag { tag });
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
        let before = {
            let tag = self
                .tags
                .iter_mut()
                .find(|tag| tag.id == tag_id)
                .ok_or(BoardError::TagNotFound(tag_id))?;
            if tag.name == name {
                return Ok(false);
            }
            let before = tag.name.clone();
            let now = timestamp();
            tag.name = name.clone();
            tag.updated_at = now;
            before
        };
        self.updated_at = timestamp();
        self.push_operation(BoardOperation::RenameTag {
            tag_id,
            before,
            after: name,
        });
        Ok(true)
    }

    pub fn set_tag_color(
        &mut self,
        tag_id: TagId,
        color: impl Into<String>,
    ) -> Result<bool, BoardError> {
        let color = color.into();
        let before = {
            let tag = self
                .tags
                .iter_mut()
                .find(|tag| tag.id == tag_id)
                .ok_or(BoardError::TagNotFound(tag_id))?;
            if tag.color == color {
                return Ok(false);
            }
            let before = tag.color.clone();
            let now = timestamp();
            tag.color = color.clone();
            tag.updated_at = now;
            before
        };
        self.updated_at = timestamp();
        self.push_operation(BoardOperation::SetTagColor {
            tag_id,
            before,
            after: color,
        });
        Ok(true)
    }

    pub fn remove_tag(&mut self, tag_id: TagId) -> Result<(), BoardError> {
        let index = self
            .tags
            .iter()
            .position(|tag| tag.id == tag_id)
            .ok_or(BoardError::TagNotFound(tag_id))?;
        let active_card_tags = self
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .filter(|card| card.tag_ids.contains(&tag_id))
            .map(|card| (card.id, card.tag_ids.clone()))
            .collect::<Vec<_>>();
        let archived_card_tags = self
            .archived_cards
            .iter()
            .filter(|card| card.tag_ids.contains(&tag_id))
            .map(|card| (card.id, card.tag_ids.clone()))
            .collect::<Vec<_>>();
        let tag = self.tags.remove(index);
        for column in &mut self.columns {
            for card in &mut column.cards {
                card.tag_ids.retain(|id| *id != tag_id);
            }
        }
        for card in &mut self.archived_cards {
            card.tag_ids.retain(|id| *id != tag_id);
        }
        self.updated_at = timestamp();
        self.push_operation(BoardOperation::RemoveTag {
            tag,
            index,
            active_card_tags,
            archived_card_tags,
        });
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
        let before = card.tag_ids.clone();
        let after = tag_ids.clone();
        let now = timestamp();
        card.tag_ids = tag_ids;
        card.updated_at = now;
        self.updated_at = now;
        self.push_operation(BoardOperation::SetCardTags {
            card_id,
            before,
            after,
        });
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
        let column = self.columns.last().cloned().expect("new column exists");
        self.next_column_id += 1;
        self.updated_at = now;
        self.push_operation(BoardOperation::AddColumn {
            column,
            index: self.columns.len() - 1,
        });
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
        let before = {
            let column = self
                .columns
                .iter_mut()
                .find(|column| column.id == column_id)
                .ok_or(BoardError::ColumnNotFound(column_id))?;

            if column.name == name {
                return Ok(false);
            }

            let before = column.name.clone();
            let now = timestamp();
            column.name = name.clone();
            column.updated_at = now;
            before
        };
        self.updated_at = timestamp();
        self.push_operation(BoardOperation::RenameColumn {
            column_id,
            before,
            after: name,
        });
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
        let column = self.columns[index].clone();
        let archived_card_column_ids = self
            .archived_cards
            .iter()
            .filter(|card| card.column_id == column_id)
            .map(|card| (card.id, card.column_id))
            .collect::<Vec<_>>();
        let deleted_card_ids = column.cards.iter().map(|card| card.id).collect::<Vec<_>>();
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
        self.push_operation(BoardOperation::RemoveColumn {
            column,
            index,
            fallback_column_id,
            archived_card_column_ids,
        });
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Result<bool, BoardError> {
        let Some(operation) = self.undo_stack.pop() else {
            return Ok(false);
        };
        if let Err(error) = self.apply_operation(&operation, true) {
            self.undo_stack.push(operation);
            return Err(error);
        }
        self.redo_stack.push(operation);
        Ok(true)
    }

    pub fn redo(&mut self) -> Result<bool, BoardError> {
        let Some(operation) = self.redo_stack.pop() else {
            return Ok(false);
        };
        if let Err(error) = self.apply_operation(&operation, false) {
            self.redo_stack.push(operation);
            return Err(error);
        }
        self.undo_stack.push(operation);
        Ok(true)
    }

    fn push_operation(&mut self, operation: BoardOperation) {
        self.undo_stack.push(operation);
        self.redo_stack.clear();
    }

    fn apply_operation(
        &mut self,
        operation: &BoardOperation,
        undo: bool,
    ) -> Result<(), BoardError> {
        match operation {
            BoardOperation::MoveCard {
                card_id,
                from_column_id,
                from_index,
                to_column_id,
                to_index,
            } => {
                if undo {
                    self.move_card_raw(*card_id, *from_column_id, *from_index)?;
                } else {
                    self.move_card_raw(*card_id, *to_column_id, *to_index)?;
                }
            }
            BoardOperation::MoveColumn {
                column_id,
                from_index,
                to_index,
            } => {
                self.move_column_raw(*column_id, if undo { *from_index } else { *to_index })?;
            }
            BoardOperation::AddCard { card } => {
                if undo {
                    self.remove_active_card(card.id)?;
                } else {
                    self.insert_active_card(card.clone(), card.position as usize)?;
                    self.next_card_id = self.next_card_id.max(card.id + 1);
                }
            }
            BoardOperation::UpdateCard {
                card_id,
                before_title,
                before_description,
                after_title,
                after_description,
            } => {
                self.update_card_raw(
                    *card_id,
                    if undo { before_title } else { after_title },
                    if undo {
                        before_description
                    } else {
                        after_description
                    },
                )?;
            }
            BoardOperation::EditCard {
                card_id,
                before_title,
                before_description,
                before_due_date,
                before_tag_ids,
                after_title,
                after_description,
                after_due_date,
                after_tag_ids,
                before_checklist_items,
                after_checklist_items,
            } => {
                self.update_card_raw(
                    *card_id,
                    if undo { before_title } else { after_title },
                    if undo {
                        before_description
                    } else {
                        after_description
                    },
                )?;
                self.set_due_date_raw(
                    *card_id,
                    if undo {
                        *before_due_date
                    } else {
                        *after_due_date
                    },
                )?;
                self.set_card_tags_raw(
                    *card_id,
                    if undo { before_tag_ids } else { after_tag_ids },
                )?;
                self.set_checklist_items_raw(
                    *card_id,
                    if undo {
                        before_checklist_items
                    } else {
                        after_checklist_items
                    },
                )?;
            }
            BoardOperation::CopyCard { card, index } => {
                if undo {
                    self.remove_active_card(card.id)?;
                } else {
                    self.insert_active_card(card.clone(), *index)?;
                    self.next_card_id = self.next_card_id.max(card.id + 1);
                    if let Some(next_item_id) =
                        card.checklist_items.iter().map(|item| item.id).max()
                    {
                        self.next_checklist_item_id =
                            self.next_checklist_item_id.max(next_item_id + 1);
                    }
                }
            }
            BoardOperation::AddChecklistItem { item, .. } => {
                if undo {
                    self.remove_checklist_item_raw(item.card_id, item.id)?;
                } else {
                    self.insert_checklist_item_raw(item.clone())?;
                    self.next_checklist_item_id = self.next_checklist_item_id.max(item.id + 1);
                }
            }
            BoardOperation::UpdateChecklistItem {
                card_id,
                item_id,
                before_text,
                after_text,
            } => self.update_checklist_item_raw(
                *card_id,
                *item_id,
                if undo { before_text } else { after_text },
            )?,
            BoardOperation::SetChecklistItemChecked {
                card_id,
                item_id,
                before,
                after,
            } => self.set_checklist_item_checked_raw(
                *card_id,
                *item_id,
                if undo { *before } else { *after },
            )?,
            BoardOperation::DeleteChecklistItem {
                card_id,
                item,
                index,
            } => {
                if undo {
                    self.insert_checklist_item_at_raw(*card_id, item.clone(), *index)?;
                } else {
                    self.remove_checklist_item_raw(*card_id, item.id)?;
                }
            }
            BoardOperation::MoveChecklistItem {
                card_id,
                from_index,
                to_index,
                ..
            } => self.move_checklist_item_raw(
                *card_id,
                if undo { *to_index } else { *from_index },
                if undo { *from_index } else { *to_index },
            )?,
            BoardOperation::DeleteCard { card, index } => {
                if undo {
                    self.insert_active_card(card.clone(), *index)?;
                } else {
                    self.remove_active_card(card.id)?;
                }
            }
            BoardOperation::ArchiveCard {
                card,
                archived_card,
                index,
                archived_index,
            } => {
                if undo {
                    self.remove_archived_card(card.id)?;
                    self.insert_active_card(card.clone(), *index)?;
                } else {
                    self.remove_active_card(card.id)?;
                    let index = (*archived_index).min(self.archived_cards.len());
                    self.archived_cards.insert(index, archived_card.clone());
                }
                self.reindex();
            }
            BoardOperation::ArchiveColumn {
                cards,
                archived_start,
                ..
            } => {
                if undo {
                    for operation in cards {
                        self.remove_archived_card(operation.card.id)?;
                        self.insert_active_card(operation.card.clone(), operation.index)?;
                    }
                } else {
                    for operation in cards {
                        self.remove_active_card(operation.card.id)?;
                        let index =
                            (*archived_start + operation.index).min(self.archived_cards.len());
                        self.archived_cards
                            .insert(index, operation.archived_card.clone());
                    }
                }
                self.reindex();
            }
            BoardOperation::RestoreCard {
                archived_card,
                restored_card,
                index,
                archive_index,
            } => {
                if undo {
                    self.remove_active_card(restored_card.id)?;
                    let archive_index = (*archive_index).min(self.archived_cards.len());
                    self.archived_cards
                        .insert(archive_index, archived_card.clone());
                } else {
                    self.remove_archived_card(archived_card.id)?;
                    self.insert_active_card(restored_card.clone(), *index)?;
                }
                self.reindex();
            }
            BoardOperation::SetDueDate {
                card_id,
                before,
                after,
            } => self.set_due_date_raw(*card_id, if undo { *before } else { *after })?,
            BoardOperation::SortColumnByDueDate {
                column_id,
                before_order,
                after_order,
            } => self.sort_column_raw(*column_id, if undo { before_order } else { after_order })?,
            BoardOperation::SetColumnWipLimit {
                column_id,
                before,
                after,
            } => self.set_column_wip_limit_raw(*column_id, if undo { *before } else { *after })?,
            BoardOperation::AddTag { tag } => {
                if undo {
                    self.remove_tag_raw(tag.id)?;
                } else {
                    self.tags.push(tag.clone());
                    self.next_tag_id = self.next_tag_id.max(tag.id + 1);
                }
            }
            BoardOperation::RenameTag {
                tag_id,
                before,
                after,
            } => self.rename_tag_raw(*tag_id, if undo { before } else { after })?,
            BoardOperation::SetTagColor {
                tag_id,
                before,
                after,
            } => self.set_tag_color_raw(*tag_id, if undo { before } else { after })?,
            BoardOperation::RemoveTag {
                tag,
                index,
                active_card_tags,
                archived_card_tags,
            } => {
                if undo {
                    self.tags.insert(*index, tag.clone());
                    self.restore_card_tags(active_card_tags);
                    self.restore_card_tags(archived_card_tags);
                } else {
                    self.remove_tag_raw(tag.id)?;
                }
            }
            BoardOperation::SetCardTags {
                card_id,
                before,
                after,
            } => self.set_card_tags_raw(*card_id, if undo { before } else { after })?,
            BoardOperation::AddColumn { column, index } => {
                if undo {
                    self.remove_column_raw(column.id)?;
                } else {
                    self.columns.insert(*index, column.clone());
                    self.next_column_id = self.next_column_id.max(column.id + 1);
                    self.reindex();
                }
            }
            BoardOperation::RenameColumn {
                column_id,
                before,
                after,
            } => self.rename_column_raw(*column_id, if undo { before } else { after })?,
            BoardOperation::RemoveColumn {
                column,
                index,
                fallback_column_id,
                archived_card_column_ids,
            } => {
                if undo {
                    self.columns.insert(*index, column.clone());
                    for (card_id, column_id) in archived_card_column_ids {
                        if let Some(card) = self
                            .archived_cards
                            .iter_mut()
                            .find(|card| card.id == *card_id)
                        {
                            card.column_id = *column_id;
                        }
                    }
                } else {
                    self.remove_column_raw(column.id)?;
                    for (card_id, _) in archived_card_column_ids {
                        if let Some(card) = self
                            .archived_cards
                            .iter_mut()
                            .find(|card| card.id == *card_id)
                        {
                            card.column_id = *fallback_column_id;
                        }
                    }
                }
                self.reindex();
            }
        }
        self.updated_at = timestamp();
        Ok(())
    }

    fn active_card_location(&self, card_id: CardId) -> Option<(usize, usize)> {
        self.columns
            .iter()
            .enumerate()
            .find_map(|(column_index, column)| {
                column
                    .cards
                    .iter()
                    .position(|card| card.id == card_id)
                    .map(|card_index| (column_index, card_index))
            })
    }

    fn find_active_card(&self, card_id: CardId) -> Result<&Card, BoardError> {
        self.columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))
    }

    fn find_checklist_item_mut(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
    ) -> Result<&mut ChecklistItem, BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        card.checklist_items
            .iter_mut()
            .find(|item| item.id == item_id)
            .ok_or(BoardError::ChecklistItemNotFound(item_id, card_id))
    }

    fn touch_active_card(&mut self, card_id: CardId, now: i64) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        card.updated_at = now;
        Ok(())
    }

    fn remove_active_card(&mut self, card_id: CardId) -> Result<Card, BoardError> {
        let (column_index, card_index) = self
            .active_card_location(card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        let card = self.columns[column_index].cards.remove(card_index);
        self.reindex();
        Ok(card)
    }

    fn insert_active_card(&mut self, mut card: Card, index: usize) -> Result<(), BoardError> {
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == card.column_id)
            .ok_or(BoardError::ColumnNotFound(card.column_id))?;
        card.archived_at = None;
        let index = index.min(column.cards.len());
        column.cards.insert(index, card);
        self.reindex();
        Ok(())
    }

    fn insert_checklist_item_raw(&mut self, item: ChecklistItem) -> Result<(), BoardError> {
        self.insert_checklist_item_at_raw(item.card_id, item, usize::MAX)
    }

    fn insert_checklist_item_at_raw(
        &mut self,
        card_id: CardId,
        mut item: ChecklistItem,
        index: usize,
    ) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        item.card_id = card_id;
        let index = index.min(card.checklist_items.len());
        card.checklist_items.insert(index, item);
        Self::reindex_checklist_items(card);
        Ok(())
    }

    fn remove_checklist_item_raw(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
    ) -> Result<(usize, ChecklistItem), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        let index = card
            .checklist_items
            .iter()
            .position(|item| item.id == item_id)
            .ok_or(BoardError::ChecklistItemNotFound(item_id, card_id))?;
        let item = card.checklist_items.remove(index);
        Self::reindex_checklist_items(card);
        Ok((index, item))
    }

    fn move_checklist_item_raw(
        &mut self,
        card_id: CardId,
        from_index: usize,
        target_index: usize,
    ) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        if from_index >= card.checklist_items.len() {
            return Err(BoardError::ChecklistItemNotFound(
                from_index as ChecklistItemId,
                card_id,
            ));
        }
        let item = card.checklist_items.remove(from_index);
        let target_index = target_index.min(card.checklist_items.len());
        card.checklist_items.insert(target_index, item);
        Self::reindex_checklist_items(card);
        Ok(())
    }

    fn update_checklist_item_raw(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
        text: &str,
    ) -> Result<(), BoardError> {
        let item = self.find_checklist_item_mut(card_id, item_id)?;
        item.text = text.to_string();
        item.updated_at = timestamp();
        Ok(())
    }

    fn set_checklist_item_checked_raw(
        &mut self,
        card_id: CardId,
        item_id: ChecklistItemId,
        checked: bool,
    ) -> Result<(), BoardError> {
        let item = self.find_checklist_item_mut(card_id, item_id)?;
        item.checked = checked;
        item.updated_at = timestamp();
        Ok(())
    }

    fn set_checklist_items_raw(
        &mut self,
        card_id: CardId,
        items: &[ChecklistItem],
    ) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        card.checklist_items = items.to_vec();
        Self::reindex_checklist_items(card);
        Ok(())
    }

    fn reindex_checklist_items(card: &mut Card) {
        for (position, item) in card.checklist_items.iter_mut().enumerate() {
            item.card_id = card.id;
            item.position = position as i64;
        }
    }

    fn remove_archived_card(&mut self, card_id: CardId) -> Result<Card, BoardError> {
        let index = self
            .archived_cards
            .iter()
            .position(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        Ok(self.archived_cards.remove(index))
    }

    fn move_card_raw(
        &mut self,
        card_id: CardId,
        target_column_id: ColumnId,
        target_index: usize,
    ) -> Result<(), BoardError> {
        let card = self.remove_active_card(card_id)?;
        let mut card = card;
        card.column_id = target_column_id;
        self.insert_active_card(card, target_index)
    }

    fn move_column_raw(
        &mut self,
        column_id: ColumnId,
        target_index: usize,
    ) -> Result<(), BoardError> {
        let source_index = self
            .columns
            .iter()
            .position(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        let column = self.columns.remove(source_index);
        let target_index = target_index.min(self.columns.len());
        self.columns.insert(target_index, column);
        self.reindex();
        Ok(())
    }

    fn update_card_raw(
        &mut self,
        card_id: CardId,
        title: &str,
        description: &str,
    ) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        card.title = title.to_string();
        card.description = description.to_string();
        card.updated_at = timestamp();
        Ok(())
    }

    fn set_due_date_raw(
        &mut self,
        card_id: CardId,
        due_date: Option<NaiveDate>,
    ) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        card.due_date = due_date;
        card.updated_at = timestamp();
        Ok(())
    }

    fn sort_column_raw(&mut self, column_id: ColumnId, order: &[CardId]) -> Result<(), BoardError> {
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        let cards = std::mem::take(&mut column.cards);
        let mut reordered = Vec::with_capacity(cards.len());
        let mut remaining = cards;
        for card_id in order {
            let index = remaining
                .iter()
                .position(|card| card.id == *card_id)
                .ok_or(BoardError::CardNotFound(*card_id))?;
            reordered.push(remaining.remove(index));
        }
        if !remaining.is_empty() {
            return Err(BoardError::CardNotFound(remaining[0].id));
        }
        column.cards = reordered;
        self.reindex();
        Ok(())
    }

    fn set_column_wip_limit_raw(
        &mut self,
        column_id: ColumnId,
        wip_limit: Option<i64>,
    ) -> Result<(), BoardError> {
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        column.wip_limit = wip_limit;
        column.updated_at = timestamp();
        Ok(())
    }

    fn remove_tag_raw(&mut self, tag_id: TagId) -> Result<(), BoardError> {
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
        Ok(())
    }

    fn restore_card_tags(&mut self, assignments: &[(CardId, Vec<TagId>)]) {
        for (card_id, tag_ids) in assignments {
            if let Some(card) = self
                .columns
                .iter_mut()
                .flat_map(|column| column.cards.iter_mut())
                .find(|card| card.id == *card_id)
                .or_else(|| {
                    self.archived_cards
                        .iter_mut()
                        .find(|card| card.id == *card_id)
                })
            {
                card.tag_ids = tag_ids.clone();
            }
        }
    }

    fn rename_tag_raw(&mut self, tag_id: TagId, name: &str) -> Result<(), BoardError> {
        let tag = self
            .tags
            .iter_mut()
            .find(|tag| tag.id == tag_id)
            .ok_or(BoardError::TagNotFound(tag_id))?;
        tag.name = name.to_string();
        tag.updated_at = timestamp();
        Ok(())
    }

    fn set_tag_color_raw(&mut self, tag_id: TagId, color: &str) -> Result<(), BoardError> {
        let tag = self
            .tags
            .iter_mut()
            .find(|tag| tag.id == tag_id)
            .ok_or(BoardError::TagNotFound(tag_id))?;
        tag.color = color.to_string();
        tag.updated_at = timestamp();
        Ok(())
    }

    fn set_card_tags_raw(&mut self, card_id: CardId, tag_ids: &[TagId]) -> Result<(), BoardError> {
        let card = self
            .columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .find(|card| card.id == card_id)
            .ok_or(BoardError::CardNotFound(card_id))?;
        card.tag_ids = tag_ids.to_vec();
        card.updated_at = timestamp();
        Ok(())
    }

    fn rename_column_raw(&mut self, column_id: ColumnId, name: &str) -> Result<(), BoardError> {
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        column.name = name.to_string();
        column.updated_at = timestamp();
        Ok(())
    }

    fn remove_column_raw(&mut self, column_id: ColumnId) -> Result<(), BoardError> {
        let index = self
            .columns
            .iter()
            .position(|column| column.id == column_id)
            .ok_or(BoardError::ColumnNotFound(column_id))?;
        if self.columns.len() == 1 {
            return Err(BoardError::LastColumn);
        }
        self.columns.remove(index);
        self.reindex();
        Ok(())
    }

    fn reindex(&mut self) {
        for (column_index, column) in self.columns.iter_mut().enumerate() {
            column.position = column_index as i64;
            for (card_index, card) in column.cards.iter_mut().enumerate() {
                card.position = card_index as i64;
                card.column_id = column.id;
                Self::reindex_checklist_items(card);
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
        card_matches_search, due_status, find_urls, normalize_search_text, parse_card_number_query,
        parse_due_date, Board, BoardError, CardEventKind, ChecklistItemDraft, DueStatus,
    };

    #[test]
    fn moves_card_to_another_column() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        assert!(board.move_card(card_id, 2, 0).unwrap());
        assert_eq!(board.columns[0].cards.len(), 1);
        assert_eq!(board.columns[1].cards[0].id, card_id);
        assert_eq!(board.columns[1].cards[0].column_id, 2);
    }

    #[test]
    fn reorders_card_inside_a_column() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        assert!(board.move_card(card_id, 1, 2).unwrap());
        assert_eq!(board.columns[0].cards[0].title, "D&D の操作を試す");
        assert_eq!(board.columns[0].cards[1].id, card_id);
    }

    #[test]
    fn moving_a_card_to_its_current_position_is_a_noop() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        assert!(!board.move_card(card_id, 1, 0).unwrap());
        assert_eq!(board.columns[0].cards[0].id, card_id);
    }

    #[test]
    fn reorders_columns() {
        let mut board = Board::fixture();

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
        let mut board = Board::fixture();

        assert!(!board.move_column(2, 1).unwrap());
        assert_eq!(board.columns[1].id, 2);
    }

    #[test]
    fn rejects_unknown_card_and_column() {
        let mut board = Board::fixture();
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
    fn discarding_an_added_card_leaves_no_trace() {
        let mut board = Board::fixture();
        board.discard_pending_events();
        let before = board.clone();

        let card_id = board.add_card(1, "", "").unwrap();
        assert_eq!(board.columns[0].cards.len(), 3);

        board.discard_added_card(card_id).unwrap();

        assert_eq!(board.columns[0].cards.len(), 2);
        // 追加も削除も履歴に残らない。使う人から見れば一度も存在していない。
        assert!(board.pending_events.is_empty());
        assert!(!board.can_undo());
        assert_eq!(board.columns, before.columns);
    }

    #[test]
    fn discarding_an_added_card_keeps_the_operations_before_it() {
        let mut board = Board::fixture();
        let moved = board.columns[0].cards[0].id;
        board.move_card(moved, 2, 0).unwrap();

        let card_id = board.add_card(1, "", "").unwrap();
        board.discard_added_card(card_id).unwrap();

        // 取り下げるのは追加した 1 手だけ。その前の移動は Undo で戻せる。
        assert!(board.undo().unwrap());
        assert_eq!(board.columns[0].cards[0].id, moved);
    }

    #[test]
    fn discarding_an_added_card_does_not_reuse_its_id() {
        let mut board = Board::fixture();

        let discarded = board.add_card(1, "", "").unwrap();
        board.discard_added_card(discarded).unwrap();
        let next = board.add_card(1, "次のカード", "").unwrap();

        assert_ne!(next, discarded);
    }

    #[test]
    fn rejects_discarding_a_card_that_is_not_there() {
        let mut board = Board::fixture();
        assert_eq!(
            board.discard_added_card(999),
            Err(BoardError::CardNotFound(999))
        );
    }

    #[test]
    fn updates_card_content() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        assert!(board
            .update_card(card_id, "更新したタイトル", "更新した説明")
            .unwrap());
        assert_eq!(board.columns[0].cards[0].title, "更新したタイトル");
        assert_eq!(board.columns[0].cards[0].description, "更新した説明");
    }

    #[test]
    fn undoes_a_card_editor_save_as_one_operation() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;
        let tag_id = board.add_tag("重要", "#ef4444").unwrap();
        let due_date = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();

        board
            .update_card_details(card_id, "更新", "説明", Some(due_date), vec![tag_id])
            .unwrap();
        assert!(board.undo().unwrap());
        let card = &board.columns[0].cards[0];
        assert_eq!(card.title, "GPUI の画面を作る");
        assert_eq!(card.description, "カラムとカードを表示する");
        assert_eq!(card.due_date, None);
        assert!(card.tag_ids.is_empty());
        assert!(board.redo().unwrap());
        let card = &board.columns[0].cards[0];
        assert_eq!(card.title, "更新");
        assert_eq!(card.due_date, Some(due_date));
        assert_eq!(card.tag_ids, vec![tag_id]);
    }

    #[test]
    fn rejects_empty_card_title() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        assert_eq!(
            board.update_card(card_id, "  ", "説明"),
            Err(BoardError::EmptyCardTitle)
        );
    }

    #[test]
    fn removes_card_and_reindexes_remaining_cards() {
        let mut board = Board::fixture();
        let removed_id = board.columns[0].cards[0].id;

        board.remove_card(removed_id).unwrap();

        assert_eq!(board.columns[0].cards.len(), 1);
        assert_eq!(board.columns[0].cards[0].position, 0);
        assert_eq!(board.columns[0].cards[0].title, "D&D の操作を試す");
    }

    #[test]
    fn does_not_reuse_deleted_card_ids() {
        let mut board = Board::fixture();
        let first = board.add_card(1, "1", "").unwrap();
        let second = board.add_card(1, "2", "").unwrap();
        let third = board.add_card(1, "3", "").unwrap();

        board.remove_card(third).unwrap();

        assert_eq!(board.add_card(1, "4", "").unwrap(), third + 1);
        assert_eq!(second, first + 1);
    }

    #[test]
    fn does_not_reuse_deleted_column_ids() {
        let mut board = Board::fixture();
        let first = board.add_column("追加 1").unwrap();
        let second = board.add_column("追加 2").unwrap();

        board.remove_column(second).unwrap();

        assert_eq!(board.add_column("追加 3").unwrap(), second + 1);
        assert_eq!(first + 1, second);
    }

    #[test]
    fn renames_column_and_skips_unchanged_values() {
        let mut board = Board::fixture();

        assert!(!board.rename_column(1, "やること").unwrap());
        assert!(board.rename_column(1, "近日中").unwrap());
        assert_eq!(board.columns[0].name, "近日中");
    }

    #[test]
    fn rejects_empty_column_names() {
        let mut board = Board::fixture();

        assert_eq!(board.add_column("  "), Err(BoardError::EmptyColumnName));
        assert_eq!(
            board.rename_column(1, "\n"),
            Err(BoardError::EmptyColumnName)
        );
    }

    #[test]
    fn renames_a_board_and_rejects_empty_names() {
        let mut board = Board::fixture();

        assert!(!board.rename("個人 Kanban").unwrap());
        assert!(board.rename("仕事").unwrap());
        assert_eq!(board.name, "仕事");
        assert_eq!(board.rename("  "), Err(BoardError::EmptyBoardName));
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
        let mut board = Board::fixture();
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
        let mut board = Board::fixture();
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
        let mut board = Board::fixture();
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
    fn finds_a_card_by_its_number() {
        let board = Board::fixture();
        let first = &board.columns[0].cards[0];
        let second = &board.columns[0].cards[1];
        assert_ne!(first.id, second.id, "the fixture has two distinct cards");

        let query = format!("#{}", first.id);
        assert!(card_matches_search(first, &query));
        assert!(!card_matches_search(second, &query));

        // 全角で打っても同じ。検索欄の正規化を通してから番号として読む。
        assert_eq!(parse_card_number_query("＃１"), Some(1));
        assert_eq!(parse_card_number_query("  #12  "), Some(12));
    }

    #[test]
    fn keeps_searching_for_text_that_merely_starts_with_a_hash() {
        let mut board = Board::fixture();
        board.update_card(1, "#イベント の準備", "").unwrap();
        let card = &board.columns[0].cards[0];

        // 番号でない `#` 付きの語は、これまでどおりの文字列検索に落ちる。
        assert_eq!(parse_card_number_query("#イベント"), None);
        assert_eq!(parse_card_number_query("#"), None);
        assert_eq!(parse_card_number_query("#12a"), None);
        assert_eq!(parse_card_number_query("イベント"), None);
        assert!(card_matches_search(card, "#イベント"));
    }

    #[test]
    fn sets_wip_limit_and_rejects_non_positive_values() {
        let mut board = Board::fixture();

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
        let mut board = Board::fixture();
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
        let mut board = Board::fixture();
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
        let mut board = Board::fixture();
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
        let mut board = Board::fixture();
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
        let mut board = Board::fixture();
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
    fn undoes_and_redoes_card_operations_without_reusing_snapshots() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;
        let original_title = board.columns[0].cards[0].title.clone();

        assert!(board.move_card(card_id, 2, 0).unwrap());
        assert!(board.can_undo());
        assert!(!board.can_redo());
        assert_eq!(board.columns[1].cards[0].id, card_id);

        assert!(board.undo().unwrap());
        assert_eq!(board.columns[0].cards[0].id, card_id);
        assert!(!board.can_undo());
        assert!(board.can_redo());

        assert!(board.redo().unwrap());
        assert_eq!(board.columns[1].cards[0].id, card_id);

        assert!(board.update_card(card_id, "更新", "説明").unwrap());
        assert!(board.undo().unwrap());
        assert_eq!(board.columns[1].cards[0].title, original_title);
        assert!(board.redo().unwrap());
        assert_eq!(board.columns[1].cards[0].title, "更新");
    }

    #[test]
    fn a_new_operation_clears_redo_history() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        board
            .set_card_due_date(card_id, Some(NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()))
            .unwrap();
        board.undo().unwrap();
        assert!(board.can_redo());

        board
            .set_card_due_date(card_id, Some(NaiveDate::from_ymd_opt(2026, 10, 1).unwrap()))
            .unwrap();
        assert!(!board.can_redo());
    }

    #[test]
    fn archives_a_column_and_keeps_archived_cards_when_column_is_deleted() {
        let mut board = Board::fixture();
        let archived_id = board.columns[0].cards[0].id;
        assert_eq!(board.archive_column(1).unwrap(), 2);
        assert_eq!(board.archived_cards[0].id, archived_id);

        board.remove_column(1).unwrap();

        assert_eq!(board.archived_cards[0].column_id, board.columns[0].id);
    }

    #[test]
    fn rejects_empty_and_duplicate_tag_names() {
        let mut board = Board::fixture();

        assert_eq!(board.add_tag(" ", "#000000"), Err(BoardError::EmptyTagName));
        board.add_tag("仕事", "#000000").unwrap();
        assert_eq!(
            board.add_tag("仕事", "#ffffff"),
            Err(BoardError::DuplicateTagName("仕事".to_string()))
        );
    }

    #[test]
    fn manages_checklist_items_and_reindexes_them() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;
        let first_id = board.add_checklist_item(card_id, "テストを書く").unwrap();
        let second_id = board.add_checklist_item(card_id, "fmt を通す").unwrap();

        assert_eq!(board.columns[0].cards[0].checklist_items.len(), 2);
        assert!(board
            .set_checklist_item_checked(card_id, second_id, true)
            .unwrap());
        assert!(board.move_checklist_item(card_id, second_id, 0).unwrap());
        assert_eq!(
            board.columns[0].cards[0]
                .checklist_items
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![second_id, first_id]
        );
        assert_eq!(board.columns[0].cards[0].checklist_items[0].position, 0);

        board
            .update_checklist_item(card_id, first_id, "テストを書く（完了）")
            .unwrap();
        board.delete_checklist_item(card_id, second_id).unwrap();
        assert_eq!(board.columns[0].cards[0].checklist_items.len(), 1);
        assert_eq!(
            board.columns[0].cards[0].checklist_items[0].text,
            "テストを書く（完了）"
        );
    }

    #[test]
    fn edits_checklist_items_with_card_details_as_one_undo_operation() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;

        board
            .update_card_details_with_checklist(
                card_id,
                "PR を出す",
                "手順を確認する",
                None,
                Vec::new(),
                vec![
                    ChecklistItemDraft {
                        id: None,
                        text: "テストを書く".to_string(),
                        checked: false,
                    },
                    ChecklistItemDraft {
                        id: None,
                        text: "fmt を通す".to_string(),
                        checked: true,
                    },
                ],
            )
            .unwrap();
        assert_eq!(board.columns[0].cards[0].checklist_items.len(), 2);
        assert!(board.columns[0].cards[0].checklist_items[1].checked);

        board.undo().unwrap();
        assert!(board.columns[0].cards[0].checklist_items.is_empty());
        assert_eq!(board.columns[0].cards[0].title, "GPUI の画面を作る");
        board.redo().unwrap();
        assert_eq!(board.columns[0].cards[0].checklist_items.len(), 2);
    }

    #[test]
    fn copies_card_content_and_resets_due_date_and_checks() {
        let mut board = Board::fixture();
        let source_id = board.columns[0].cards[0].id;
        let tag_id = board.add_tag("手順", "#60a5fa").unwrap();
        board
            .update_card_details_with_checklist(
                source_id,
                "元カード",
                "説明",
                Some(NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()),
                vec![tag_id],
                vec![ChecklistItemDraft {
                    id: None,
                    text: "確認する".to_string(),
                    checked: true,
                }],
            )
            .unwrap();
        board.pending_events.clear();

        let copied_id = board.copy_card(source_id).unwrap();
        let copied = board.columns[0]
            .cards
            .iter()
            .find(|card| card.id == copied_id)
            .unwrap();
        let source = board.columns[0]
            .cards
            .iter()
            .find(|card| card.id == source_id)
            .unwrap();
        assert_eq!(copied.title, source.title);
        assert_eq!(copied.description, source.description);
        assert_eq!(copied.tag_ids, source.tag_ids);
        assert_eq!(copied.due_date, None);
        assert_eq!(copied.archived_at, None);
        assert_eq!(copied.checklist_items[0].text, "確認する");
        assert!(!copied.checklist_items[0].checked);
        assert_ne!(copied.checklist_items[0].id, source.checklist_items[0].id);
        assert_eq!(board.pending_events.len(), 1);
        assert_eq!(board.pending_events[0].kind, CardEventKind::Created);

        board.undo().unwrap();
        assert!(board.columns[0]
            .cards
            .iter()
            .all(|card| card.id != copied_id));
        board.redo().unwrap();
        assert!(board.columns[0]
            .cards
            .iter()
            .any(|card| card.id == copied_id));
    }

    #[test]
    fn rejects_empty_checklist_items() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;
        assert_eq!(
            board.add_checklist_item(card_id, "  "),
            Err(BoardError::EmptyChecklistItemText)
        );
        assert_eq!(
            board.update_card_details_with_checklist(
                card_id,
                "タイトル",
                "",
                None,
                Vec::new(),
                vec![ChecklistItemDraft {
                    id: None,
                    text: "".to_string(),
                    checked: false,
                }],
            ),
            Err(BoardError::EmptyChecklistItemText)
        );
    }

    #[test]
    fn finds_the_urls_in_a_description() {
        assert_eq!(
            find_urls("設計は https://example.com/design 、PR は https://example.com/pull/1 です"),
            ["https://example.com/design", "https://example.com/pull/1"],
            "the links come back in the order they were written"
        );
        assert_eq!(
            find_urls("改行のあと\nhttp://example.com/plain"),
            ["http://example.com/plain"]
        );
    }

    #[test]
    fn leaves_plain_text_alone() {
        for description in [
            "",
            "URL のない説明",
            "example.com は裸のホスト名なので拾わない",
            "ftp://example.com/file は http でも https でもない",
            "https:// だけではアドレスにならない",
        ] {
            assert!(
                find_urls(description).is_empty(),
                "nothing in {description:?} is a link"
            );
        }
    }

    #[test]
    fn drops_trailing_punctuation_but_keeps_a_balanced_bracket() {
        assert_eq!(
            find_urls("詳しくは https://example.com/a 。"),
            ["https://example.com/a"],
            "the full stop after the address is not part of it"
        );
        assert_eq!(
            find_urls("(https://example.com/b) を見る"),
            ["https://example.com/b"],
            "and neither is a bracket that only wraps it"
        );
        assert_eq!(
            find_urls("https://ja.wikipedia.org/wiki/Rust_(プログラミング言語)"),
            ["https://ja.wikipedia.org/wiki/Rust_(プログラミング言語)"],
            "but a bracket the address opened itself stays"
        );
    }

    #[test]
    fn lists_a_repeated_url_once() {
        assert_eq!(
            find_urls("https://example.com/a と https://example.com/a は同じ"),
            ["https://example.com/a"]
        );
    }
}
