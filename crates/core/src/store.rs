//! 盤面の置き場所（[ADR 0036]）。
//!
//! 2 つあります。**配るアプリは SQLite**（[`crate::db::Database`]）、**ブラウザ版は
//! JSON**（[`JsonStore`]）。呼ぶ側（`crates/app` の `commands`）は [`Store`] だけを
//! 見るので、どちらが下にいるかを知りません。
//!
//! **分かれているのは「どう置くか」だけです。** 採番も並べ替えも Undo も
//! アーカイブも `model.rs` の 1 つのままで、ここには **`Board` をどう書き出し、
//! どう読み戻すか**しかありません。モデルを 2 つ持たないことが、ADR 0021 の
//! 「偽物のバックエンドを書かない」がブラウザでも成り立つ条件です。
//!
//! ブラウザに SQLite を積まないのは大きさのためです。`wasm32-unknown-unknown`
//! に組んだ SQLite だけで 2.1 MB あり、こちらのコード全部より 5 倍大きい
//! （[ADR 0036]）。
//!
//! [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

use std::collections::BTreeMap;

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::export;
use crate::model::{
    due_status, Board, BoardId, BoardSummary, Card, CardEvent, Column, ColumnId, DueCounts,
    DueStatus, Tag,
};

/// 置き場所が返す失敗。
///
/// SQLite そのものの失敗は、SQLite を積んでいるときだけあります。**理由を
/// 潰しません**——`crates/app` の `error.rs` がエラーコードごとに「次に何を
/// すればよいか」を出し分けており（ディスクが一杯、読み取り専用、壊れている）、
/// 1 つの文字列に丸めるとそれが全部消えます。
#[derive(Debug, Error)]
pub enum StoreError {
    #[cfg(feature = "sqlite")]
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("no board exists in the database")]
    NoBoard,
    #[error("cannot delete the last board")]
    LastBoard,
    #[error("a board name cannot be empty")]
    EmptyBoardName,
    #[error("invalid saved application state")]
    InvalidAppState,
    /// 置き場所を、開いたままもう 1 つ開いた。
    ///
    /// **待たずに落とします。** 置き場所がページに 1 つしかないところ
    /// （ブラウザ版）では、待っても相手は自分なので永遠に空きません。
    /// SQLite は接続を 2 つ持てるので、こちらは出てきません（[ADR 0036]）。
    ///
    /// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
    #[error("the store is already open")]
    AlreadyOpen,
    #[error("could not encode board export: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowBoundsState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilterState {
    pub search: String,
    pub tag_id: Option<i64>,
}

// 付随する表示の状態の鍵。SQLite では `app_state` テーブルの、JSON では
// `state` の鍵になります。**同じ綴りを使います**——置き場所を移し替える道を
// 塞がないためです。
pub(crate) const LAST_BOARD_STATE_KEY: &str = "last_board_id";
pub(crate) const NEXT_BOARD_STATE_KEY: &str = "next_board_id";
pub(crate) const WINDOW_BOUNDS_STATE_KEY: &str = "window_bounds";
pub(crate) const FILTER_SEARCH_STATE_KEY: &str = "filter_search";
pub(crate) const FILTER_TAG_STATE_KEY: &str = "filter_tag_id";
pub(crate) const THEME_PREFERENCE_STATE_KEY: &str = "theme_preference";
pub(crate) const SIDEBAR_COLLAPSED_STATE_KEY: &str = "sidebar_collapsed";
pub(crate) const QUICK_CAPTURE_SHORTCUT_STATE_KEY: &str = "quick_capture_shortcut";
pub(crate) const CAPTURE_BOARD_STATE_KEY: &str = "capture_board_id";
pub(crate) const CAPTURE_COLUMN_STATE_KEY: &str = "capture_column_id";

pub(crate) const BOARD_ID_NAMESPACE_SHIFT: u32 = 32;

/// 新しいボードが使う ID の名前空間の先頭。
///
/// ID は `(board_id, id)` の組ではなく主キー 1 本なので、ボードごとに区画を
/// 取ります。こうしておくと、ボードを切り替えたあとに別々の `Board` が
/// 採番しても衝突しません。
pub(crate) fn board_scoped_id(board_id: BoardId) -> i64 {
    board_id
        .checked_shl(BOARD_ID_NAMESPACE_SHIFT)
        .and_then(|id| id.checked_add(1))
        .expect("board ID namespace overflowed")
}

/// 保存されたカードの履歴の 1 件。
///
/// `model::CardEvent` と違って `id` を持ちます。**並び順が書き出しに出る**ので、
/// 置き場所が振った番号をそのまま運びます。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCardEvent {
    pub id: i64,
    pub card_id: i64,
    pub kind: String,
    pub from_column_id: Option<ColumnId>,
    pub to_column_id: Option<ColumnId>,
    pub at: i64,
}

// ---------------------------------------------------------------- JSON の置き場所

/// JSON で持つ盤面 1 つぶん。
///
/// `Board` をそのまま `serde_json` に渡せません。**採番の続きが
/// `#[serde(skip)]` になっている**ためです——あれは webview に見せないための
/// 印で、保存から外してよいという意味ではありません。落とすと、次に開いたとき
/// に同じ ID を振り直します。
///
/// Undo / Redo のスタックは持ちません。SQLite 側も持っておらず、**取り消せる
/// のはアプリを開いている間だけ**という振る舞いをそろえます。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredBoard {
    id: BoardId,
    name: String,
    created_at: i64,
    updated_at: i64,
    next_card_id: i64,
    next_column_id: ColumnId,
    next_tag_id: i64,
    next_checklist_item_id: i64,
    tags: Vec<Tag>,
    archived_cards: Vec<Card>,
    columns: Vec<Column>,
    #[serde(default)]
    events: Vec<StoredCardEvent>,
}

impl StoredBoard {
    fn of(board: &Board, events: Vec<StoredCardEvent>) -> Self {
        Self {
            id: board.id,
            name: board.name.clone(),
            created_at: board.created_at,
            updated_at: board.updated_at,
            next_card_id: board.next_card_id,
            next_column_id: board.next_column_id,
            next_tag_id: board.next_tag_id,
            next_checklist_item_id: board.next_checklist_item_id,
            tags: board.tags.clone(),
            archived_cards: board.archived_cards.clone(),
            columns: board.columns.clone(),
            events,
        }
    }

    fn to_board(&self) -> Board {
        Board {
            id: self.id,
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            next_card_id: self.next_card_id,
            next_column_id: self.next_column_id,
            next_tag_id: self.next_tag_id,
            next_checklist_item_id: self.next_checklist_item_id,
            tags: self.tags.clone(),
            archived_cards: self.archived_cards.clone(),
            columns: self.columns.clone(),
            pending_events: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    fn summary(&self, today: NaiveDate) -> BoardSummary {
        let mut due = DueCounts {
            overdue: 0,
            today: 0,
        };
        // アーカイブ済みと、終わったものの置き場（`columns.done`、[ADR 0038]）に
        // あるカードは数えません（SQLite 側の `archived_at IS NULL` と
        // `columns.done = 0` と同じ）。
        //
        // [ADR 0038]: ../../../docs/adr/0038-a-column-that-means-done.md
        for card in self
            .columns
            .iter()
            .filter(|column| !column.done)
            .flat_map(|column| column.cards.iter())
        {
            match due_status(card.due_date, today) {
                DueStatus::Overdue(_) => due.overdue += 1,
                DueStatus::Today => due.today += 1,
                DueStatus::Soon(_) | DueStatus::Upcoming(_) | DueStatus::None => {}
            }
        }
        BoardSummary {
            id: self.id,
            name: self.name.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            due,
        }
    }
}

/// ブラウザ版の置き場所（[ADR 0036]）。
///
/// **文字列 1 つに収まります。** 呼ぶ側（`crates/web`）が [`JsonStore::encode`]
/// で取り出して `localStorage` に置き、[`JsonStore::decode`] で戻します。ここは
/// どこに置かれるかを知りません——中核が UI もブラウザも知らない、という決めごと
/// （`docs/DESIGN.md`「層の分け方」）はここにも掛かります。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
/// タグの色を自分で決める前の、かつての既定色（ADR 0044）。
///
/// **いまはどこも書き込みません。** 置いてあるものを「色を決めていない」に
/// 戻す移行だけが読みます——SQLite 側は `db/mod.rs` の移行 13、こちらは
/// [`JsonStore::migrate`]。過去の値なので、色を選ぶ側（`web/src/panel/tags.ts`）
/// とは別に、置き場所の側に置いてあります。
pub(crate) const LEGACY_DEFAULT_TAG_COLOR: &str = "#94a3b8";

/// 置いてある形の版。SQLite の `schema_migrations` に当たるものです。
const JSON_STORE_VERSION: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonStore {
    /// ボード。**`id` の昇順に保ちます**——一覧の並びがこれで決まり、SQLite の
    /// `ORDER BY boards.id` と同じ順になります。
    boards: Vec<StoredBoard>,
    /// 付随する表示の状態。鍵は SQLite の `app_state` と同じ綴り。
    state: BTreeMap<String, String>,
    /// 次に振るカードの履歴の番号。
    next_event_id: i64,
    /// 置いてある形の版（[`JSON_STORE_VERSION`]）。**この欄が無いころに置かれた
    /// 文字列では 0 になり**、[`JsonStore::decode`] がそこから進めます。
    #[serde(default)]
    version: i64,
}

/// **`derive` にしません。** 導かれる既定は `next_event_id` が 0 になり、
/// カードの履歴の 1 件目が [`StoredCardEvent::id`] 0 で入ります。SQLite の
/// `AUTOINCREMENT` は 1 から始まるので、そこだけ書き出しがずれます。
impl Default for JsonStore {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonStore {
    /// 何も入っていない置き場所。最初のボードは [`Store::seed_if_empty`] が蒔きます。
    pub fn new() -> Self {
        Self {
            boards: Vec::new(),
            state: BTreeMap::new(),
            next_event_id: 1,
            version: JSON_STORE_VERSION,
        }
    }

    /// 置いておく形の文字列。
    pub fn encode(&self) -> Result<String, StoreError> {
        serde_json::to_string(self).map_err(StoreError::from)
    }

    /// 置いてあった文字列から戻す。
    ///
    /// **読めなければ `None` を返します。** 呼ぶ側は新しい置き場所から始めます
    /// ——読めない文字列を抱えて起動を断ると、ページを開くことすらできません。
    pub fn decode(stored: &str) -> Option<Self> {
        let mut store: Self = serde_json::from_str(stored).ok()?;
        store.migrate();
        Some(store)
    }

    /// 置いてあった形を、いまの版まで進める。
    ///
    /// SQLite 側の `migrate` と同じ仕事です（`db/mod.rs`）。**移行が 2 か所に
    /// あるのは、置き方が 2 つあるからです**——盤面の判断ではないので、
    /// `model.rs` には入りません（[ADR 0036]）。
    ///
    /// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
    fn migrate(&mut self) {
        if self.version < 1 {
            // タグの色を自動で振り分けるようにしたので（ADR 0044）、かつての
            // 既定色で溜まったタグを「色を決めていない」に戻す。SQLite 側の
            // 移行 13 と同じで、当てるのは既定色と一致する行だけ。
            for board in &mut self.boards {
                for tag in &mut board.tags {
                    if tag.color == LEGACY_DEFAULT_TAG_COLOR {
                        tag.color.clear();
                    }
                }
            }
        }
        self.version = JSON_STORE_VERSION;
    }

    fn index_of(&self, board_id: BoardId) -> Option<usize> {
        self.boards.iter().position(|board| board.id == board_id)
    }
}

// ---------------------------------------------------------------- 共通の口

/// 開いている置き場所。
///
/// `crates/app` の `commands` はこれだけを見ます。**中身の判断をここに書きません**
/// ——ここにあるのは「どちらの置き場所に渡すか」だけで、盤面の判断は `model.rs`
/// にあります。
pub enum Store<'a> {
    #[cfg(feature = "sqlite")]
    Sqlite(crate::db::Database),
    Json(std::sync::MutexGuard<'a, JsonStore>),
}

/// どちらの置き場所にも同じ操作を書くための小道具。
///
/// `match` を 20 回書く代わりに、SQLite 側の呼び出しと JSON 側の式を並べます。
macro_rules! either {
    ($self:ident, $sqlite:ident => $on_sqlite:expr, $json:ident => $on_json:expr $(,)?) => {
        match $self {
            #[cfg(feature = "sqlite")]
            Store::Sqlite($sqlite) => $on_sqlite,
            Store::Json($json) => $on_json,
        }
    };
}

impl Store<'_> {
    /// 最後に開いていたボード。無ければ先頭。
    pub fn load_board(&self) -> Result<Board, StoreError> {
        if let Some(board_id) = self.load_last_board_id()? {
            match self.load_board_by_id(board_id) {
                Ok(board) => return Ok(board),
                Err(StoreError::NoBoard) => {}
                Err(error) => return Err(error),
            }
        }
        let first = self
            .load_boards_as_of(Local::now().date_naive())?
            .first()
            .map(|board| board.id)
            .ok_or(StoreError::NoBoard)?;
        self.load_board_by_id(first)
    }

    pub fn load_boards(&self) -> Result<Vec<BoardSummary>, StoreError> {
        self.load_boards_as_of(Local::now().date_naive())
    }

    /// ボード一覧。各行に出す期限の件数まで含めて読む。
    ///
    /// 並びは `id` の昇順です。サイドバーの並びがこれで決まります。
    pub fn load_boards_as_of(&self, today: NaiveDate) -> Result<Vec<BoardSummary>, StoreError> {
        either!(
            self,
            database => database.load_boards_as_of(today),
            json => Ok(json.boards.iter().map(|board| board.summary(today)).collect()),
        )
    }

    pub fn load_board_by_id(&self, id: BoardId) -> Result<Board, StoreError> {
        either!(
            self,
            database => database.load_board_by_id(id),
            json => json
                .index_of(id)
                .map(|at| json.boards[at].to_board())
                .ok_or(StoreError::NoBoard),
        )
    }

    /// 盤面を書き込む。`pending_events` は取り込まれて空になる。
    pub fn save_board(&mut self, board: &mut Board) -> Result<(), StoreError> {
        either!(
            self,
            database => database.save_board(board),
            json => {
                let pending = std::mem::take(&mut board.pending_events);
                let Some(at) = json.index_of(board.id) else {
                    return Err(StoreError::NoBoard);
                };
                let mut events = std::mem::take(&mut json.boards[at].events);
                for event in pending {
                    let id = json.next_event_id;
                    json.next_event_id += 1;
                    events.push(stored_event(id, &event));
                }
                json.boards[at] = StoredBoard::of(board, events);
                Ok(())
            },
        )
    }

    pub fn create_board(&mut self, name: &str) -> Result<Board, StoreError> {
        either!(
            self,
            database => database.create_board(name),
            json => {
                if name.trim().is_empty() {
                    return Err(StoreError::EmptyBoardName);
                }
                let largest = json.boards.iter().map(|board| board.id).max().unwrap_or(0);
                let stored_next = read_id(&json.state, NEXT_BOARD_STATE_KEY)?;
                let board_id = stored_next.unwrap_or(largest + 1).max(largest + 1);
                json.state
                    .insert(NEXT_BOARD_STATE_KEY.to_string(), (board_id + 1).to_string());

                let board = Board::new_empty(
                    board_id,
                    name,
                    board_scoped_id(board_id),
                    board_scoped_id(board_id),
                    board_scoped_id(board_id),
                    board_scoped_id(board_id),
                    chrono::Utc::now().timestamp_millis(),
                );
                json.boards.push(StoredBoard::of(&board, Vec::new()));
                json.boards.sort_by_key(|board| board.id);
                Ok(board)
            },
        )
    }

    /// ボードを消す。**最後の 1 つは消せません。**
    pub fn delete_board(&mut self, board_id: BoardId) -> Result<(), StoreError> {
        either!(
            self,
            database => database.delete_board(board_id),
            json => {
                let Some(at) = json.index_of(board_id) else {
                    return Err(StoreError::NoBoard);
                };
                if json.boards.len() <= 1 {
                    return Err(StoreError::LastBoard);
                }
                json.boards.remove(at);
                if read_id(&json.state, LAST_BOARD_STATE_KEY)? == Some(board_id) {
                    json.state.remove(LAST_BOARD_STATE_KEY);
                }
                Ok(())
            },
        )
    }

    /// 書き出す JSON。組み立ては `export` にあり、ここは履歴を渡すだけ。
    pub fn export_board_json(&self, board: &Board) -> Result<String, StoreError> {
        let events = self.card_events(board.id)?;
        export::render_board_json(board, &events)
    }

    fn card_events(&self, board_id: BoardId) -> Result<Vec<StoredCardEvent>, StoreError> {
        either!(
            self,
            database => database.card_events(board_id),
            json => Ok(json
                .index_of(board_id)
                .map(|at| json.boards[at].events.clone())
                .unwrap_or_default()),
        )
    }

    /// ボードが 1 つも無ければ、最初の盤面を蒔く。
    pub fn seed_if_empty(&mut self) -> Result<(), StoreError> {
        either!(
            self,
            database => { let _ = database; Ok(()) },
            json => {
                if !json.boards.is_empty() {
                    return Ok(());
                }
                let mut board = Board::first_run();
                json.boards.push(StoredBoard::of(&board, Vec::new()));
                let pending = std::mem::take(&mut board.pending_events);
                let events = pending
                    .iter()
                    .enumerate()
                    .map(|(offset, event)| stored_event(json.next_event_id + offset as i64, event))
                    .collect::<Vec<_>>();
                json.next_event_id += events.len() as i64;
                let at = json.boards.len() - 1;
                json.boards[at].events = events;
                json.state
                    .insert(LAST_BOARD_STATE_KEY.to_string(), board.id.to_string());
                Ok(())
            },
        )
    }

    // ------------------------------------------------------------ 付随する状態

    pub fn load_last_board_id(&self) -> Result<Option<BoardId>, StoreError> {
        either!(
            self,
            database => database.load_last_board_id(),
            json => read_id(&json.state, LAST_BOARD_STATE_KEY),
        )
    }

    pub fn set_last_board_id(&mut self, board_id: BoardId) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_last_board_id(board_id),
            json => {
                json.state
                    .insert(LAST_BOARD_STATE_KEY.to_string(), board_id.to_string());
                Ok(())
            },
        )
    }

    pub fn load_window_bounds(&self) -> Result<Option<WindowBoundsState>, StoreError> {
        either!(
            self,
            database => database.load_window_bounds(),
            json => Ok(json
                .state
                .get(WINDOW_BOUNDS_STATE_KEY)
                .and_then(|value| serde_json::from_str(value).ok())),
        )
    }

    pub fn set_window_bounds(&mut self, bounds: WindowBoundsState) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_window_bounds(bounds),
            json => {
                json.state.insert(
                    WINDOW_BOUNDS_STATE_KEY.to_string(),
                    serde_json::to_string(&bounds)?,
                );
                Ok(())
            },
        )
    }

    pub fn load_filter_state(&self) -> Result<FilterState, StoreError> {
        either!(
            self,
            database => database.load_filter_state(),
            json => Ok(FilterState {
                search: json.state.get(FILTER_SEARCH_STATE_KEY).cloned().unwrap_or_default(),
                tag_id: read_id(&json.state, FILTER_TAG_STATE_KEY)?,
            }),
        )
    }

    pub fn set_filter_state(&mut self, state: &FilterState) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_filter_state(state),
            json => {
                json.state
                    .insert(FILTER_SEARCH_STATE_KEY.to_string(), state.search.clone());
                match state.tag_id {
                    Some(tag_id) => json
                        .state
                        .insert(FILTER_TAG_STATE_KEY.to_string(), tag_id.to_string()),
                    None => json.state.remove(FILTER_TAG_STATE_KEY),
                };
                Ok(())
            },
        )
    }

    pub fn load_theme_preference(&self) -> Result<Option<String>, StoreError> {
        either!(
            self,
            database => database.load_theme_preference(),
            json => Ok(json.state.get(THEME_PREFERENCE_STATE_KEY).cloned()),
        )
    }

    pub fn set_theme_preference(&mut self, preference: &str) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_theme_preference(preference),
            json => {
                json.state
                    .insert(THEME_PREFERENCE_STATE_KEY.to_string(), preference.to_string());
                Ok(())
            },
        )
    }

    pub fn load_sidebar_collapsed(&self) -> Result<bool, StoreError> {
        either!(
            self,
            database => database.load_sidebar_collapsed(),
            json => Ok(json
                .state
                .get(SIDEBAR_COLLAPSED_STATE_KEY)
                .is_some_and(|value| value == "1")),
        )
    }

    pub fn set_sidebar_collapsed(&mut self, collapsed: bool) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_sidebar_collapsed(collapsed),
            json => {
                json.state.insert(
                    SIDEBAR_COLLAPSED_STATE_KEY.to_string(),
                    if collapsed { "1" } else { "0" }.to_string(),
                );
                Ok(())
            },
        )
    }

    pub fn load_quick_capture_shortcut(&self) -> Result<Option<String>, StoreError> {
        either!(
            self,
            database => database.load_quick_capture_shortcut(),
            json => Ok(json.state.get(QUICK_CAPTURE_SHORTCUT_STATE_KEY).cloned()),
        )
    }

    pub fn set_quick_capture_shortcut(&mut self, shortcut: Option<&str>) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_quick_capture_shortcut(shortcut),
            json => {
                match shortcut {
                    Some(shortcut) => json.state.insert(
                        QUICK_CAPTURE_SHORTCUT_STATE_KEY.to_string(),
                        shortcut.to_string(),
                    ),
                    None => json.state.remove(QUICK_CAPTURE_SHORTCUT_STATE_KEY),
                };
                Ok(())
            },
        )
    }

    pub fn load_capture_target(&self) -> Result<Option<(BoardId, ColumnId)>, StoreError> {
        either!(
            self,
            database => database.load_capture_target(),
            json => {
                let board_id = read_id(&json.state, CAPTURE_BOARD_STATE_KEY)?;
                let column_id = read_id(&json.state, CAPTURE_COLUMN_STATE_KEY)?;
                Ok(board_id.zip(column_id))
            },
        )
    }

    pub fn set_capture_target(
        &mut self,
        target: Option<(BoardId, ColumnId)>,
    ) -> Result<(), StoreError> {
        either!(
            self,
            database => database.set_capture_target(target),
            json => {
                match target {
                    Some((board_id, column_id)) => {
                        json.state
                            .insert(CAPTURE_BOARD_STATE_KEY.to_string(), board_id.to_string());
                        json.state
                            .insert(CAPTURE_COLUMN_STATE_KEY.to_string(), column_id.to_string());
                    }
                    None => {
                        json.state.remove(CAPTURE_BOARD_STATE_KEY);
                        json.state.remove(CAPTURE_COLUMN_STATE_KEY);
                    }
                }
                Ok(())
            },
        )
    }

    pub fn load_column_name(
        &self,
        board_id: BoardId,
        column_id: ColumnId,
    ) -> Result<Option<String>, StoreError> {
        either!(
            self,
            database => database.load_column_name(board_id, column_id),
            json => Ok(json.index_of(board_id).and_then(|at| {
                json.boards[at]
                    .columns
                    .iter()
                    .find(|column| column.id == column_id)
                    .map(|column| column.name.clone())
            })),
        )
    }
}

fn stored_event(id: i64, event: &CardEvent) -> StoredCardEvent {
    StoredCardEvent {
        id,
        card_id: event.card_id,
        kind: event.kind.as_str().to_string(),
        from_column_id: event.from_column_id,
        to_column_id: event.to_column_id,
        at: event.at,
    }
}

fn read_id(state: &BTreeMap<String, String>, key: &str) -> Result<Option<i64>, StoreError> {
    state
        .get(key)
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| StoreError::InvalidAppState)
        })
        .transpose()
}
