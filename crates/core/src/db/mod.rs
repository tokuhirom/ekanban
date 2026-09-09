//! SQLite の置き場所。**配るアプリはこれです**（`docs/DESIGN.md`「層の分け方」）。
//!
//! 口は `store::Store` にあり、ブラウザ版は同じ口の裏で JSON を使います
//! （[ADR 0036]）。ここに残っているのは SQL とスキーマ移行だけです。
//!
//! [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

use std::path::Path;

use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::model::{
    Board, BoardId, BoardSummary, Card, ChecklistItem, Column, ColumnId, NextIds, PreviousPolicy,
    Recurrence, Schedule, Tag,
};
pub use crate::store::{FilterState, StoredDocument, WindowBoundsState};

use crate::store::{
    board_scoped_id, StoreError, StoredCardEvent, BOARD_ID_NAMESPACE_SHIFT,
    CAPTURE_BOARD_STATE_KEY, CAPTURE_COLUMN_STATE_KEY, DAY_BOUNDARY_HOUR_STATE_KEY,
    DEFAULT_DAY_BOUNDARY_HOUR, FILTER_SEARCH_STATE_KEY, FILTER_TAG_STATE_KEY, LAST_BOARD_STATE_KEY,
    NEXT_BOARD_STATE_KEY, QUICK_CAPTURE_SHORTCUT_STATE_KEY, SIDEBAR_COLLAPSED_STATE_KEY,
    THEME_PREFERENCE_STATE_KEY, WINDOW_BOUNDS_STATE_KEY,
};

const CURRENT_SCHEMA_VERSION: i64 = 15;

pub struct Database {
    connection: Connection,
}

/// Opens the database on the caller's thread and persists a board snapshot.
///
/// Keeping this small operation separate from [`Database`] lets the UI hand a
/// detached board clone to a background executor without moving the SQLite
/// connection that is used during startup.
pub fn save_board_snapshot(path: impl AsRef<Path>, mut board: Board) -> Result<(), StoreError> {
    let mut database = Database::open(path)?;
    database.save_board(&mut board)
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;

        let mut database = Self { connection };
        database.migrate()?;
        database.seed_if_empty()?;
        Ok(database)
    }

    pub fn load_board(&self) -> Result<Board, StoreError> {
        if let Some(board_id) = self.load_last_board_id()? {
            match self.load_board_by_id(board_id) {
                Ok(board) => return Ok(board),
                Err(StoreError::NoBoard) => {}
                Err(error) => return Err(error),
            }
        }
        let id = self
            .connection
            .query_row("SELECT id FROM boards ORDER BY id LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or(StoreError::NoBoard)?;

        self.load_board_by_id(id)
    }

    /// ボード一覧。**名前と並びだけ**です。
    ///
    /// 期限の件数はここで数えません（[ADR 0039]）——盤面を持つのは webview で、
    /// 数えるのに要る材料はそちらにあります（`web/src/model/due.ts`）。数える
    /// 場所が 1 つになったので、[ADR 0038] が気にしていた「3 か所を同じ条件に
    /// 保つ」も要らなくなりました。
    ///
    /// [ADR 0038]: ../../../../docs/adr/0038-a-column-that-means-done.md
    /// [ADR 0039]: ../../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    pub fn load_boards(&self) -> Result<Vec<BoardSummary>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, created_at, updated_at FROM boards ORDER BY id")?;
        let summaries = statement
            .query_map([], |row| {
                Ok(BoardSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(summaries)
    }

    pub fn load_last_board_id(&self) -> Result<Option<BoardId>, StoreError> {
        let value = self
            .connection
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![LAST_BOARD_STATE_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        value
            .map(|value| {
                value
                    .parse::<BoardId>()
                    .map_err(|_| StoreError::InvalidAppState)
            })
            .transpose()
    }

    pub fn set_last_board_id(&self, board_id: BoardId) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![LAST_BOARD_STATE_KEY, board_id.to_string()],
        )?;
        Ok(())
    }

    pub fn load_window_bounds(&self) -> Result<Option<WindowBoundsState>, StoreError> {
        let value = self.load_app_state(WINDOW_BOUNDS_STATE_KEY)?;
        value
            .map(|value| {
                let values = value
                    .split(',')
                    .map(str::parse::<f32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| StoreError::InvalidAppState)?;
                match values.as_slice() {
                    [x, y, width, height] if *width > 0.0 && *height > 0.0 => {
                        Ok(WindowBoundsState {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                        })
                    }
                    _ => Err(StoreError::InvalidAppState),
                }
            })
            .transpose()
    }

    pub fn set_window_bounds(&self, bounds: WindowBoundsState) -> Result<(), StoreError> {
        self.set_app_state(
            WINDOW_BOUNDS_STATE_KEY,
            format!(
                "{},{},{},{}",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
        )
    }

    pub fn load_filter_state(&self) -> Result<FilterState, StoreError> {
        let search = self
            .load_app_state(FILTER_SEARCH_STATE_KEY)?
            .unwrap_or_default();
        let tag_id = self
            .load_app_state(FILTER_TAG_STATE_KEY)?
            .map(|value| {
                value
                    .parse::<i64>()
                    .map_err(|_| StoreError::InvalidAppState)
            })
            .transpose()?;
        Ok(FilterState { search, tag_id })
    }

    pub fn set_filter_state(&self, state: &FilterState) -> Result<(), StoreError> {
        self.set_app_state(FILTER_SEARCH_STATE_KEY, state.search.clone())?;
        match state.tag_id {
            Some(tag_id) => self.set_app_state(FILTER_TAG_STATE_KEY, tag_id.to_string())?,
            None => self.delete_app_state(FILTER_TAG_STATE_KEY)?,
        }
        Ok(())
    }

    pub fn load_theme_preference(&self) -> Result<Option<String>, StoreError> {
        self.load_app_state(THEME_PREFERENCE_STATE_KEY)
    }

    pub fn set_theme_preference(&self, preference: &str) -> Result<(), StoreError> {
        if !matches!(preference, "system" | "light" | "dark") {
            return Err(StoreError::InvalidAppState);
        }
        self.set_app_state(THEME_PREFERENCE_STATE_KEY, preference)
    }

    pub fn load_sidebar_collapsed(&self) -> Result<bool, StoreError> {
        Ok(self
            .load_app_state(SIDEBAR_COLLAPSED_STATE_KEY)?
            .is_some_and(|value| value == "1"))
    }

    pub fn set_sidebar_collapsed(&self, collapsed: bool) -> Result<(), StoreError> {
        self.set_app_state(
            SIDEBAR_COLLAPSED_STATE_KEY,
            if collapsed { "1" } else { "0" },
        )
    }

    /// 日付が変わる時刻（0〜23）。置かれていなければ既定の 4（[ADR 0048]）。
    ///
    /// **読めない値でも失敗させません。** 境界時刻が壊れていても盤面は開ける
    /// べきなので、既定に落とします。書くほうは断るので、ここへ来るのは手で
    /// 書き換えられた行だけです。
    ///
    /// [ADR 0048]: ../../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md
    pub fn load_day_boundary_hour(&self) -> Result<u8, StoreError> {
        Ok(self
            .load_app_state(DAY_BOUNDARY_HOUR_STATE_KEY)?
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|hour| *hour < 24)
            .unwrap_or(DEFAULT_DAY_BOUNDARY_HOUR))
    }

    pub fn set_day_boundary_hour(&self, hour: u8) -> Result<(), StoreError> {
        if hour > 23 {
            return Err(StoreError::InvalidAppState);
        }
        self.set_app_state(DAY_BOUNDARY_HOUR_STATE_KEY, hour.to_string())
    }

    pub fn load_quick_capture_shortcut(&self) -> Result<Option<String>, StoreError> {
        self.load_app_state(QUICK_CAPTURE_SHORTCUT_STATE_KEY)
    }

    /// クイックキャプチャの割り当てを保存する。`None` で解除する。
    pub fn set_quick_capture_shortcut(&self, shortcut: Option<&str>) -> Result<(), StoreError> {
        match shortcut {
            Some(shortcut) => self.set_app_state(QUICK_CAPTURE_SHORTCUT_STATE_KEY, shortcut),
            None => self.delete_app_state(QUICK_CAPTURE_SHORTCUT_STATE_KEY),
        }
    }

    /// クイックキャプチャの入れ先。ボードとカラムの組で持つ。
    pub fn load_capture_target(&self) -> Result<Option<(BoardId, ColumnId)>, StoreError> {
        let Some(board_id) = self.load_app_state(CAPTURE_BOARD_STATE_KEY)? else {
            return Ok(None);
        };
        let Some(column_id) = self.load_app_state(CAPTURE_COLUMN_STATE_KEY)? else {
            return Ok(None);
        };
        match (board_id.parse::<BoardId>(), column_id.parse::<ColumnId>()) {
            (Ok(board_id), Ok(column_id)) => Ok(Some((board_id, column_id))),
            // 壊れた値は無かったことにする。起動を妨げない。
            _ => Ok(None),
        }
    }

    /// キャプチャ先を保存する。`None` で既定（開いているボードの先頭カラム）に戻す。
    pub fn set_capture_target(
        &self,
        target: Option<(BoardId, ColumnId)>,
    ) -> Result<(), StoreError> {
        match target {
            Some((board_id, column_id)) => {
                self.set_app_state(CAPTURE_BOARD_STATE_KEY, board_id.to_string())?;
                self.set_app_state(CAPTURE_COLUMN_STATE_KEY, column_id.to_string())
            }
            None => {
                self.delete_app_state(CAPTURE_BOARD_STATE_KEY)?;
                self.delete_app_state(CAPTURE_COLUMN_STATE_KEY)
            }
        }
    }

    /// カラムの名前。そのボードに属していなければ `None`。
    ///
    /// キャプチャ先がまだ生きているかを、ボードを丸ごと読まずに確かめるために使う。
    pub fn load_column_name(
        &self,
        board_id: BoardId,
        column_id: ColumnId,
    ) -> Result<Option<String>, StoreError> {
        self.connection
            .query_row(
                "SELECT name FROM columns WHERE id = ?1 AND board_id = ?2",
                params![column_id, board_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// 保存してあるカードの履歴。書き出しの JSON に載る。
    pub(crate) fn card_events(
        &self,
        board_id: BoardId,
    ) -> Result<Vec<StoredCardEvent>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, card_id, kind, from_column_id, to_column_id, at
             FROM card_events WHERE board_id = ?1 ORDER BY id",
        )?;
        let events = statement
            .query_map(params![board_id], |row| {
                Ok(StoredCardEvent {
                    id: row.get(0)?,
                    card_id: row.get(1)?,
                    kind: row.get(2)?,
                    from_column_id: row.get(3)?,
                    to_column_id: row.get(4)?,
                    at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    pub fn backup_to(&self, destination: &Path) -> Result<(), StoreError> {
        self.connection
            .execute("VACUUM INTO ?1", params![destination.to_string_lossy()])?;
        Ok(())
    }

    fn load_app_state(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.connection
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    fn set_app_state(&self, key: &str, value: impl Into<String>) -> Result<(), StoreError> {
        self.connection.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.into()],
        )?;
        Ok(())
    }

    fn delete_app_state(&self, key: &str) -> Result<(), StoreError> {
        self.connection
            .execute("DELETE FROM app_state WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn load_board_by_id(&self, id: BoardId) -> Result<Board, StoreError> {
        let (
            id,
            name,
            created_at,
            updated_at,
            next_card_id,
            next_column_id,
            next_tag_id,
            next_checklist_item_id,
            next_recurrence_id,
        ) = self
            .connection
            .query_row(
                "SELECT id, name, created_at, updated_at, next_card_id, next_column_id,
                        next_tag_id, next_checklist_item_id, next_recurrence_id
                 FROM boards WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StoreError::NoBoard)?;

        let mut tag_statement = self.connection.prepare(
            "SELECT id, board_id, name, color, created_at, updated_at
             FROM tags WHERE board_id = ?1 ORDER BY id",
        )?;
        let tags = tag_statement
            .query_map(params![id], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    board_id: row.get(1)?,
                    name: row.get(2)?,
                    color: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut column_statement = self.connection.prepare(
            "SELECT id, board_id, name, position, created_at, updated_at, done
                 FROM columns WHERE board_id = ?1 ORDER BY position, id",
        )?;
        let column_rows = column_statement.query_map(params![id], |row| {
            Ok(Column {
                id: row.get(0)?,
                board_id: row.get(1)?,
                name: row.get(2)?,
                position: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                done: row.get(6)?,
                cards: Vec::new(),
            })
        })?;

        let mut columns = Vec::new();
        for row in column_rows {
            let mut column = row?;
            let mut card_statement = self.connection.prepare(
                "SELECT id, column_id, title, description, position, created_at, updated_at,
                        due_date, archived_at, recurrence_id, occurrence_date
                 FROM cards WHERE column_id = ?1 AND archived_at IS NULL
                 ORDER BY position, id",
            )?;
            column.cards = card_statement
                .query_map(params![column.id], |row| {
                    let due_date = row
                        .get::<_, Option<String>>(7)?
                        .map(|value| {
                            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    7,
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    Ok(Card {
                        id: row.get(0)?,
                        column_id: row.get(1)?,
                        title: row.get(2)?,
                        description: row.get(3)?,
                        position: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        due_date,
                        tag_ids: Vec::new(),
                        checklist_items: Vec::new(),
                        archived_at: row.get(8)?,
                        recurrence_id: row.get(9)?,
                        occurrence_date: stored_date(row.get::<_, Option<String>>(10)?, 10)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for card in &mut column.cards {
                let mut card_tag_statement = self
                    .connection
                    .prepare("SELECT tag_id FROM card_tags WHERE card_id = ?1 ORDER BY tag_id")?;
                card.tag_ids = card_tag_statement
                    .query_map(params![card.id], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                card.checklist_items = self.load_checklist_items(card.id)?;
            }
            columns.push(column);
        }

        let mut archived_statement = self.connection.prepare(
            "SELECT cards.id, cards.column_id, cards.title, cards.description,
                    cards.position, cards.created_at, cards.updated_at,
                    cards.due_date, cards.archived_at, cards.recurrence_id,
                    cards.occurrence_date
             FROM cards
             JOIN columns ON columns.id = cards.column_id
             WHERE columns.board_id = ?1 AND cards.archived_at IS NOT NULL
             ORDER BY cards.archived_at DESC, cards.id",
        )?;
        let archived_rows = archived_statement.query_map(params![id], |row| {
            let due_date = row
                .get::<_, Option<String>>(7)?
                .map(|value| {
                    NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
                })
                .transpose()?;
            Ok(Card {
                id: row.get(0)?,
                column_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                position: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                due_date,
                tag_ids: Vec::new(),
                checklist_items: Vec::new(),
                archived_at: row.get(8)?,
                recurrence_id: row.get(9)?,
                occurrence_date: stored_date(row.get::<_, Option<String>>(10)?, 10)?,
            })
        })?;
        let mut archived_cards = archived_rows.collect::<Result<Vec<_>, _>>()?;
        for card in &mut archived_cards {
            let mut card_tag_statement = self
                .connection
                .prepare("SELECT tag_id FROM card_tags WHERE card_id = ?1 ORDER BY tag_id")?;
            card.tag_ids = card_tag_statement
                .query_map(params![card.id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            card.checklist_items = self.load_checklist_items(card.id)?;
        }

        Ok(Board {
            id,
            name,
            created_at,
            updated_at,
            next_card_id,
            next_column_id,
            next_tag_id,
            next_checklist_item_id,
            next_recurrence_id,
            tags,
            archived_cards,
            columns,
            recurrences: self.load_recurrences(id)?,
            pending_events: Vec::new(),
        })
    }

    /// 繰り返しの定義を読む（#198）。
    ///
    /// 周期は 3 列（`schedule_kind` / `schedule_days` / `schedule_day`）で
    /// 置いてあります。JSON を 1 列に詰めると、置いてある値を SQL から
    /// 読めなくなるためです。
    fn load_recurrences(&self, board_id: BoardId) -> Result<Vec<Recurrence>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, board_id, title, description, column_id, schedule_kind, schedule_days,
                    schedule_day, lead_days, previous, enabled, last_generated_on,
                    created_at, updated_at
             FROM recurrences WHERE board_id = ?1 ORDER BY id",
        )?;
        let rows = statement
            .query_map(params![board_id], |row| {
                let kind: String = row.get(5)?;
                let days: String = row.get(6)?;
                let day: Option<i64> = row.get(7)?;
                let previous: String = row.get(9)?;
                Ok(Recurrence {
                    id: row.get(0)?,
                    board_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    column_id: row.get(4)?,
                    tag_ids: Vec::new(),
                    checklist: Vec::new(),
                    schedule: read_schedule(&kind, &days, day),
                    lead_days: row.get(8)?,
                    previous: PreviousPolicy::parse(&previous),
                    enabled: row.get(10)?,
                    last_generated_on: stored_date(row.get::<_, Option<String>>(11)?, 11)?,
                    created_at: row.get(12)?,
                    updated_at: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut recurrences = rows;
        for recurrence in &mut recurrences {
            let mut tag_statement = self.connection.prepare(
                "SELECT tag_id FROM recurrence_tags WHERE recurrence_id = ?1 ORDER BY tag_id",
            )?;
            recurrence.tag_ids = tag_statement
                .query_map(params![recurrence.id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            let mut item_statement = self.connection.prepare(
                "SELECT text FROM recurrence_checklist_items
                 WHERE recurrence_id = ?1 ORDER BY position",
            )?;
            recurrence.checklist = item_statement
                .query_map(params![recurrence.id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(recurrences)
    }

    fn load_checklist_items(&self, card_id: i64) -> Result<Vec<ChecklistItem>, StoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, card_id, text, checked, position, created_at, updated_at
             FROM checklist_items WHERE card_id = ?1 ORDER BY position, id",
        )?;
        let items = statement
            .query_map(params![card_id], |row| {
                Ok(ChecklistItem {
                    id: row.get(0)?,
                    card_id: row.get(1)?,
                    text: row.get(2)?,
                    checked: row.get(3)?,
                    position: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from);
        items
    }

    pub fn create_board(&mut self, name: impl Into<String>) -> Result<Board, StoreError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(StoreError::EmptyBoardName);
        }

        let now = now();
        let transaction = self.connection.transaction()?;
        let largest_board_id =
            transaction.query_row("SELECT COALESCE(MAX(id), 0) FROM boards", [], |row| {
                row.get::<_, BoardId>(0)
            })?;
        let stored_next_board_id = transaction
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![NEXT_BOARD_STATE_KEY],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(|value| {
                value
                    .parse::<BoardId>()
                    .map_err(|_| StoreError::InvalidAppState)
            })
            .transpose()?;
        let board_id = stored_next_board_id
            .unwrap_or(largest_board_id + 1)
            .max(largest_board_id + 1);
        // IDs are primary keys rather than (board_id, id) pairs. Reserve a
        // namespace per newly-created board so independent Board values can
        // allocate IDs without colliding after a board switch.
        let first = board_scoped_id(board_id);
        let next = NextIds {
            card: first,
            column: first,
            tag: first,
            checklist_item: first,
            recurrence: first,
        };

        // **最初のカラムをここで決めません**（ADR 0038）。名前も、どれを
        // 終わったものの置き場にするかも `Board::new_empty` が持っていて、
        // ここはそれを書き写すだけです。2 か所で決めると片方だけ変わります。
        let board = Board::new_empty(board_id, name, next, now);

        transaction.execute(
            "INSERT INTO boards
             (id, name, created_at, updated_at, next_card_id, next_column_id, next_tag_id,
              next_checklist_item_id, next_recurrence_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                board_id,
                board.name,
                now,
                now,
                next.card,
                board.next_column_id,
                next.tag,
                next.checklist_item,
                next.recurrence
            ],
        )?;
        transaction.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![NEXT_BOARD_STATE_KEY, (board_id + 1).to_string()],
        )?;
        for column in &board.columns {
            transaction.execute(
                "INSERT INTO columns
                 (id, board_id, name, position, created_at, updated_at, done)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    column.id,
                    board_id,
                    column.name,
                    column.position,
                    column.created_at,
                    column.updated_at,
                    column.done
                ],
            )?;
        }
        transaction.commit()?;

        Ok(board)
    }

    pub fn delete_board(&mut self, board_id: BoardId) -> Result<(), StoreError> {
        let transaction = self.connection.transaction()?;
        let board_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM boards WHERE id = ?1)",
            params![board_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !board_exists {
            return Err(StoreError::NoBoard);
        }
        let board_count = transaction.query_row("SELECT COUNT(*) FROM boards", [], |row| {
            row.get::<_, i64>(0)
        })?;
        if board_count <= 1 {
            return Err(StoreError::LastBoard);
        }
        transaction.execute("DELETE FROM boards WHERE id = ?1", params![board_id])?;
        transaction.execute(
            "DELETE FROM app_state WHERE key = ?1 AND value = ?2",
            params![LAST_BOARD_STATE_KEY, board_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// 盤面を書く。**版は確かめません。**
    ///
    /// 盤面を Rust が持っている経路（クイックキャプチャ）はこちらを通ります。
    /// 手元の写しがそのまま最新なので、確かめる相手がいません。
    pub fn save_board(&mut self, board: &mut Board) -> Result<(), StoreError> {
        self.save_board_with_rev(board, None).map(|_| ())
    }

    /// 版を確かめてから書く（[ADR 0040]）。書けたら次の版を返す。
    ///
    /// **合わなければ書きません。** 盤面を持つのが webview になると、ボードの窓と
    /// キャプチャの窓がそれぞれ手元に写しを持ちます。古い写しをそのまま書くと、
    /// もう片方が足したカードが黙って消えます。
    ///
    /// [ADR 0040]: ../../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
    pub fn save_board_at(&mut self, board: &mut Board, expected: i64) -> Result<i64, StoreError> {
        self.save_board_with_rev(board, Some(expected))
    }

    fn save_board_with_rev(
        &mut self,
        board: &mut Board,
        expected: Option<i64>,
    ) -> Result<i64, StoreError> {
        let pending_events = std::mem::take(&mut board.pending_events);
        let transaction = self.connection.transaction()?;
        let current: Option<i64> = transaction
            .query_row(
                "SELECT rev FROM boards WHERE id = ?1",
                params![board.id],
                |row| row.get(0),
            )
            .optional()?;
        if let (Some(expected), Some(current)) = (expected, current) {
            if expected != current {
                return Err(StoreError::Conflict { expected, current });
            }
        }
        let rev = current.map_or(0, |rev| rev + 1);
        transaction.execute(
            "INSERT INTO boards
             (id, name, created_at, updated_at, next_card_id, next_column_id, next_tag_id,
              next_checklist_item_id, next_recurrence_id, rev)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               created_at = excluded.created_at,
               updated_at = excluded.updated_at,
               next_card_id = excluded.next_card_id,
               next_column_id = excluded.next_column_id,
               next_tag_id = excluded.next_tag_id,
               next_checklist_item_id = excluded.next_checklist_item_id,
               next_recurrence_id = excluded.next_recurrence_id,
               rev = excluded.rev",
            params![
                board.id,
                board.name,
                board.created_at,
                board.updated_at,
                board.next_card_id,
                board.next_column_id,
                board.next_tag_id,
                board.next_checklist_item_id,
                board.next_recurrence_id,
                rev
            ],
        )?;

        let active_card_ids = board
            .columns
            .iter()
            .flat_map(|column| column.cards.iter().map(|card| card.id))
            .collect::<Vec<_>>();
        let archived_card_ids = board
            .archived_cards
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        let card_ids = active_card_ids
            .into_iter()
            .chain(archived_card_ids)
            .collect::<Vec<_>>();
        if card_ids.is_empty() {
            transaction.execute(
                "DELETE FROM cards
                 WHERE column_id IN (SELECT id FROM columns WHERE board_id = ?1)",
                params![board.id],
            )?;
        } else {
            let placeholders = std::iter::repeat_n("?", card_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM cards
                 WHERE column_id IN (SELECT id FROM columns WHERE board_id = ?1)
                   AND id NOT IN ({placeholders})"
            );
            let mut values = vec![board.id];
            values.extend(card_ids);
            transaction.execute(&sql, rusqlite::params_from_iter(values))?;
        }

        let column_ids = board
            .columns
            .iter()
            .map(|column| column.id)
            .collect::<Vec<_>>();
        if column_ids.is_empty() {
            transaction.execute("DELETE FROM columns WHERE board_id = ?1", params![board.id])?;
        } else {
            let placeholders = std::iter::repeat_n("?", column_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM columns
                 WHERE board_id = ?1 AND id NOT IN ({placeholders})"
            );
            let mut values = vec![board.id];
            values.extend(column_ids);
            transaction.execute(&sql, rusqlite::params_from_iter(values))?;
        }

        let tag_ids = board.tags.iter().map(|tag| tag.id).collect::<Vec<_>>();
        if tag_ids.is_empty() {
            transaction.execute("DELETE FROM tags WHERE board_id = ?1", params![board.id])?;
        } else {
            let placeholders = std::iter::repeat_n("?", tag_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM tags
                 WHERE board_id = ?1 AND id NOT IN ({placeholders})"
            );
            let mut values = vec![board.id];
            values.extend(tag_ids);
            transaction.execute(&sql, rusqlite::params_from_iter(values))?;
        }

        for column in &board.columns {
            transaction.execute(
                "INSERT INTO columns
                 (id, board_id, name, position, created_at, updated_at, done)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                   board_id = excluded.board_id,
                   name = excluded.name,
                   position = excluded.position,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at,
                   done = excluded.done",
                params![
                    column.id,
                    board.id,
                    column.name,
                    column.position,
                    column.created_at,
                    column.updated_at,
                    column.done
                ],
            )?;
            for card in &column.cards {
                transaction.execute(
                    "INSERT INTO cards
                     (id, column_id, title, description, position, created_at, updated_at,
                      due_date, archived_at, recurrence_id, occurrence_date)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(id) DO UPDATE SET
                       column_id = excluded.column_id,
                       title = excluded.title,
                       description = excluded.description,
                       position = excluded.position,
                       created_at = excluded.created_at,
                       updated_at = excluded.updated_at,
                       due_date = excluded.due_date,
                       archived_at = excluded.archived_at,
                       recurrence_id = excluded.recurrence_id,
                       occurrence_date = excluded.occurrence_date",
                    params![
                        card.id,
                        column.id,
                        card.title,
                        card.description,
                        card.position,
                        card.created_at,
                        card.updated_at,
                        card.due_date
                            .map(|date| date.format("%Y-%m-%d").to_string()),
                        card.archived_at,
                        card.recurrence_id,
                        card.occurrence_date
                            .map(|date| date.format("%Y-%m-%d").to_string())
                    ],
                )?;
            }
        }

        for card in &board.archived_cards {
            transaction.execute(
                "INSERT INTO cards
                 (id, column_id, title, description, position, created_at, updated_at,
                  due_date, archived_at, recurrence_id, occurrence_date)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET
                   column_id = excluded.column_id,
                   title = excluded.title,
                   description = excluded.description,
                   position = excluded.position,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at,
                   due_date = excluded.due_date,
                   archived_at = excluded.archived_at,
                   recurrence_id = excluded.recurrence_id,
                   occurrence_date = excluded.occurrence_date",
                params![
                    card.id,
                    card.column_id,
                    card.title,
                    card.description,
                    card.position,
                    card.created_at,
                    card.updated_at,
                    card.due_date
                        .map(|date| date.format("%Y-%m-%d").to_string()),
                    card.archived_at,
                    card.recurrence_id,
                    card.occurrence_date
                        .map(|date| date.format("%Y-%m-%d").to_string())
                ],
            )?;
        }

        let checklist_item_ids = board
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .chain(board.archived_cards.iter())
            .flat_map(|card| card.checklist_items.iter().map(|item| item.id))
            .collect::<Vec<_>>();
        if checklist_item_ids.is_empty() {
            transaction.execute(
                "DELETE FROM checklist_items
                 WHERE card_id IN (
                     SELECT cards.id FROM cards
                     JOIN columns ON columns.id = cards.column_id
                     WHERE columns.board_id = ?1
                 )",
                params![board.id],
            )?;
        } else {
            let placeholders = std::iter::repeat_n("?", checklist_item_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM checklist_items
                 WHERE card_id IN (
                     SELECT cards.id FROM cards
                     JOIN columns ON columns.id = cards.column_id
                     WHERE columns.board_id = ?1
                 )
                   AND id NOT IN ({placeholders})"
            );
            let mut values = vec![board.id];
            values.extend(checklist_item_ids);
            transaction.execute(&sql, rusqlite::params_from_iter(values))?;
        }

        for card in board
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .chain(board.archived_cards.iter())
        {
            for item in &card.checklist_items {
                transaction.execute(
                    "INSERT INTO checklist_items
                     (id, card_id, text, checked, position, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(id) DO UPDATE SET
                       card_id = excluded.card_id,
                       text = excluded.text,
                       checked = excluded.checked,
                       position = excluded.position,
                       created_at = excluded.created_at,
                       updated_at = excluded.updated_at",
                    params![
                        item.id,
                        card.id,
                        item.text,
                        item.checked,
                        item.position,
                        item.created_at,
                        item.updated_at
                    ],
                )?;
            }
        }

        for tag in &board.tags {
            transaction.execute(
                "INSERT INTO tags
                 (id, board_id, name, color, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   board_id = excluded.board_id,
                   name = excluded.name,
                   color = excluded.color,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at",
                params![
                    tag.id,
                    board.id,
                    tag.name,
                    tag.color,
                    tag.created_at,
                    tag.updated_at
                ],
            )?;
        }

        transaction.execute(
            "DELETE FROM card_tags
             WHERE card_id IN (
                 SELECT cards.id FROM cards
                 JOIN columns ON columns.id = cards.column_id
                 WHERE columns.board_id = ?1
             )",
            params![board.id],
        )?;
        for column in &board.columns {
            for card in &column.cards {
                for tag_id in &card.tag_ids {
                    transaction.execute(
                        "INSERT INTO card_tags (card_id, tag_id) VALUES (?1, ?2)",
                        params![card.id, tag_id],
                    )?;
                }
            }
        }
        for card in &board.archived_cards {
            for tag_id in &card.tag_ids {
                transaction.execute(
                    "INSERT INTO card_tags (card_id, tag_id) VALUES (?1, ?2)",
                    params![card.id, tag_id],
                )?;
            }
        }

        // 繰り返しの定義（#198）。カラムやタグと同じく、**手元に無い行は消して**
        // から書き直します。定義は `recurrence_tags` と
        // `recurrence_checklist_items` を連れているので、`ON DELETE CASCADE`
        // に任せて親だけを見ます。
        let recurrence_ids = board
            .recurrences
            .iter()
            .map(|recurrence| recurrence.id)
            .collect::<Vec<_>>();
        if recurrence_ids.is_empty() {
            transaction.execute(
                "DELETE FROM recurrences WHERE board_id = ?1",
                params![board.id],
            )?;
        } else {
            let placeholders = std::iter::repeat_n("?", recurrence_ids.len())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "DELETE FROM recurrences
                 WHERE board_id = ?1 AND id NOT IN ({placeholders})"
            );
            let mut values = vec![board.id];
            values.extend(recurrence_ids);
            transaction.execute(&sql, rusqlite::params_from_iter(values))?;
        }

        for recurrence in &board.recurrences {
            let (kind, days, day) = write_schedule(&recurrence.schedule);
            transaction.execute(
                "INSERT INTO recurrences
                 (id, board_id, title, description, column_id, schedule_kind, schedule_days,
                  schedule_day, lead_days, previous, enabled, last_generated_on,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(id) DO UPDATE SET
                   board_id = excluded.board_id,
                   title = excluded.title,
                   description = excluded.description,
                   column_id = excluded.column_id,
                   schedule_kind = excluded.schedule_kind,
                   schedule_days = excluded.schedule_days,
                   schedule_day = excluded.schedule_day,
                   lead_days = excluded.lead_days,
                   previous = excluded.previous,
                   enabled = excluded.enabled,
                   last_generated_on = excluded.last_generated_on,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at",
                params![
                    recurrence.id,
                    board.id,
                    recurrence.title,
                    recurrence.description,
                    recurrence.column_id,
                    kind,
                    days,
                    day,
                    recurrence.lead_days,
                    recurrence.previous.as_str(),
                    recurrence.enabled,
                    recurrence
                        .last_generated_on
                        .map(|date| date.format("%Y-%m-%d").to_string()),
                    recurrence.created_at,
                    recurrence.updated_at
                ],
            )?;
            transaction.execute(
                "DELETE FROM recurrence_tags WHERE recurrence_id = ?1",
                params![recurrence.id],
            )?;
            for tag_id in &recurrence.tag_ids {
                transaction.execute(
                    "INSERT INTO recurrence_tags (recurrence_id, tag_id) VALUES (?1, ?2)",
                    params![recurrence.id, tag_id],
                )?;
            }
            transaction.execute(
                "DELETE FROM recurrence_checklist_items WHERE recurrence_id = ?1",
                params![recurrence.id],
            )?;
            for (position, text) in recurrence.checklist.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO recurrence_checklist_items (recurrence_id, position, text)
                     VALUES (?1, ?2, ?3)",
                    params![recurrence.id, i64::try_from(position).unwrap_or(0), text],
                )?;
            }
        }

        for event in &pending_events {
            transaction.execute(
                "INSERT INTO card_events
                 (board_id, card_id, kind, from_column_id, to_column_id, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    board.id,
                    event.card_id,
                    event.kind.as_str(),
                    event.from_column_id,
                    event.to_column_id,
                    event.at
                ],
            )?;
        }

        transaction.commit()?;
        Ok(rev)
    }

    /// 全部のボードを、webview が持つ形で読む（[ADR 0039]）。
    ///
    /// **一覧のためだけに開くのではありません。** 盤面を持つのが webview に
    /// なると、ボードの切り替えも期限の件数も手元で済みます。読むのは起動の
    /// 1 回と、ほかの窓が書いたときだけです。
    ///
    /// [ADR 0039]: ../../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    /// 1 つのボードの版だけを読む。
    ///
    /// 作ったばかりのボードを webview へ返すときに要ります（`create_board`）。
    /// 盤面まで読み直さずに済ませるためのものです。
    pub fn load_board_rev(&self, id: BoardId) -> Result<i64, StoreError> {
        self.connection
            .query_row("SELECT rev FROM boards WHERE id = ?1", params![id], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?
            .ok_or(StoreError::NoBoard)
    }

    pub fn load_documents(&self) -> Result<Vec<StoredDocument>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, rev FROM boards ORDER BY id")?;
        let rows = statement
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, rev)| {
                Ok(StoredDocument {
                    board: self.load_board_by_id(id)?,
                    rev,
                })
            })
            .collect()
    }

    fn migrate(&mut self) -> Result<(), StoreError> {
        self.connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );",
        )?;
        let version = self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get::<_, i64>(0),
        )?;

        if version < 1 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE IF NOT EXISTS boards (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS columns (
                    id INTEGER PRIMARY KEY,
                    board_id INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS cards (
                    id INTEGER PRIMARY KEY,
                    column_id INTEGER NOT NULL REFERENCES columns(id) ON DELETE CASCADE,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    position INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS columns_board_position
                    ON columns(board_id, position);
                CREATE INDEX IF NOT EXISTS cards_column_position
                    ON cards(column_id, position);",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![1, now()],
            )?;
            transaction.commit()?;
        }

        if version < 2 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE boards ADD COLUMN next_card_id INTEGER NOT NULL DEFAULT 1;
                 ALTER TABLE boards ADD COLUMN next_column_id INTEGER NOT NULL DEFAULT 1;",
            )?;
            transaction.execute_batch(
                "UPDATE boards
                 SET next_card_id = COALESCE(
                         (SELECT MAX(cards.id) + 1
                          FROM cards
                          JOIN columns ON columns.id = cards.column_id
                          WHERE columns.board_id = boards.id),
                         1
                     ),
                     next_column_id = COALESCE(
                         (SELECT MAX(columns.id) + 1
                          FROM columns
                          WHERE columns.board_id = boards.id),
                         1
                     );",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![2, now()],
            )?;
            transaction.commit()?;
        }

        if version < 3 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE cards ADD COLUMN due_date TEXT;
                 CREATE INDEX IF NOT EXISTS idx_cards_due_date ON cards(due_date);",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![3, now()],
            )?;
            transaction.commit()?;
        }

        if version < 4 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE columns ADD COLUMN wip_limit INTEGER;
                 CREATE INDEX IF NOT EXISTS idx_columns_wip_limit ON columns(wip_limit);",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![4, now()],
            )?;
            transaction.commit()?;
        }

        if version < 5 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE boards ADD COLUMN next_tag_id INTEGER NOT NULL DEFAULT 1;
                 CREATE TABLE IF NOT EXISTS tags (
                    id INTEGER PRIMARY KEY,
                    board_id INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    color TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS card_tags (
                    card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
                    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                    PRIMARY KEY (card_id, tag_id)
                 );
                 CREATE INDEX IF NOT EXISTS idx_tags_board_id ON tags(board_id);
                 CREATE INDEX IF NOT EXISTS idx_card_tags_tag_id ON card_tags(tag_id);",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![5, now()],
            )?;
            transaction.commit()?;
        }

        if version < 6 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE cards ADD COLUMN archived_at INTEGER;
                 CREATE INDEX IF NOT EXISTS idx_cards_archived_at ON cards(archived_at);",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![6, now()],
            )?;
            transaction.commit()?;
        }

        if version < 7 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE IF NOT EXISTS card_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    board_id INTEGER NOT NULL,
                    card_id INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    from_column_id INTEGER,
                    to_column_id INTEGER,
                    at INTEGER NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_card_events_card
                     ON card_events(card_id, at);
                 CREATE INDEX IF NOT EXISTS idx_card_events_board
                     ON card_events(board_id, at);",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![7, now()],
            )?;
            transaction.commit()?;
        }

        if version < 8 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE IF NOT EXISTS app_state (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                 );",
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![8, now()],
            )?;
            transaction.commit()?;
        }

        if version < 9 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "ALTER TABLE boards ADD COLUMN next_checklist_item_id INTEGER NOT NULL DEFAULT 1;
                 CREATE TABLE IF NOT EXISTS checklist_items (
                    id INTEGER PRIMARY KEY,
                    card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
                    text TEXT NOT NULL,
                    checked INTEGER NOT NULL DEFAULT 0,
                    position INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_checklist_items_card
                     ON checklist_items(card_id, position);",
            )?;
            transaction.execute(
                "UPDATE boards
                 SET next_checklist_item_id = COALESCE(
                     (SELECT MAX(checklist_items.id) + 1
                      FROM checklist_items
                      JOIN cards ON cards.id = checklist_items.card_id
                      JOIN columns ON columns.id = cards.column_id
                      WHERE columns.board_id = boards.id),
                     1
                 );",
                [],
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![9, now()],
            )?;
            transaction.commit()?;
        }

        if version < 10 {
            // 期限での絞り込みを外したので、保存していた選択も残さない。読まれない
            // 行を全ユーザーの DB に置いたままにしない。
            let transaction = self.connection.transaction()?;
            transaction.execute(
                "DELETE FROM app_state WHERE key = ?1",
                params!["filter_due"],
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![10, now()],
            )?;
            transaction.commit()?;
        }

        if version < 11 {
            // WIP 上限をやめたので、列も索引も残さない（ADR 0034）。読まれない
            // 列を全ユーザーの DB に置いたままにしない。SQLite に
            // `DROP COLUMN IF EXISTS` は無いので、有無を先に見る。
            let transaction = self.connection.transaction()?;
            transaction.execute_batch("DROP INDEX IF EXISTS idx_columns_wip_limit;")?;
            let has_wip_limit = transaction.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('columns') WHERE name = 'wip_limit'",
                [],
                |row| row.get::<_, i64>(0),
            )? > 0;
            if has_wip_limit {
                transaction.execute_batch("ALTER TABLE columns DROP COLUMN wip_limit;")?;
            }
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![11, now()],
            )?;
            transaction.commit()?;
        }

        if version < 12 {
            // 終わったものの置き場かどうか（ADR 0038）。**既存のボードでは 1 本も
            // 立てません。** カラム名（「完了」「Done」「済」）から当てると、外した
            // ボードでは何も起きず、当たったボードでは黙って期限の件数が変わります。
            let transaction = self.connection.transaction()?;
            // SQLite に `ADD COLUMN IF NOT EXISTS` は無いので、移行 11 が
            // `DROP COLUMN` の前に有無を見ているのと同じ形で先に見る。移行の
            // テストは、進んだ DB の `schema_migrations` を巻き戻して古い DB を
            // 装うので、列がすでにある状態でここへ来ることがある。
            let has_done = transaction.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('columns') WHERE name = 'done'",
                [],
                |row| row.get::<_, i64>(0),
            )? > 0;
            if !has_done {
                transaction.execute_batch(
                    "ALTER TABLE columns ADD COLUMN done INTEGER NOT NULL DEFAULT 0;",
                )?;
            }
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![12, now()],
            )?;
            transaction.commit()?;
        }

        if version < 13 {
            // タグの色を自動で振り分けるようにしたので（ADR 0044）、**それまでの
            // 既定色で溜まったタグを「色を決めていない」に戻します**。既定色は
            // 選んだ色ではなく、色を決める前の姿だったものです。戻さないと、
            // いままでのタグだけが灰色のまま残り、新しく作ったものとの違いが
            // 「いつ作ったか」でしか説明できなくなります。
            //
            // **当てるのは、当時の既定色そのものと一致する行だけです。** 自分で
            // この灰色を選んだ人はその指定を失いますが（そのタグは自動の色に
            // 移ります）、色見本から選び直せます。
            let transaction = self.connection.transaction()?;
            transaction.execute(
                "UPDATE tags SET color = '' WHERE color = ?1",
                [crate::store::LEGACY_DEFAULT_TAG_COLOR],
            )?;
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![13, now()],
            )?;
            transaction.commit()?;
        }

        if version < 14 {
            // 保存の競合を見るための版（[ADR 0040]）。盤面を持つのが webview に
            // なると、ボードの窓とキャプチャの窓がそれぞれ手元に写しを持ちます。
            // **書き負けが黙って消えるのではなく、`Conflict` として見える**ように
            // するための 1 列です。
            //
            // 既存の行は 0 から始めます。移行のあとで開いた画面が読む版と、
            // 次に書く版が食い違わなければよく、どこから数え始めるかは問いません。
            //
            // [ADR 0040]: ../../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
            let transaction = self.connection.transaction()?;
            let has_rev = transaction.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('boards') WHERE name = 'rev'",
                [],
                |row| row.get::<_, i64>(0),
            )? > 0;
            if !has_rev {
                transaction.execute_batch(
                    "ALTER TABLE boards ADD COLUMN rev INTEGER NOT NULL DEFAULT 0;",
                )?;
            }
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![14, now()],
            )?;
            transaction.commit()?;
        }

        if version < 15 {
            // 繰り返しの定義（#198、[ADR 0049]）。**カードとは別の表**です——
            // 次を出すかどうかを決める真実は定義側の `last_generated_on` に
            // あり、カードから逆引きすると手で消したカードが生えてきます。
            //
            // 周期は 3 列に開きます。JSON を 1 列に詰めると、置いてある値を
            // SQL から読めなくなります。
            //
            // `column_id` にも `cards.recurrence_id` にも**外部キーを張りません**。
            // 入れ先のカラムは消えることがあり（消えたら一番左）、定義を消しても
            // 盤面のカードは残ります。
            //
            // [ADR 0049]: ../../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE IF NOT EXISTS recurrences (
                    id INTEGER PRIMARY KEY,
                    board_id INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    column_id INTEGER NOT NULL,
                    schedule_kind TEXT NOT NULL,
                    schedule_days TEXT NOT NULL DEFAULT '',
                    schedule_day INTEGER,
                    lead_days INTEGER NOT NULL DEFAULT 0,
                    previous TEXT NOT NULL DEFAULT 'archive',
                    enabled INTEGER NOT NULL DEFAULT 1,
                    last_generated_on TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_recurrences_board
                     ON recurrences(board_id, id);
                 CREATE TABLE IF NOT EXISTS recurrence_tags (
                    recurrence_id INTEGER NOT NULL
                        REFERENCES recurrences(id) ON DELETE CASCADE,
                    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                    PRIMARY KEY (recurrence_id, tag_id)
                 );
                 CREATE TABLE IF NOT EXISTS recurrence_checklist_items (
                    recurrence_id INTEGER NOT NULL
                        REFERENCES recurrences(id) ON DELETE CASCADE,
                    position INTEGER NOT NULL,
                    text TEXT NOT NULL,
                    PRIMARY KEY (recurrence_id, position)
                 );",
            )?;
            // SQLite に `ADD COLUMN IF NOT EXISTS` は無いので、移行 12 や 14 と
            // 同じ形で先に有無を見る。移行のテストは進んだ DB の
            // `schema_migrations` を巻き戻して古い DB を装うので、列がすでに
            // ある状態でここへ来ることがある。
            let mut added_the_counter = false;
            for (table, column, definition) in [
                ("boards", "next_recurrence_id", "INTEGER NOT NULL DEFAULT 1"),
                ("cards", "recurrence_id", "INTEGER"),
                ("cards", "occurrence_date", "TEXT"),
            ] {
                let present = transaction.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
                    params![table, column],
                    |row| row.get::<_, i64>(0),
                )? > 0;
                if !present {
                    transaction.execute_batch(&format!(
                        "ALTER TABLE {table} ADD COLUMN {column} {definition};"
                    ))?;
                    added_the_counter = added_the_counter || table == "boards";
                }
            }
            // 採番の続きは、ボードごとの区画の先頭から始めます
            // （`store::board_scoped_id` と同じ数え方）。ID は主キー 1 本なので、
            // 別々のボードが手元で採番しても衝突しないように。
            //
            // **列を足したときにだけ書きます。** 移行のテストは進んだ DB の
            // `schema_migrations` を巻き戻して古い DB を装うので、無条件に書くと
            // すでに数え始めているボードの続きを巻き戻してしまいます。
            if added_the_counter {
                transaction.execute(
                    "UPDATE boards SET next_recurrence_id = (id << ?1) + 1",
                    params![BOARD_ID_NAMESPACE_SHIFT],
                )?;
            }
            transaction.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![CURRENT_SCHEMA_VERSION, now()],
            )?;
            transaction.commit()?;
        }
        Ok(())
    }

    fn seed_if_empty(&mut self) -> Result<(), StoreError> {
        let count = self
            .connection
            .query_row("SELECT COUNT(*) FROM boards", [], |row| {
                row.get::<_, i64>(0)
            })?;
        if count == 0 {
            let mut board = Board::first_run();
            self.save_board(&mut board)?;
            self.set_last_board_id(board.id)?;
        }
        Ok(())
    }
}

/// 置いてある `"YYYY-MM-DD"` を読む。空でなければ日付として成り立っていること。
///
/// 期限・発生日・最後に出した発生日で同じ形を使うので、1 か所に置いてあります。
fn stored_date(value: Option<String>, column: usize) -> rusqlite::Result<Option<NaiveDate>> {
    value
        .map(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    column,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()
}

/// 置いてある 3 列から周期を組み立てる（#198）。
///
/// **知らない綴りは `Daily` に落とします。** 読めない行 1 つでボードが開かなく
/// なるより、いちばん素直な周期として読むほうがましです。
fn read_schedule(kind: &str, days: &str, day: Option<i64>) -> Schedule {
    match kind {
        "weekday" => Schedule::Weekday,
        "weekly" => Schedule::Weekly {
            days: days
                .split(',')
                .filter_map(|value| value.trim().parse::<u8>().ok())
                .filter(|value| *value <= 6)
                .collect(),
        },
        "monthly" => Schedule::Monthly {
            day: u8::try_from(day.unwrap_or(1)).unwrap_or(1).clamp(1, 31),
        },
        "monthlyLast" => Schedule::MonthlyLast,
        _ => Schedule::Daily,
    }
}

/// 周期を、置き場所の 3 列に開く。
fn write_schedule(schedule: &Schedule) -> (&'static str, String, Option<i64>) {
    match schedule {
        Schedule::Weekly { days } => (
            schedule.kind_str(),
            days.iter()
                .map(|day| day.to_string())
                .collect::<Vec<_>>()
                .join(","),
            None,
        ),
        Schedule::Monthly { day } => (schedule.kind_str(), String::new(), Some(i64::from(*day))),
        _ => (schedule.kind_str(), String::new(), None),
    }
}

/// いまの時刻をミリ秒で。
fn now() -> i64 {
    Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use crate::store::{Store, BOARD_ID_NAMESPACE_SHIFT};

    use chrono::NaiveDate;
    use rusqlite::Connection;
    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        board_scoped_id, params, save_board_snapshot, Database, FilterState, StoreError,
        WindowBoundsState, CURRENT_SCHEMA_VERSION, DAY_BOUNDARY_HOUR_STATE_KEY,
        DEFAULT_DAY_BOUNDARY_HOUR,
    };
    use crate::model::{Board, CardEvent, CardEventKind, CardId, ChecklistItem, ColumnId, TagId};
    use crate::MAX_SAFE_JS_INTEGER;

    /// カードの入ったボードを持つデータベースを開く。
    ///
    /// 初回のシードは空の 3 カラムだけ（`Board::first_run`）なので、中身が要る
    /// テストはここを通してテスト用の盤面を載せる。載せるのはファイルが新しい
    /// ときだけ。保存したデータベースを開き直して中身を確かめるテストが多く、
    /// 開くたびに載せ直すと、そのテストが自分で保存した内容を潰す。
    /// webview が積んだことにする履歴 1 件。
    ///
    /// 置き場所は受け取って追記するだけで、中身を見直しません（[ADR 0039]）。
    ///
    /// [ADR 0039]: ../../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    fn event(
        card_id: CardId,
        kind: CardEventKind,
        from_column_id: Option<ColumnId>,
        to_column_id: Option<ColumnId>,
    ) -> CardEvent {
        CardEvent {
            card_id,
            kind,
            from_column_id,
            to_column_id,
            at: 1_700_000_000_000,
        }
    }

    fn open_with_cards(path: &std::path::Path) -> Database {
        let is_new = !path.exists();
        let mut database = Database::open(path).expect("the database opens");
        if is_new {
            let mut fixture = Board::fixture();
            database
                .save_board(&mut fixture)
                .expect("the fixture board is stored");
        }
        database
    }

    /// 日付が変わる時刻（ADR 0048）。**選べるのは 0〜23 の整数だけ。**
    #[test]
    fn stores_the_day_boundary_hour() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);

        assert_eq!(
            database.load_day_boundary_hour().unwrap(),
            DEFAULT_DAY_BOUNDARY_HOUR,
            "何も選ばれていなければ午前 4 時から"
        );

        database.set_day_boundary_hour(0).unwrap();
        assert_eq!(database.load_day_boundary_hour().unwrap(), 0);

        database.set_day_boundary_hour(23).unwrap();
        assert_eq!(database.load_day_boundary_hour().unwrap(), 23);

        assert!(
            matches!(
                database.set_day_boundary_hour(24),
                Err(StoreError::InvalidAppState)
            ),
            "24 時は無い"
        );
        assert_eq!(
            database.load_day_boundary_hour().unwrap(),
            23,
            "断られた値は覚えない"
        );
    }

    /// 手で書き換えられた行を読んでも、盤面は開ける（ADR 0048）。
    #[test]
    fn falls_back_to_the_default_when_the_stored_day_boundary_hour_is_unreadable() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);

        for stored in ["", "よる", "-1", "24", "4.5"] {
            database
                .set_app_state(DAY_BOUNDARY_HOUR_STATE_KEY, stored)
                .unwrap();
            assert_eq!(
                database.load_day_boundary_hour().unwrap(),
                DEFAULT_DAY_BOUNDARY_HOUR,
                "{stored:?} は読めない"
            );
        }
    }

    /// 旧い版のデータベースを開く（ADR 0048）。
    ///
    /// **スキーマの版は上げていません。** `app_state` は鍵と値の表なので、
    /// 行が無いだけの旧 DB はそのまま開き、一律に午前 4 時から始まります。
    /// 0 時に戻したい人は設定で 0 を選べます。
    #[test]
    fn starts_an_existing_database_at_four_in_the_morning() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let before = {
            let database = open_with_cards(&path);
            // 版 13 まで巻き戻し、この設定を知らない DB にする。
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [14])
                .unwrap();
            database
                .connection
                .execute_batch("ALTER TABLE boards DROP COLUMN rev;")
                .unwrap();
            database
                .connection
                .execute(
                    "DELETE FROM app_state WHERE key = ?1",
                    params![DAY_BOUNDARY_HOUR_STATE_KEY],
                )
                .unwrap();
            database.load_board().unwrap()
        };

        let database = open_with_cards(&path);

        assert_eq!(database.load_board().unwrap(), before, "盤面は変わらない");
        assert_eq!(
            database.load_day_boundary_hour().unwrap(),
            4,
            "旧い DB も午前 4 時から始まる"
        );

        // 0 を選べば 0 時境界に戻り、それは覚えられる。
        database.set_day_boundary_hour(0).unwrap();
        assert_eq!(database.load_day_boundary_hour().unwrap(), 0);
    }

    /// 初回起動で見えるもの。読んだ人が消して回らずに使い始められること（#57）。
    #[test]
    fn starts_a_new_database_with_empty_columns() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = Database::open(&path).unwrap();

        let board = database.load_board().unwrap();

        assert_eq!(
            board
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["やること", "進行中", "完了"],
            "the board opens with the three columns a kanban needs"
        );
        assert!(
            board.columns.iter().all(|column| column.cards.is_empty()),
            "and with nothing to delete first: {:?}",
            board.columns
        );
        assert!(board.archived_cards.is_empty());
    }

    #[test]
    fn creates_schema_and_round_trips_a_board() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let original = database.load_board().unwrap();

        assert_eq!(original.columns.len(), 3);
        assert_eq!(original.columns[0].cards.len(), 2);

        let card_id = original.columns[0].cards[0].id;
        let mut changed = original.clone();
        let card = changed.take_card(card_id);
        changed.place_card(card, 3, 0);
        database.save_board(&mut changed).unwrap();

        assert_eq!(database.load_board().unwrap(), changed);
    }

    #[test]
    fn saves_a_detached_board_snapshot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.push_card(1, "バックグラウンド保存", "");
        let expected_title = board.columns[0]
            .cards
            .iter()
            .find(|card| card.id == card_id)
            .unwrap()
            .title
            .clone();

        save_board_snapshot(&path, board).unwrap();

        let reloaded = open_with_cards(&path).load_board().unwrap();
        assert_eq!(
            reloaded.columns[0]
                .cards
                .iter()
                .find(|card| card.id == card_id)
                .unwrap()
                .title,
            expected_title
        );
    }

    #[test]
    fn existing_database_is_migrated_only_once() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);
        let first = database.load_board().unwrap();
        drop(database);

        let database = open_with_cards(&path);
        assert_eq!(database.load_board().unwrap(), first);
    }

    #[test]
    fn lists_loads_and_remembers_multiple_boards() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let first = database.load_board().unwrap();
        let second = database.create_board("仕事").unwrap();

        let summaries = database.load_boards().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, first.id);
        assert_eq!(summaries[1].id, second.id);
        assert_eq!(database.load_board_by_id(second.id).unwrap(), second);

        database.set_last_board_id(second.id).unwrap();
        assert_eq!(database.load_last_board_id().unwrap(), Some(second.id));
        drop(database);

        let database = open_with_cards(&path);
        assert_eq!(database.load_last_board_id().unwrap(), Some(second.id));
        assert_eq!(database.load_board().unwrap().id, second.id);
    }

    /// ボードの一覧は、名前と並びだけを返す（[ADR 0039]）。
    ///
    /// **件数は数えません。** 数えるのに要る材料（盤面）は webview にあり、
    /// SQL と webview の 2 か所で数えると、答えが食い違う日が来ます。
    ///
    /// [ADR 0039]: ../../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    #[test]
    fn lists_every_board_by_name_and_order() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);

        let first = database.load_board().unwrap();
        let second = database.create_board("仕事").unwrap();

        let summaries = database.load_boards().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, first.id);
        assert_eq!(summaries[0].name, first.name);
        assert_eq!(summaries[1].id, second.id);
        assert_eq!(summaries[1].name, "仕事");
    }

    #[test]
    fn persists_window_bounds_and_filter_state_in_app_state() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);
        let bounds = WindowBoundsState {
            x: -120.5,
            y: 42.25,
            width: 1280.0,
            height: 720.0,
        };
        let filters = FilterState {
            search: "日本語".to_string(),
            tag_id: Some(42),
        };

        database.set_window_bounds(bounds).unwrap();
        database.set_filter_state(&filters).unwrap();

        assert_eq!(database.load_window_bounds().unwrap(), Some(bounds));
        assert_eq!(database.load_filter_state().unwrap(), filters);

        let cleared = FilterState::default();
        database.set_filter_state(&cleared).unwrap();
        assert_eq!(database.load_filter_state().unwrap(), cleared);
    }

    #[test]
    fn persists_and_clears_the_quick_capture_shortcut() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);

        assert_eq!(database.load_quick_capture_shortcut().unwrap(), None);

        database
            .set_quick_capture_shortcut(Some("ctrl-alt-shift-cmd-n"))
            .unwrap();
        assert_eq!(
            database.load_quick_capture_shortcut().unwrap(),
            Some("ctrl-alt-shift-cmd-n".to_string())
        );

        database.set_quick_capture_shortcut(None).unwrap();
        assert_eq!(database.load_quick_capture_shortcut().unwrap(), None);
    }

    #[test]
    fn persists_and_clears_the_capture_target() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);
        let board = database.load_boards().unwrap()[0].id;
        let column = database.load_board_by_id(board).unwrap().columns[1].id;

        assert_eq!(database.load_capture_target().unwrap(), None);

        database.set_capture_target(Some((board, column))).unwrap();
        assert_eq!(
            database.load_capture_target().unwrap(),
            Some((board, column))
        );

        database.set_capture_target(None).unwrap();
        assert_eq!(database.load_capture_target().unwrap(), None);
    }

    #[test]
    fn reads_a_column_name_only_within_its_own_board() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let board = database
            .load_board_by_id(database.load_boards().unwrap()[0].id)
            .unwrap();
        let column = &board.columns[0];

        assert_eq!(
            database.load_column_name(board.id, column.id).unwrap(),
            Some(column.name.clone())
        );

        let other = database.create_board("別のボード".to_string()).unwrap();
        assert_eq!(
            database.load_column_name(other.id, column.id).unwrap(),
            None
        );
        assert_eq!(database.load_column_name(board.id, 9999).unwrap(), None);
    }

    #[test]
    fn exports_board_state_and_card_events_as_json() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.columns[0].cards[0].id;
        let tag_id = board.push_tag("書き出し", "#ef4444");
        let item_id = board.next_checklist_item_id;
        board.next_checklist_item_id += 1;

        let card = board.card_mut(card_id);
        card.title = "書き出し対象".to_string();
        card.description = "日本語の説明".to_string();
        card.due_date = NaiveDate::from_ymd_opt(2026, 12, 24);
        card.tag_ids = vec![tag_id];
        card.checklist_items = vec![ChecklistItem {
            id: item_id,
            card_id,
            text: "確認する".to_string(),
            checked: true,
            position: 0,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
        }];
        board.stash_card(card_id, 1_700_000_000_000);
        // 履歴は webview が積む（ADR 0039）。書き出しに出るのはこれ。
        board.adopt_pending_events(vec![event(card_id, CardEventKind::Archived, Some(1), None)]);
        database.save_board(&mut board).unwrap();

        let document: Value =
            serde_json::from_str(&Store::Sqlite(database).export_board_json(&board).unwrap())
                .unwrap();
        assert_eq!(document["format"], "ekanban-board");
        assert_eq!(document["board"]["name"], board.name);
        assert_eq!(document["tags"][0]["name"], "書き出し");
        assert_eq!(document["archived_cards"][0]["title"], "書き出し対象");
        assert_eq!(document["archived_cards"][0]["due_date"], "2026-12-24");
        assert_eq!(document["archived_cards"][0]["tag_ids"][0], tag_id);
        assert_eq!(
            document["archived_cards"][0]["checklist_items"][0]["text"],
            "確認する"
        );
        assert!(document["card_events"].is_array());
        assert!(!document["card_events"].as_array().unwrap().is_empty());
    }

    #[test]
    fn backs_up_database_to_a_new_sqlite_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let backup_path = directory.path().join("backup.sqlite3");
        let database = open_with_cards(&path);

        database.backup_to(&backup_path).unwrap();

        let backup = Database::open(&backup_path).unwrap();
        assert_eq!(backup.load_board().unwrap(), database.load_board().unwrap());
    }

    #[test]
    fn creates_boards_with_non_overlapping_item_ids() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut first = database.load_board().unwrap();
        let mut second = database.create_board("別のボード").unwrap();

        let first_card = first.push_card(1, "一枚目", "");
        let second_column = second.columns[0].id;
        let second_card = second.push_card(second_column, "二枚目", "");
        assert_ne!(first_card, second_card);

        database.save_board(&mut first).unwrap();
        database.save_board(&mut second).unwrap();
        assert_eq!(
            database.load_board_by_id(first.id).unwrap().columns[0]
                .cards
                .len(),
            3
        );
        assert_eq!(
            database.load_board_by_id(second.id).unwrap().columns[0]
                .cards
                .len(),
            1
        );
    }

    #[test]
    fn refuses_to_delete_the_last_board() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let first = database.load_board().unwrap();
        let second = database.create_board("削除対象").unwrap();

        database.delete_board(first.id).unwrap();
        assert!(matches!(
            database.load_board_by_id(first.id),
            Err(super::StoreError::NoBoard)
        ));
        assert!(matches!(
            database.delete_board(second.id),
            Err(super::StoreError::LastBoard)
        ));
    }

    #[test]
    fn does_not_reuse_deleted_board_ids() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let first = database.load_board().unwrap();
        let second = database.create_board("一時ボード").unwrap();

        database.delete_board(second.id).unwrap();
        let replacement = database.create_board("新しいボード").unwrap();

        assert_eq!(first.id, 1);
        assert_eq!(second.id, 2);
        assert_eq!(replacement.id, 3);
    }

    #[test]
    fn rejects_empty_board_names() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);

        assert!(matches!(
            database.create_board("  "),
            Err(super::StoreError::EmptyBoardName)
        ));
    }

    #[test]
    fn saves_a_new_local_board_snapshot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = Board::fixture();
        board.name = "日本語ボード".to_string();
        database.save_board(&mut board).unwrap();

        assert_eq!(database.load_board().unwrap().name, "日本語ボード");
    }

    #[test]
    fn round_trips_edited_and_deleted_cards() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let edited_id = board.columns[0].cards[0].id;
        let deleted_id = board.columns[0].cards[1].id;

        let card = board.card_mut(edited_id);
        card.title = "編集済み".to_string();
        card.description = "新しい説明".to_string();
        board.take_card(deleted_id);
        database.save_board(&mut board).unwrap();

        let reloaded = database.load_board().unwrap();
        assert_eq!(reloaded.columns[0].cards.len(), 1);
        assert_eq!(reloaded.columns[0].cards[0].id, edited_id);
        assert_eq!(reloaded.columns[0].cards[0].title, "編集済み");
        assert_eq!(reloaded.columns[0].cards[0].description, "新しい説明");
    }

    #[test]
    fn preserves_created_at_when_saving_a_moved_card() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.columns[0].cards[0].id;
        let created_at = board.columns[0].cards[0].created_at;

        let card = board.take_card(card_id);
        board.place_card(card, 3, 0);
        database.save_board(&mut board).unwrap();

        let reloaded = database.load_board().unwrap();
        let moved_card = reloaded.columns[2]
            .cards
            .iter()
            .find(|card| card.id == card_id)
            .unwrap();
        assert_eq!(moved_card.created_at, created_at);
    }

    /// 繰り返しの定義が、周期もテンプレートも変わらずに戻ってくる（#198）。
    ///
    /// 周期は 3 列に開いて置いてあるので、**開いて畳んで同じもの**であることを
    /// ここで押さえます。
    #[test]
    fn round_trips_recurrences_with_their_schedules_and_templates() {
        use crate::model::{PreviousPolicy, Schedule};

        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let tag_id = board.push_tag("毎日", "");

        let daily = board.push_recurrence("メールを見る", Schedule::Daily);
        {
            let recurrence = board.recurrence_mut(daily);
            recurrence.description = "受信箱を空にする".to_string();
            recurrence.tag_ids = vec![tag_id];
            recurrence.checklist = vec!["未読を見る".to_string(), "返信する".to_string()];
            recurrence.previous = PreviousPolicy::Delete;
            recurrence.last_generated_on = NaiveDate::from_ymd_opt(2026, 2, 14);
        }
        let weekly = board.push_recurrence("週次の振り返り", Schedule::Weekly { days: vec![0, 4] });
        board.recurrence_mut(weekly).lead_days = 2;
        let monthly = board.push_recurrence("社内報", Schedule::Monthly { day: 31 });
        board.recurrence_mut(monthly).enabled = false;
        let month_end = board.push_recurrence("締め", Schedule::MonthlyLast);
        let weekday = board.push_recurrence("朝の予定確認", Schedule::Weekday);

        // 出来たカードは、定義と発生日を参照として持つ。
        let card_id = board.push_card(1, "メールを見る", "");
        {
            let card = board.card_mut(card_id);
            card.recurrence_id = Some(daily);
            card.occurrence_date = NaiveDate::from_ymd_opt(2026, 2, 14);
        }

        board.validate().expect("the board holds together");
        database.save_board(&mut board).unwrap();

        let read = Database::open(&path).unwrap().load_board().unwrap();
        assert_eq!(read.recurrences, board.recurrences, "定義がそのまま戻る");
        assert_eq!(read.next_recurrence_id, board.next_recurrence_id);
        let stored_card = read
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .expect("the card is there");
        assert_eq!(stored_card.recurrence_id, Some(daily));
        assert_eq!(
            stored_card.occurrence_date,
            NaiveDate::from_ymd_opt(2026, 2, 14)
        );
        assert_eq!(
            read.recurrences
                .iter()
                .map(|recurrence| recurrence.id)
                .collect::<Vec<_>>(),
            vec![daily, weekly, monthly, month_end, weekday]
        );

        // 手元から消した定義は、次の保存で行ごと消える（差分保存）。
        let mut fewer = read;
        fewer
            .recurrences
            .retain(|recurrence| recurrence.id != weekly);
        database.save_board(&mut fewer).unwrap();
        let read = Database::open(&path).unwrap().load_board().unwrap();
        assert_eq!(read.recurrences.len(), 4);
        let orphans: i64 = Database::open(&path)
            .unwrap()
            .connection
            .query_row(
                "SELECT COUNT(*) FROM recurrence_checklist_items
                 WHERE recurrence_id NOT IN (SELECT id FROM recurrences)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0, "連れていた行も一緒に消える");
    }

    /// 繰り返しを知らない DB（版 14）を開いても、盤面はそのまま（#198）。
    #[test]
    fn adds_the_recurrence_tables_when_migrating_a_version_fourteen_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");

        let before = {
            let database = open_with_cards(&path);
            let board = database.load_board().unwrap();
            // 版 14 まで巻き戻し、繰り返しを知らない DB にする。
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [15])
                .unwrap();
            database
                .connection
                .execute_batch(
                    "DROP TABLE recurrence_checklist_items;
                     DROP TABLE recurrence_tags;
                     DROP TABLE recurrences;
                     ALTER TABLE boards DROP COLUMN next_recurrence_id;
                     ALTER TABLE cards DROP COLUMN recurrence_id;
                     ALTER TABLE cards DROP COLUMN occurrence_date;",
                )
                .unwrap();
            board
        };

        let mut database = open_with_cards(&path);
        let read = database.load_board().unwrap();

        assert_eq!(read.recurrences, Vec::new(), "繰り返しは 1 つも立たない");
        assert_eq!(read.columns, before.columns, "盤面は変わらない");
        assert_eq!(
            read.next_recurrence_id,
            board_scoped_id(read.id),
            "採番はボードごとの区画の先頭から"
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            CURRENT_SCHEMA_VERSION
        );

        // 移行したあとの DB にも、繰り返しをそのまま置ける。
        let mut board = read;
        board.push_recurrence("メールを見る", crate::model::Schedule::Daily);
        database.save_board(&mut board).unwrap();
        assert_eq!(
            Database::open(&path)
                .unwrap()
                .load_board()
                .unwrap()
                .recurrences,
            board.recurrences
        );
    }

    #[test]
    fn saves_card_lifecycle_events_and_clears_pending_events() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.push_card(1, "履歴を記録", "");
        let card = board.take_card(card_id);
        board.place_card(card, 2, 0);
        board.stash_card(card_id, 1_700_000_000_000);
        // 履歴を積むのは webview（ADR 0039）。置き場所は受け取って追記するだけ。
        board.adopt_pending_events(vec![
            event(card_id, CardEventKind::Created, None, Some(1)),
            event(card_id, CardEventKind::Moved, Some(1), Some(2)),
            event(card_id, CardEventKind::Archived, Some(2), None),
        ]);

        database.save_board(&mut board).unwrap();
        assert!(board.pending_events.is_empty());

        let events = database
            .connection
            .prepare(
                "SELECT kind, from_column_id, to_column_id
                 FROM card_events WHERE card_id = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([card_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            events,
            vec![
                ("created".to_string(), None, Some(1)),
                ("moved".to_string(), Some(1), Some(2)),
                ("archived".to_string(), Some(2), None),
            ]
        );
    }

    #[test]
    fn keeps_events_when_a_card_is_deleted() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.push_card(1, "削除するカード", "");
        board.take_card(card_id);
        board.adopt_pending_events(vec![
            event(card_id, CardEventKind::Created, None, Some(1)),
            event(card_id, CardEventKind::Deleted, Some(1), None),
        ]);

        database.save_board(&mut board).unwrap();

        let events = database
            .connection
            .prepare("SELECT kind FROM card_events WHERE card_id = ?1 ORDER BY id")
            .unwrap()
            .query_map([card_id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(events, ["created", "deleted"]);
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM cards WHERE id = ?1",
                    [card_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn archives_a_column_with_one_event_per_card() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_ids = board.columns[0]
            .cards
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();

        for card_id in &card_ids {
            board.stash_card(*card_id, 1_700_000_000_000);
        }
        board.adopt_pending_events(
            card_ids
                .iter()
                .map(|card_id| event(*card_id, CardEventKind::Archived, Some(1), None))
                .collect(),
        );
        database.save_board(&mut board).unwrap();

        let event_count = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM card_events
                 WHERE kind = 'archived' AND card_id IN (?1, ?2)",
                [card_ids[0], card_ids[1]],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(event_count, 2);
    }

    #[test]
    fn drops_pending_events_when_saving_fails() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.push_card(1, "保存に失敗するカード", "");
        board.adopt_pending_events(vec![event(card_id, CardEventKind::Created, None, Some(1))]);
        board.columns[0]
            .cards
            .iter_mut()
            .find(|card| card.id == card_id)
            .unwrap()
            .tag_ids
            .push(999);

        assert!(database.save_board(&mut board).is_err());
        assert!(board.pending_events.is_empty());

        board.columns[0]
            .cards
            .iter_mut()
            .find(|card| card.id == card_id)
            .unwrap()
            .tag_ids
            .clear();
        database.save_board(&mut board).unwrap();
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM card_events WHERE card_id = ?1",
                    [card_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn drops_the_saved_due_filter_when_migrating_a_version_nine_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        {
            // v9 まで進んだ DB を作り、当時の絞り込みの選択を残しておく。
            let database = open_with_cards(&path);
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [10])
                .unwrap();
            database
                .connection
                .execute(
                    "INSERT INTO app_state (key, value) VALUES ('filter_due', '2')",
                    [],
                )
                .unwrap();
        }

        let database = open_with_cards(&path);

        assert_eq!(database.load_app_state("filter_due").unwrap(), None);
        assert_eq!(
            database.load_filter_state().unwrap(),
            FilterState::default()
        );
        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn adds_the_done_column_when_migrating_a_version_eleven_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        {
            // v11 まで進んだ DB を作り、当時の形（`done` の無い `columns`）に戻す。
            let database = open_with_cards(&path);
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [12])
                .unwrap();
            database
                .connection
                .execute_batch("ALTER TABLE columns DROP COLUMN done;")
                .unwrap();
        }

        let database = open_with_cards(&path);
        let board = database.load_board().unwrap();

        // **移行では 1 本も立てません**（ADR 0038）。「完了」という名前から
        // 当てると、当たったボードでは黙って期限の件数が変わります。
        assert!(
            board.columns.iter().all(|column| !column.done),
            "the migration guesses nothing"
        );
        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    /// 既定色で溜まったタグを、自動の色に戻す（ADR 0044）。**自分で選んだ色は
    /// 版の列を足すだけの移行。**盤面には触らない**（[ADR 0040]）。
    ///
    /// 既存のボードは 0 から数え始める。どこから数え始めるかは問わない——
    /// 読んだ版と次に書く版が食い違わなければよい。
    ///
    /// [ADR 0040]: ../../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
    #[test]
    fn adds_the_revision_column_when_migrating_a_version_thirteen_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let before = {
            let database = open_with_cards(&path);
            let board = database.load_board().unwrap();
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [14])
                .unwrap();
            database
                .connection
                .execute_batch("ALTER TABLE boards DROP COLUMN rev;")
                .unwrap();
            board
        };

        let mut database = open_with_cards(&path);

        assert_eq!(database.load_board().unwrap(), before, "盤面は変わらない");
        let documents = database.load_documents().unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].rev, 0, "移行したボードは 0 から数え始める");

        // 数え始めたあとは、書くたびに 1 つ進み、古い版は断られる。
        let mut board = database.load_board().unwrap();
        assert_eq!(database.save_board_at(&mut board, 0).unwrap(), 1);
        assert!(matches!(
            database.save_board_at(&mut board, 0),
            Err(crate::store::StoreError::Conflict {
                expected: 0,
                current: 1
            })
        ));

        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    /// そのまま**で、当てるのは当時の既定色と一致する行だけ。
    #[test]
    fn clears_the_old_default_tag_color_when_migrating_a_version_twelve_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let (defaulted, chosen) = {
            // v12 まで進んだ DB に、当時の既定色のタグと、選んだ色のタグを置く。
            let mut database = open_with_cards(&path);
            let mut board = database.load_board().unwrap();
            let defaulted = board.push_tag("既定色のまま", "#94a3b8");
            let chosen = board.push_tag("自分で選んだ", "#ef4444");
            database.save_board(&mut board).unwrap();
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [13])
                .unwrap();
            (defaulted, chosen)
        };

        let database = open_with_cards(&path);
        let board = database.load_board().unwrap();

        let color = |tag_id: TagId| {
            board
                .tags
                .iter()
                .find(|tag| tag.id == tag_id)
                .expect("the tag survives the migration")
                .color
                .clone()
        };
        assert_eq!(color(defaulted), "", "the old default becomes automatic");
        assert_eq!(color(chosen), "#ef4444", "a chosen color is left alone");
        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn round_trips_the_done_flag_on_a_column() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let column_id = board.columns[2].id;

        board.column_mut(column_id).done = true;
        database.save_board(&mut board).unwrap();
        let reloaded = database.load_board().unwrap();
        assert!(reloaded.columns[2].done);
        assert!(!reloaded.columns[0].done);

        let mut reloaded = reloaded;
        reloaded.column_mut(column_id).done = false;
        database.save_board(&mut reloaded).unwrap();
        assert!(!database.load_board().unwrap().columns[2].done);
    }

    #[test]
    fn creates_a_board_with_a_place_for_finished_work() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);

        let created = database.create_board("2 つ目").unwrap();
        let loaded = database.load_board_by_id(created.id).unwrap();

        // 書いた形と読み直した形が同じであること。カラムの名前と `done` を
        // 決めているのは `Board::new_empty` の 1 か所（ADR 0038）。
        assert_eq!(created.columns.len(), 2);
        assert_eq!(loaded.columns.len(), 2);
        assert_eq!(loaded.columns[0].name, "やること");
        assert!(!loaded.columns[0].done);
        assert_eq!(loaded.columns[1].name, "完了");
        assert!(loaded.columns[1].done);
        assert_eq!(loaded.next_column_id, created.next_column_id);
    }

    #[test]
    fn migrates_a_version_one_database_and_initializes_id_counters() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                );
                INSERT INTO schema_migrations (version, applied_at) VALUES (1, 1);
                CREATE TABLE boards (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE TABLE columns (
                    id INTEGER PRIMARY KEY,
                    board_id INTEGER NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE TABLE cards (
                    id INTEGER PRIMARY KEY,
                    column_id INTEGER NOT NULL REFERENCES columns(id) ON DELETE CASCADE,
                    title TEXT NOT NULL,
                    description TEXT NOT NULL DEFAULT '',
                    position INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                INSERT INTO boards (id, name, created_at, updated_at)
                    VALUES (7, '旧ボード', 10, 11);
                INSERT INTO columns
                    (id, board_id, name, position, created_at, updated_at)
                    VALUES (12, 7, '列', 0, 10, 11);
                INSERT INTO cards
                    (id, column_id, title, description, position, created_at, updated_at)
                    VALUES (34, 12, 'カード', '', 0, 10, 11);",
            )
            .unwrap();
        drop(connection);

        let database = open_with_cards(&path);
        let board = database.load_board().unwrap();

        assert_eq!(board.next_card_id, 35);
        assert_eq!(board.next_column_id, 13);
        assert_eq!(board.next_checklist_item_id, 1);

        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        assert_eq!(board.columns[0].cards[0].id, 34);
        assert_eq!(board.columns[0].cards[0].due_date, None);
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM card_events", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            0
        );
    }

    #[test]
    fn round_trips_due_dates_and_clear_values() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_with_due_date = board.columns[0].cards[0].id;
        let card_without_due_date = board.columns[0].cards[1].id;
        let due_date = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();

        board.card_mut(card_with_due_date).due_date = Some(due_date);
        database.save_board(&mut board).unwrap();
        let reloaded = database.load_board().unwrap();
        assert_eq!(reloaded.columns[0].cards[0].due_date, Some(due_date));
        assert_eq!(reloaded.columns[0].cards[1].due_date, None);

        let mut reloaded = reloaded;
        reloaded.card_mut(card_with_due_date).due_date = None;
        reloaded.card_mut(card_without_due_date).due_date = Some(due_date);
        database.save_board(&mut reloaded).unwrap();
        let final_board = database.load_board().unwrap();
        assert_eq!(final_board.columns[0].cards[0].due_date, None);
        assert_eq!(final_board.columns[0].cards[1].due_date, Some(due_date));
    }

    #[test]
    fn round_trips_the_sidebar_collapsed_state() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);

        assert!(!database.load_sidebar_collapsed().unwrap());

        database.set_sidebar_collapsed(true).unwrap();
        assert!(database.load_sidebar_collapsed().unwrap());

        database.set_sidebar_collapsed(false).unwrap();
        assert!(!database.load_sidebar_collapsed().unwrap());
    }

    #[test]
    fn drops_the_wip_limit_column_when_migrating_a_version_ten_database() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        {
            // v10 まで進んだ DB を作り、当時の WIP 上限の列と索引を戻しておく。
            let database = open_with_cards(&path);
            database
                .connection
                .execute("DELETE FROM schema_migrations WHERE version >= ?1", [11])
                .unwrap();
            database
                .connection
                .execute_batch(
                    "ALTER TABLE columns ADD COLUMN wip_limit INTEGER;
                     CREATE INDEX idx_columns_wip_limit ON columns(wip_limit);
                     UPDATE columns SET wip_limit = 3;",
                )
                .unwrap();
        }

        let database = open_with_cards(&path);

        assert!(database.load_board().is_ok());
        assert!(
            database
                .connection
                .prepare("SELECT wip_limit FROM columns")
                .is_err(),
            "the column is gone"
        );
        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn round_trips_tags_and_card_assignments() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let tag_id = board.push_tag("重要", "#ef4444");
        let card_id = board.columns[0].cards[0].id;
        board.card_mut(card_id).tag_ids = vec![tag_id];
        database.save_board(&mut board).unwrap();

        let reloaded = database.load_board().unwrap();
        assert_eq!(reloaded.tags[0].name, "重要");
        assert_eq!(reloaded.columns[0].cards[0].tag_ids, vec![tag_id]);
    }

    #[test]
    fn round_trips_checklist_items_and_removes_deleted_items() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.columns[0].cards[0].id;
        let first_item = board.next_checklist_item_id;
        board.next_checklist_item_id += 2;
        let card = board.card_mut(card_id);
        card.title = "チェックリスト付き".to_string();
        card.description = "説明".to_string();
        card.checklist_items = vec![
            item(first_item, card_id, "一つ目", false, 0),
            item(first_item + 1, card_id, "二つ目", true, 1),
        ];
        database.save_board(&mut board).unwrap();

        let reloaded = database.load_board().unwrap();
        let card = &reloaded.columns[0].cards[0];
        assert_eq!(card.checklist_items.len(), 2);
        assert_eq!(card.checklist_items[1].text, "二つ目");
        assert!(card.checklist_items[1].checked);

        let mut reloaded = reloaded;
        reloaded.take_card(card_id);
        database.save_board(&mut reloaded).unwrap();
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM checklist_items WHERE card_id = ?1",
                    [card_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn round_trips_archived_cards_and_restoration() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.columns[0].cards[0].id;
        let tag_id = board.push_tag("保管", "#64748b");
        board.card_mut(card_id).tag_ids = vec![tag_id];

        board.stash_card(card_id, 1_700_000_000_000);
        database.save_board(&mut board).unwrap();

        let mut reloaded = database.load_board().unwrap();
        assert_eq!(reloaded.columns[0].cards.len(), 1);
        assert_eq!(reloaded.archived_cards[0].id, card_id);
        assert!(reloaded.archived_cards[0].archived_at.is_some());
        assert_eq!(reloaded.archived_cards[0].tag_ids, vec![tag_id]);

        reloaded.tags.retain(|tag| tag.id != tag_id);
        reloaded.card_mut(card_id).tag_ids.clear();
        database.save_board(&mut reloaded).unwrap();
        reloaded = database.load_board().unwrap();
        assert!(reloaded.archived_cards[0].tag_ids.is_empty());

        reloaded.unstash_card(card_id);
        database.save_board(&mut reloaded).unwrap();
        let restored = database.load_board().unwrap();
        assert!(restored.archived_cards.is_empty());
        assert!(restored.columns[0]
            .cards
            .iter()
            .any(|card| card.id == card_id));
    }

    /// チェック項目 1 つ。土台を組み立てるためだけのもの。
    fn item(id: i64, card_id: CardId, text: &str, checked: bool, position: i64) -> ChecklistItem {
        ChecklistItem {
            id,
            card_id,
            text: text.to_string(),
            checked,
            position,
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
        }
    }

    /// ID は JSON の数値として webview に渡る（`docs/DESIGN.md`「境界を越える値」）。
    ///
    /// ボードごとに `board_id << 32` で ID の名前空間を切っているので、ボードの
    /// 番号が伸びるほど ID の桁が上がる。2^53 を超えると JavaScript 側で丸められ、
    /// **例外も出ないまま別のカードを指す**。ここが上限との距離を書き留めておく
    /// 唯一の場所なので、`BOARD_ID_NAMESPACE_SHIFT` を触るときはここを見ること。
    #[test]
    fn board_id_namespaces_stay_inside_the_javascript_safe_integer_range() {
        let last_safe_board_id = MAX_SAFE_JS_INTEGER >> BOARD_ID_NAMESPACE_SHIFT;
        assert_eq!(
            last_safe_board_id, 2_097_151,
            "2^21 - 1 のボードまでは安全に扱える"
        );
        assert!(board_scoped_id(last_safe_board_id) <= MAX_SAFE_JS_INTEGER);
        assert!(
            board_scoped_id(last_safe_board_id + 1) > MAX_SAFE_JS_INTEGER,
            "この 1 つ先から JavaScript が丸めはじめる"
        );
    }

    /// 実際に作ったボードの ID が上限の内側にあること。
    ///
    /// 上の計算だけだと、`create_board` が別の採番に変わったときに気づけない。
    #[test]
    fn ids_handed_out_by_the_database_are_safe_javascript_integers() {
        let directory = tempdir().expect("a temporary directory is available");
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        database
            .create_board("2 つ目")
            .expect("a second board is created");

        for summary in database.load_boards().expect("the board list loads") {
            let board = database
                .load_board_by_id(summary.id)
                .expect("the board loads");
            let mut ids = vec![board.id];
            ids.extend(board.tags.iter().map(|tag| tag.id));
            for column in &board.columns {
                ids.push(column.id);
                for card in &column.cards {
                    ids.push(card.id);
                    ids.extend(card.checklist_items.iter().map(|item| item.id));
                }
            }
            for id in ids {
                assert!(
                    id > 0 && id <= MAX_SAFE_JS_INTEGER,
                    "{id} は JavaScript が誤差なく扱える範囲の外"
                );
            }
        }
    }

    /// webview に渡る JSON の形（`docs/DESIGN.md`「境界を越える値」）。
    ///
    /// 鍵は camelCase、期限は `"YYYY-MM-DD"` の文字列、時刻は数値。**時刻の単位は
    /// ミリ秒**で、秒ではない（`now()` が `as_millis`）。ここを取り違えると
    /// webview は 1970 年を描く。
    #[test]
    fn the_board_crosses_the_boundary_in_the_shape_the_webview_expects() {
        let mut board = Board::fixture();
        let card_id = board.columns[0].cards[0].id;
        board.card_mut(card_id).due_date = NaiveDate::from_ymd_opt(2026, 3, 4);

        let value: Value = serde_json::to_value(&board).expect("the board serializes");
        let card = &value["columns"][0]["cards"][0];

        assert_eq!(card["dueDate"], Value::from("2026-03-04"));
        assert!(card["columnId"].is_number(), "ID は JSON の数値");
        assert!(card["tagIds"].is_array());
        assert!(card["checklistItems"].is_array());
        assert!(value["archivedCards"].is_array());

        let created_at = card["createdAt"].as_i64().expect("epoch の数値");
        assert!(
            created_at > 1_600_000_000_000,
            "時刻はエポックからのミリ秒。秒だと {created_at} はこの桁にならない"
        );

        assert!(
            value.get("nextCardId").is_none(),
            "採番のカウンタは渡さない。ID を割り当てるのは Rust だけ（ADR 0018）"
        );
        assert!(value.get("pendingEvents").is_none());
    }
}
