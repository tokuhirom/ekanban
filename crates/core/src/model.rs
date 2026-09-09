//! 盤面の形（[ADR 0039]、[ADR 0040]）。
//!
//! **振る舞いはここにありません。** カードを足す・動かす・戻すは webview の
//! `web/src/model/board.ts` にあり、こちらに残っているのは
//!
//! - 境界を越える形そのもの（`ts-rs` がここから TypeScript の型を書き出す）
//! - 受け取った盤面が**行として成り立っているか**の検め（[`Board::validate`]）
//! - 置いてある文字列を日付として読む [`parse_stored_due_date`]
//!
//! の 3 つです。盤面の判断をここでやり直さないこと——やり直した時点で真実が
//! 2 つに戻ります。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
//! [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub type BoardId = i64;
pub type ColumnId = i64;
pub type CardId = i64;
pub type TagId = i64;
pub type ChecklistItemId = i64;
pub type RecurrenceId = i64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BoardSummary {
    pub id: BoardId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum CardEventKind {
    Created,
    Moved,
    Archived,
    Restored,
    Deleted,
}

impl CardEventKind {
    // 置き場所が SQL に書くときの綴り。`--no-default-features` では誰も呼ばない。
    #[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
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

/// 積まれたカードの履歴 1 件。**次の保存で書き、書けたら捨てます。**
///
/// 盤面を持つのは webview なので、履歴を積むのもそちら側です（[ADR 0039]）。
/// 置き場所は受け取ったものを追記するだけで、何が起きたかを見直しません。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardEvent {
    pub card_id: CardId,
    pub kind: CardEventKind,
    pub from_column_id: Option<ColumnId>,
    pub to_column_id: Option<ColumnId>,
    pub at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChecklistItem {
    pub id: ChecklistItemId,
    pub card_id: CardId,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChecklistItemDraft {
    pub id: Option<ChecklistItemId>,
    pub text: String,
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
    /// このカードを出した繰り返しの定義（#198）。
    ///
    /// **参照であって、生成の真実ではありません。** 次を出すかどうかを決める
    /// のは定義側の `last_generated_on` です（[ADR 0049]）。定義を消しても
    /// この参照は残り、盤面のカードもそのまま残ります——指す先が消えたことは、
    /// 画面が 🔁 を出さないことで表れます。
    ///
    /// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md
    pub recurrence_id: Option<RecurrenceId>,
    /// このカードが受け持っている発生日（`"YYYY-MM-DD"`）。
    ///
    /// 片付ける相手を選ぶのに要ります——「次の発生日が来たら片付ける」の
    /// 「次」は、この日から数えます（[ADR 0049]）。
    ///
    /// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md
    pub occurrence_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tag {
    pub id: TagId,
    pub board_id: BoardId,
    pub name: String,
    pub color: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Column {
    pub id: ColumnId,
    pub board_id: BoardId,
    pub name: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
    /// 終わったものの置き場か（[ADR 0038]）。
    ///
    /// **何本でも立てられます。** 「完了」と「キャンセル済み」は別の終わり方ですが、
    /// もう手を動かさない点では同じなので、区別せず同じ扱いにします。ここに入って
    /// いるカードは期限の件数に数えず、画面ではトーンダウンして描きます。
    ///
    /// [ADR 0038]: ../../../docs/adr/0038-a-column-that-means-done.md
    pub done: bool,
    pub cards: Vec<Card>,
}

/// 繰り返しの周期（#198）。
///
/// **`weekly{月〜金}` と `weekday` は分けてあります。** 見た目の並びは同じに
/// なりますが、祝日の扱いを持つのは後者だけです（いまはまだ曜日の意味しか
/// 持ちません）。片方に寄せると、祝日を足す日に受け皿が無くなります。
///
/// 曜日は**月曜を 0** とした 0〜6 です。`web/src/model/dates.ts` の
/// `weekdayFromMonday` と同じ数え方で、`Date` の日曜 0 とは 1 つずれます。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Schedule {
    /// 毎日。
    Daily,
    /// 平日（月〜金）。祝日の扱いはここに足す。
    Weekday,
    /// 決まった曜日。
    Weekly { days: Vec<u8> },
    /// 毎月の決まった日。**その日が無い月は月末に丸めます**（判断は webview）。
    Monthly { day: u8 },
    /// 毎月末。
    MonthlyLast,
}

impl Schedule {
    /// 置き場所が書くときの綴り。`--no-default-features` では誰も呼ばない。
    #[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
    pub(crate) fn kind_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekday => "weekday",
            Self::Weekly { .. } => "weekly",
            Self::Monthly { .. } => "monthly",
            Self::MonthlyLast => "monthlyLast",
        }
    }
}

/// 前回のカードの片付け方（#198）。
///
/// **未完了でも進行中のカラムにあっても同じように片付けます。** 選んだのは
/// この旗を立てた人で、置き場所がそこを読み替えません。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum PreviousPolicy {
    /// 盤面に残す。
    Keep,
    /// アーカイブへ移す。
    Archive,
    /// 消す。
    Delete,
}

impl PreviousPolicy {
    /// 置き場所が書くときの綴り。`--no-default-features` では誰も呼ばない。
    #[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Archive => "archive",
            Self::Delete => "delete",
        }
    }

    /// 置いてある綴りから読む。知らない綴りは `Keep`——**片付けないほうへ倒す**。
    #[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "archive" => Self::Archive,
            "delete" => Self::Delete,
            _ => Self::Keep,
        }
    }
}

/// 繰り返しの定義 1 つ（#198、[ADR 0049]）。
///
/// **カードとは別のものです。** カード側が持つ `recurrence_id` は参照で、
/// 次を出すかどうかを決める真実はこちらの `last_generated_on` にあります。
/// カードから逆引きすると、手で消したカードが消した次の瞬間に生えてきます。
///
/// 前半はテンプレート（出すカードの中身）、後半は周期と片付け方です。
///
/// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Recurrence {
    pub id: RecurrenceId,
    pub board_id: BoardId,
    pub title: String,
    pub description: String,
    /// 入れ先のカラム。**消えていることがあります**——そのときは一番左に入ります。
    pub column_id: ColumnId,
    pub tag_ids: Vec<TagId>,
    /// チェックリストのひな型。**項目に ID を持たせません**——テンプレートの
    /// 項目は同一性を持たない文字列の並びで、ID を持つのは出来たカードのほうです。
    pub checklist: Vec<String>,
    pub schedule: Schedule,
    /// 発生日の何日前から出すか。`daily` / `weekday` は 0 に固定です
    /// （毎日 `⚠` を 1 件増やさないため、期限も先読みも持ちません）。
    pub lead_days: i64,
    pub previous: PreviousPolicy,
    pub enabled: bool,
    /// **生成の真実**。最後に出したカードの発生日（`"YYYY-MM-DD"`）。
    pub last_generated_on: Option<NaiveDate>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip)]
    #[ts(skip)]
    pub next_card_id: CardId,
    #[serde(skip)]
    #[ts(skip)]
    pub next_column_id: ColumnId,
    #[serde(skip)]
    #[ts(skip)]
    pub next_tag_id: TagId,
    #[serde(skip)]
    #[ts(skip)]
    pub next_checklist_item_id: ChecklistItemId,
    #[serde(skip)]
    #[ts(skip)]
    pub next_recurrence_id: RecurrenceId,
    pub tags: Vec<Tag>,
    pub archived_cards: Vec<Card>,
    pub columns: Vec<Column>,
    /// 繰り返しの定義（#198）。**カードとは別に持ちます**（[ADR 0049]）。
    ///
    /// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md
    pub recurrences: Vec<Recurrence>,
    /// Events that are written by the next save and then cleared.
    #[serde(skip)]
    #[ts(skip)]
    pub(crate) pending_events: Vec<CardEvent>,
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
            && self.next_recurrence_id == other.next_recurrence_id
            && self.tags == other.tags
            && self.recurrences == other.recurrences
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
    #[error("a recurrence title cannot be empty")]
    EmptyRecurrenceTitle,
    #[error("recurrence {0} was not found")]
    RecurrenceNotFound(RecurrenceId),
    /// 保存を頼まれた盤面が、行として成り立っていない（[ADR 0040]）。
    ///
    /// 使う人の入力の間違いではなく、**画面の側の食い違い**です。入力欄の脇に
    /// 返すものが無いので、ダイアログに出します。
    ///
    /// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
    #[error("the board does not hold together: {0}")]
    Inconsistent(String),
}

/// 保存された形の期限を読む（`"YYYY-MM-DD"` か空文字）。
///
/// **打った文字を読むのは webview です**（`web/src/model/due.ts`、[ADR 0031]、
/// [ADR 0039]）。`9/12` や `明日` をここで受けません。ここが見るのは、渡された
/// ものが日付として成り立っているかだけです——置き場所は受け取ったものを
/// 信じない、という線の内側にあります（[ADR 0040]）。
///
/// [ADR 0031]: ../../../docs/adr/0031-typing-a-due-date.md
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
/// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
pub fn parse_stored_due_date(value: &str) -> Result<Option<NaiveDate>, BoardError> {
    let raw = value.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| BoardError::InvalidDueDate(raw.to_string()))
}

/// 新しいボードが使いはじめる採番の続き。
///
/// **ボードごとに区画を切ります**（`store::board_scoped_id`）。ID は
/// `(board_id, id)` の組ではなく主キー 1 本なので、別々のボードが手元で採番
/// しても衝突しないように、先頭を離してあります。
///
/// 5 つを 1 つにまとめてあるのは、渡す先（[`Board::new_empty`]）が引数の並び
/// だけで意味を伝えられなくなったためです。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextIds {
    pub card: CardId,
    pub column: ColumnId,
    pub tag: TagId,
    pub checklist_item: ChecklistItemId,
    pub recurrence: RecurrenceId,
}

impl Board {
    // 新しいボードを作るのは置き場所。`--no-default-features` では誰も呼ばない。
    #[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
    pub(crate) fn new_empty(id: BoardId, name: impl Into<String>, next: NextIds, now: i64) -> Self {
        let first_column_id = next.column;
        Self {
            id,
            name: name.into(),
            created_at: now,
            updated_at: now,
            next_card_id: next.card,
            next_column_id: first_column_id + 2,
            next_tag_id: next.tag,
            next_checklist_item_id: next.checklist_item,
            next_recurrence_id: next.recurrence,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![
                Column::new(first_column_id, id, "やること", 0, now),
                Column::new_done(first_column_id + 1, id, "完了", 1, now),
            ],
            recurrences: Vec::new(),
            pending_events: Vec::new(),
        }
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
            next_recurrence_id: 1,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![
                Column::new(1, 1, "やること", 0, now),
                Column::new(2, 1, "進行中", 1, now),
                Column::new_done(3, 1, "完了", 2, now),
            ],
            recurrences: Vec::new(),
            pending_events: Vec::new(),
        }
    }

    /// テストの土台。カードが 2 / 1 / 1 枚入った 3 カラムのボード。
    ///
    /// 初回のシード（[`Board::first_run`]）とは別物にしてある。1 つの関数が
    /// 両方を兼ねていたころは、初回の見た目を直すつもりで中身を変えると
    /// テストが壊れた。
    ///
    /// **カードは組み立てて置く。** 盤面を変える操作は webview のモデルにあり
    /// （[ADR 0039]）、こちらに残っているのは形だけである。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    // ほかのクレートのテストも使うので、feature で出す。実行ファイルには
    // 入れない（`cargo build --release` では立たない）。
    #[cfg(any(test, feature = "test-fixtures"))]
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
            next_recurrence_id: 1,
            tags: Vec::new(),
            archived_cards: Vec::new(),
            columns: vec![
                Column::new(1, 1, "やること", 0, now),
                Column::new(2, 1, "進行中", 1, now),
                Column::new(3, 1, "完了", 2, now),
            ],
            recurrences: Vec::new(),
            pending_events: Vec::new(),
        };

        for (column_id, title, description) in [
            (1, "画面の描画をひととおり通す", "カラムとカードを表示する"),
            (1, "D&D の操作を試す", "カードを掴んで移動する"),
            (2, "SQLite の設計", "マイグレーションを用意する"),
            (3, "README を書く", "プロジェクトの方針をまとめる"),
        ] {
            board.push_card(column_id, title, description);
        }
        board
    }

    /// カラムの末尾にカードを 1 枚積む。**土台を組み立てるためだけのもの。**
    ///
    /// 採番も `position` もここで合わせる——`validate` が見るのはその 2 つで、
    /// ずれた土台はテストではなく置き場所のほうを落とす。
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn push_card(
        &mut self,
        column_id: ColumnId,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> CardId {
        let now = timestamp();
        let id = self.next_card_id;
        self.next_card_id += 1;
        let column = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .expect("the column is there");
        let position = i64::try_from(column.cards.len()).expect("a card index fits");
        column.cards.push(Card {
            id,
            column_id,
            title: title.into(),
            description: description.into(),
            position,
            created_at: now,
            updated_at: now,
            due_date: None,
            tag_ids: Vec::new(),
            checklist_items: Vec::new(),
            archived_at: None,
            recurrence_id: None,
            occurrence_date: None,
        });
        id
    }

    /// タグを 1 つ足す。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn push_tag(&mut self, name: impl Into<String>, color: impl Into<String>) -> TagId {
        let now = timestamp();
        let id = self.next_tag_id;
        self.next_tag_id += 1;
        self.tags.push(Tag {
            id,
            board_id: self.id,
            name: name.into(),
            color: color.into(),
            created_at: now,
            updated_at: now,
        });
        id
    }

    /// 繰り返しの定義を 1 つ足す。**土台を組み立てるためだけのもの。**
    ///
    /// 中身は返ってきた ID から引いて直に書き換えられます（`recurrence_mut`）。
    /// 盤面の判断は入りません——それは webview のモデルの仕事です（[ADR 0039]）。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn push_recurrence(
        &mut self,
        title: impl Into<String>,
        schedule: Schedule,
    ) -> RecurrenceId {
        let now = timestamp();
        let id = self.next_recurrence_id;
        self.next_recurrence_id += 1;
        let column_id = self.columns.first().map_or(0, |column| column.id);
        self.recurrences.push(Recurrence {
            id,
            board_id: self.id,
            title: title.into(),
            description: String::new(),
            column_id,
            tag_ids: Vec::new(),
            checklist: Vec::new(),
            schedule,
            lead_days: 0,
            previous: PreviousPolicy::Archive,
            enabled: true,
            last_generated_on: None,
            created_at: now,
            updated_at: now,
        });
        id
    }

    /// 繰り返しの定義を 1 つ引き当てる。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn recurrence_mut(&mut self, recurrence_id: RecurrenceId) -> &mut Recurrence {
        self.recurrences
            .iter_mut()
            .find(|recurrence| recurrence.id == recurrence_id)
            .expect("the recurrence is there")
    }

    /// カラムを 1 本足す。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn push_column(&mut self, name: impl Into<String>) -> ColumnId {
        let now = timestamp();
        let id = self.next_column_id;
        self.next_column_id += 1;
        let position = i64::try_from(self.columns.len()).expect("a column index fits");
        self.columns
            .push(Column::new(id, self.id, name, position, now));
        id
    }

    /// カードを 1 枚引き当てる。**土台を組み立てるためだけのもの。**
    ///
    /// 返すのは中身そのものなので、期限もタイトルもタグも直に書き換えられます。
    /// 盤面の判断は入りません——それは webview のモデルの仕事です（[ADR 0039]）。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn card_mut(&mut self, card_id: CardId) -> &mut Card {
        self.columns
            .iter_mut()
            .flat_map(|column| column.cards.iter_mut())
            .chain(self.archived_cards.iter_mut())
            .find(|card| card.id == card_id)
            .expect("the card is there")
    }

    /// カラムを 1 本引き当てる。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn column_mut(&mut self, column_id: ColumnId) -> &mut Column {
        self.columns
            .iter_mut()
            .find(|column| column.id == column_id)
            .expect("the column is there")
    }

    /// 盤面からカードを抜く。**土台を組み立てるためだけのもの。**
    ///
    /// 抜いたあとのカラムは `position` を詰め直します——`validate` が見るのが
    /// そこなので、詰め忘れた土台はテストではなく置き場所のほうを落とします。
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn take_card(&mut self, card_id: CardId) -> Card {
        for column in &mut self.columns {
            if let Some(at) = column.cards.iter().position(|card| card.id == card_id) {
                let card = column.cards.remove(at);
                reposition(&mut column.cards);
                return card;
            }
        }
        panic!("the card is there");
    }

    /// カラムの指定の位置にカードを差す。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn place_card(&mut self, mut card: Card, column_id: ColumnId, index: usize) {
        card.column_id = column_id;
        card.archived_at = None;
        let column = self.column_mut(column_id);
        let at = index.min(column.cards.len());
        column.cards.insert(at, card);
        reposition(&mut column.cards);
    }

    /// カードをしまう。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn stash_card(&mut self, card_id: CardId, at: i64) {
        let mut card = self.take_card(card_id);
        card.archived_at = Some(at);
        self.archived_cards.push(card);
    }

    /// しまったカードを、そのカラムの末尾に戻す。**土台を組み立てるためだけのもの。**
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn unstash_card(&mut self, card_id: CardId) {
        let at = self
            .archived_cards
            .iter()
            .position(|card| card.id == card_id)
            .expect("the card is in the archive");
        let card = self.archived_cards.remove(at);
        let column_id = card.column_id;
        let index = self.column_mut(column_id).cards.len();
        self.place_card(card, column_id, index);
    }

    /// 積んである `card_events` を捨てる。
    ///
    /// 保存し終えたぶんと、巻き戻して無かったことにするぶんの両方が通る。
    /// 受け取った履歴を、次の保存で書くものとして抱える。
    ///
    /// **盤面を持つのは webview** なので、履歴を積むのもそちら側です
    /// （[ADR 0039]）。置き場所は追記するだけで、何が起きたかを見直しません。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    pub fn adopt_pending_events(&mut self, events: Vec<CardEvent>) {
        self.pending_events = events;
    }

    /// 保存を頼まれた盤面が、行として成り立っているかを見る（[ADR 0040]）。
    ///
    /// **盤面の判断をやり直しません。** カードをどのカラムの何枚目に置くかは
    /// webview が決めたことで、ここが見るのは「そのまま書けるか」だけです。
    /// スキーマ（`NOT NULL`・外部キー・`UNIQUE`）が見ているものは重ねて見ません。
    ///
    /// 見るのは 4 つ。
    ///
    /// - 空のタイトル・カラム名・タグ名・繰り返しの題（画面が断っているはずのもの）
    /// - 知らないタグを指すカードと、知らないタグを指す繰り返しの定義
    /// - 並びと `position` の食い違い（見た目と保存が別のことを言う）
    /// - 採番の続きが、使っている ID を追い越していない状態（次に採ると衝突する）
    ///
    /// **カードの `recurrence_id` が実在するかは見ません**（#198）。定義を
    /// 消しても盤面のカードは残るので、そこを見ると消した瞬間から保存できなく
    /// なります。同じ理由で `Recurrence::column_id` の行き先も見ません——
    /// 消えていたら一番左に入れる、と webview が決めています。
    ///
    /// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
    pub fn validate(&self) -> Result<(), BoardError> {
        if self.name.trim().is_empty() {
            return Err(BoardError::EmptyBoardName);
        }
        let tag_ids: Vec<TagId> = self.tags.iter().map(|tag| tag.id).collect();
        for tag in &self.tags {
            if tag.name.trim().is_empty() {
                return Err(BoardError::EmptyTagName);
            }
            if tag.id >= self.next_tag_id {
                return Err(BoardError::Inconsistent(format!(
                    "tag {} is at or past the next tag id {}",
                    tag.id, self.next_tag_id
                )));
            }
        }
        for (index, column) in self.columns.iter().enumerate() {
            if column.name.trim().is_empty() {
                return Err(BoardError::EmptyColumnName);
            }
            if column.position != index as i64 {
                return Err(BoardError::Inconsistent(format!(
                    "column {} sits at {index} but says position {}",
                    column.id, column.position
                )));
            }
            if column.id >= self.next_column_id {
                return Err(BoardError::Inconsistent(format!(
                    "column {} is at or past the next column id {}",
                    column.id, self.next_column_id
                )));
            }
            for (card_index, card) in column.cards.iter().enumerate() {
                self.validate_card(card, &tag_ids)?;
                if card.position != card_index as i64 {
                    return Err(BoardError::Inconsistent(format!(
                        "card {} sits at {card_index} but says position {}",
                        card.id, card.position
                    )));
                }
                if card.column_id != column.id {
                    return Err(BoardError::Inconsistent(format!(
                        "card {} sits in column {} but says column {}",
                        card.id, column.id, card.column_id
                    )));
                }
                if card.archived_at.is_some() {
                    return Err(BoardError::Inconsistent(format!(
                        "card {} is on the board but says it is archived",
                        card.id
                    )));
                }
            }
        }
        for card in &self.archived_cards {
            self.validate_card(card, &tag_ids)?;
            if card.archived_at.is_none() {
                return Err(BoardError::Inconsistent(format!(
                    "card {} is in the archive but says it is not archived",
                    card.id
                )));
            }
        }
        for recurrence in &self.recurrences {
            self.validate_recurrence(recurrence, &tag_ids)?;
        }
        Ok(())
    }

    /// 繰り返しの定義 1 つが、行として成り立っているか（#198）。
    fn validate_recurrence(
        &self,
        recurrence: &Recurrence,
        tag_ids: &[TagId],
    ) -> Result<(), BoardError> {
        if recurrence.title.trim().is_empty() {
            return Err(BoardError::EmptyRecurrenceTitle);
        }
        if recurrence.id >= self.next_recurrence_id {
            return Err(BoardError::Inconsistent(format!(
                "recurrence {} is at or past the next recurrence id {}",
                recurrence.id, self.next_recurrence_id
            )));
        }
        for tag_id in &recurrence.tag_ids {
            if !tag_ids.contains(tag_id) {
                return Err(BoardError::TagNotFound(*tag_id));
            }
        }
        if recurrence.lead_days < 0 {
            return Err(BoardError::Inconsistent(format!(
                "recurrence {} looks ahead {} days",
                recurrence.id, recurrence.lead_days
            )));
        }
        match &recurrence.schedule {
            Schedule::Weekly { days } => {
                if days.is_empty() || days.iter().any(|day| *day > 6) {
                    return Err(BoardError::Inconsistent(format!(
                        "recurrence {} names no usable weekday",
                        recurrence.id
                    )));
                }
            }
            Schedule::Monthly { day } => {
                if *day < 1 || *day > 31 {
                    return Err(BoardError::Inconsistent(format!(
                        "recurrence {} names day {} of the month",
                        recurrence.id, day
                    )));
                }
            }
            Schedule::Daily | Schedule::Weekday | Schedule::MonthlyLast => {}
        }
        Ok(())
    }

    fn validate_card(&self, card: &Card, tag_ids: &[TagId]) -> Result<(), BoardError> {
        if card.title.trim().is_empty() {
            return Err(BoardError::EmptyCardTitle);
        }
        if card.id >= self.next_card_id {
            return Err(BoardError::Inconsistent(format!(
                "card {} is at or past the next card id {}",
                card.id, self.next_card_id
            )));
        }
        for tag_id in &card.tag_ids {
            if !tag_ids.contains(tag_id) {
                return Err(BoardError::TagNotFound(*tag_id));
            }
        }
        for (index, item) in card.checklist_items.iter().enumerate() {
            if item.text.trim().is_empty() {
                return Err(BoardError::EmptyChecklistItemText);
            }
            if item.position != index as i64 || item.card_id != card.id {
                return Err(BoardError::Inconsistent(format!(
                    "checklist item {} on card {} sits at {index} but says position {} on card {}",
                    item.id, card.id, item.position, item.card_id
                )));
            }
            if item.id >= self.next_checklist_item_id {
                return Err(BoardError::Inconsistent(format!(
                    "checklist item {} is at or past the next item id {}",
                    item.id, self.next_checklist_item_id
                )));
            }
        }
        Ok(())
    }

    pub fn discard_pending_events(&mut self) {
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
            done: false,
            cards: Vec::new(),
        }
    }

    /// 終わったものの置き場として作る（[ADR 0038]）。
    ///
    /// [ADR 0038]: ../../../docs/adr/0038-a-column-that-means-done.md
    fn new_done(
        id: ColumnId,
        board_id: BoardId,
        name: impl Into<String>,
        position: i64,
        now: i64,
    ) -> Self {
        Self {
            done: true,
            ..Self::new(id, board_id, name, position, now)
        }
    }
}

/// カードの `position` を、並んでいる順に振り直す。
#[cfg(any(test, feature = "test-fixtures"))]
fn reposition(cards: &mut [Card]) {
    for (at, card) in cards.iter_mut().enumerate() {
        card.position = i64::try_from(at).expect("a card index fits");
    }
}

/// いまの時刻をミリ秒で。
fn timestamp() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
