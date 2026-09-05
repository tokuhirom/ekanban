use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use thiserror::Error;

use crate::model::{Board, BoardId, BoardSummary, Card, ChecklistItem, Column, ColumnId, Tag};

const CURRENT_SCHEMA_VERSION: i64 = 10;

const LAST_BOARD_STATE_KEY: &str = "last_board_id";
const NEXT_BOARD_STATE_KEY: &str = "next_board_id";
const WINDOW_BOUNDS_STATE_KEY: &str = "window_bounds";
const FILTER_SEARCH_STATE_KEY: &str = "filter_search";
const FILTER_TAG_STATE_KEY: &str = "filter_tag_id";
const THEME_PREFERENCE_STATE_KEY: &str = "theme_preference";
const SIDEBAR_COLLAPSED_STATE_KEY: &str = "sidebar_collapsed";
const QUICK_CAPTURE_SHORTCUT_STATE_KEY: &str = "quick_capture_shortcut";
const CAPTURE_BOARD_STATE_KEY: &str = "capture_board_id";
const CAPTURE_COLUMN_STATE_KEY: &str = "capture_column_id";
const BOARD_ID_NAMESPACE_SHIFT: u32 = 32;

#[derive(Debug, Error)]
pub enum DbError {
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
    #[error("could not encode board export: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct Database {
    connection: Connection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowBoundsState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterState {
    pub search: String,
    pub tag_id: Option<i64>,
}

/// Opens the database on the caller's thread and persists a board snapshot.
///
/// Keeping this small operation separate from [`Database`] lets the UI hand a
/// detached board clone to a background executor without moving the SQLite
/// connection that is used during startup.
pub fn save_board_snapshot(path: impl AsRef<Path>, mut board: Board) -> Result<(), DbError> {
    let mut database = Database::open(path)?;
    database.save_board(&mut board)
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;

        let mut database = Self { connection };
        database.migrate()?;
        database.seed_if_empty()?;
        Ok(database)
    }

    pub fn load_board(&self) -> Result<Board, DbError> {
        if let Some(board_id) = self.load_last_board_id()? {
            match self.load_board_by_id(board_id) {
                Ok(board) => return Ok(board),
                Err(DbError::NoBoard) => {}
                Err(error) => return Err(error),
            }
        }
        let id = self
            .connection
            .query_row("SELECT id FROM boards ORDER BY id LIMIT 1", [], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or(DbError::NoBoard)?;

        self.load_board_by_id(id)
    }

    pub fn load_boards(&self) -> Result<Vec<BoardSummary>, DbError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, created_at, updated_at
             FROM boards ORDER BY id",
        )?;
        let summaries = statement
            .query_map([], |row| {
                Ok(BoardSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DbError::from);
        summaries
    }

    pub fn load_last_board_id(&self) -> Result<Option<BoardId>, DbError> {
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
                    .map_err(|_| DbError::InvalidAppState)
            })
            .transpose()
    }

    pub fn set_last_board_id(&self, board_id: BoardId) -> Result<(), DbError> {
        self.connection.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![LAST_BOARD_STATE_KEY, board_id.to_string()],
        )?;
        Ok(())
    }

    pub fn load_window_bounds(&self) -> Result<Option<WindowBoundsState>, DbError> {
        let value = self.load_app_state(WINDOW_BOUNDS_STATE_KEY)?;
        value
            .map(|value| {
                let values = value
                    .split(',')
                    .map(str::parse::<f32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| DbError::InvalidAppState)?;
                match values.as_slice() {
                    [x, y, width, height] if *width > 0.0 && *height > 0.0 => {
                        Ok(WindowBoundsState {
                            x: *x,
                            y: *y,
                            width: *width,
                            height: *height,
                        })
                    }
                    _ => Err(DbError::InvalidAppState),
                }
            })
            .transpose()
    }

    pub fn set_window_bounds(&self, bounds: WindowBoundsState) -> Result<(), DbError> {
        self.set_app_state(
            WINDOW_BOUNDS_STATE_KEY,
            format!(
                "{},{},{},{}",
                bounds.x, bounds.y, bounds.width, bounds.height
            ),
        )
    }

    pub fn load_filter_state(&self) -> Result<FilterState, DbError> {
        let search = self
            .load_app_state(FILTER_SEARCH_STATE_KEY)?
            .unwrap_or_default();
        let tag_id = self
            .load_app_state(FILTER_TAG_STATE_KEY)?
            .map(|value| value.parse::<i64>().map_err(|_| DbError::InvalidAppState))
            .transpose()?;
        Ok(FilterState { search, tag_id })
    }

    pub fn set_filter_state(&self, state: &FilterState) -> Result<(), DbError> {
        self.set_app_state(FILTER_SEARCH_STATE_KEY, state.search.clone())?;
        match state.tag_id {
            Some(tag_id) => self.set_app_state(FILTER_TAG_STATE_KEY, tag_id.to_string())?,
            None => self.delete_app_state(FILTER_TAG_STATE_KEY)?,
        }
        Ok(())
    }

    pub fn load_theme_preference(&self) -> Result<Option<String>, DbError> {
        self.load_app_state(THEME_PREFERENCE_STATE_KEY)
    }

    pub fn set_theme_preference(&self, preference: &str) -> Result<(), DbError> {
        if !matches!(preference, "system" | "light" | "dark") {
            return Err(DbError::InvalidAppState);
        }
        self.set_app_state(THEME_PREFERENCE_STATE_KEY, preference)
    }

    pub fn load_sidebar_collapsed(&self) -> Result<bool, DbError> {
        Ok(self
            .load_app_state(SIDEBAR_COLLAPSED_STATE_KEY)?
            .is_some_and(|value| value == "1"))
    }

    pub fn set_sidebar_collapsed(&self, collapsed: bool) -> Result<(), DbError> {
        self.set_app_state(
            SIDEBAR_COLLAPSED_STATE_KEY,
            if collapsed { "1" } else { "0" },
        )
    }

    pub fn load_quick_capture_shortcut(&self) -> Result<Option<String>, DbError> {
        self.load_app_state(QUICK_CAPTURE_SHORTCUT_STATE_KEY)
    }

    /// クイックキャプチャの割り当てを保存する。`None` で解除する。
    pub fn set_quick_capture_shortcut(&self, shortcut: Option<&str>) -> Result<(), DbError> {
        match shortcut {
            Some(shortcut) => self.set_app_state(QUICK_CAPTURE_SHORTCUT_STATE_KEY, shortcut),
            None => self.delete_app_state(QUICK_CAPTURE_SHORTCUT_STATE_KEY),
        }
    }

    /// クイックキャプチャの入れ先。ボードとカラムの組で持つ。
    pub fn load_capture_target(&self) -> Result<Option<(BoardId, ColumnId)>, DbError> {
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
    pub fn set_capture_target(&self, target: Option<(BoardId, ColumnId)>) -> Result<(), DbError> {
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
    ) -> Result<Option<String>, DbError> {
        self.connection
            .query_row(
                "SELECT name FROM columns WHERE id = ?1 AND board_id = ?2",
                params![column_id, board_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn export_board_json(&self, board: &Board) -> Result<String, DbError> {
        let mut event_statement = self.connection.prepare(
            "SELECT id, card_id, kind, from_column_id, to_column_id, at
             FROM card_events WHERE board_id = ?1 ORDER BY id",
        )?;
        let events = event_statement
            .query_map(params![board.id], |row| {
                Ok(json!({
                    "id": row.get::<_, i64>(0)?,
                    "board_id": board.id,
                    "card_id": row.get::<_, i64>(1)?,
                    "kind": row.get::<_, String>(2)?,
                    "from_column_id": row.get::<_, Option<i64>>(3)?,
                    "to_column_id": row.get::<_, Option<i64>>(4)?,
                    "at": row.get::<_, i64>(5)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

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
                    "wip_limit": column.wip_limit,
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
        .map_err(DbError::from)
    }

    pub fn backup_to(&self, destination: &Path) -> Result<(), DbError> {
        self.connection
            .execute("VACUUM INTO ?1", params![destination.to_string_lossy()])?;
        Ok(())
    }

    fn load_app_state(&self, key: &str) -> Result<Option<String>, DbError> {
        self.connection
            .query_row(
                "SELECT value FROM app_state WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(DbError::from)
    }

    fn set_app_state(&self, key: &str, value: impl Into<String>) -> Result<(), DbError> {
        self.connection.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.into()],
        )?;
        Ok(())
    }

    fn delete_app_state(&self, key: &str) -> Result<(), DbError> {
        self.connection
            .execute("DELETE FROM app_state WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn load_board_by_id(&self, id: BoardId) -> Result<Board, DbError> {
        let (
            id,
            name,
            created_at,
            updated_at,
            next_card_id,
            next_column_id,
            next_tag_id,
            next_checklist_item_id,
        ) = self
            .connection
            .query_row(
                "SELECT id, name, created_at, updated_at, next_card_id, next_column_id,
                        next_tag_id, next_checklist_item_id
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
                    ))
                },
            )
            .optional()?
            .ok_or(DbError::NoBoard)?;

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
            "SELECT id, board_id, name, position, created_at, updated_at, wip_limit
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
                wip_limit: row.get(6)?,
                cards: Vec::new(),
            })
        })?;

        let mut columns = Vec::new();
        for row in column_rows {
            let mut column = row?;
            let mut card_statement = self.connection.prepare(
                "SELECT id, column_id, title, description, position, created_at, updated_at,
                        due_date, archived_at
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
                    cards.due_date, cards.archived_at
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
            tags,
            archived_cards,
            columns,
            pending_events: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    fn load_checklist_items(&self, card_id: i64) -> Result<Vec<ChecklistItem>, DbError> {
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
            .map_err(DbError::from);
        items
    }

    pub fn create_board(&mut self, name: impl Into<String>) -> Result<Board, DbError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(DbError::EmptyBoardName);
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
                    .map_err(|_| DbError::InvalidAppState)
            })
            .transpose()?;
        let board_id = stored_next_board_id
            .unwrap_or(largest_board_id + 1)
            .max(largest_board_id + 1);
        // IDs are primary keys rather than (board_id, id) pairs. Reserve a
        // namespace per newly-created board so independent Board values can
        // allocate IDs without colliding after a board switch.
        let next_card_id = board_scoped_id(board_id);
        let first_column_id = board_scoped_id(board_id);
        let next_tag_id = board_scoped_id(board_id);
        let next_checklist_item_id = board_scoped_id(board_id);

        transaction.execute(
            "INSERT INTO boards
             (id, name, created_at, updated_at, next_card_id, next_column_id, next_tag_id,
              next_checklist_item_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                board_id,
                name,
                now,
                now,
                next_card_id,
                first_column_id + 1,
                next_tag_id,
                next_checklist_item_id
            ],
        )?;
        transaction.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![NEXT_BOARD_STATE_KEY, (board_id + 1).to_string()],
        )?;
        transaction.execute(
            "INSERT INTO columns
             (id, board_id, name, position, created_at, updated_at, wip_limit)
             VALUES (?1, ?2, ?3, 0, ?4, ?5, NULL)",
            params![first_column_id, board_id, "やること", now, now],
        )?;
        transaction.commit()?;

        Ok(Board::new_empty(
            board_id,
            name,
            next_card_id,
            first_column_id,
            next_tag_id,
            next_checklist_item_id,
            now,
        ))
    }

    pub fn delete_board(&mut self, board_id: BoardId) -> Result<(), DbError> {
        let transaction = self.connection.transaction()?;
        let board_exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM boards WHERE id = ?1)",
            params![board_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !board_exists {
            return Err(DbError::NoBoard);
        }
        let board_count = transaction.query_row("SELECT COUNT(*) FROM boards", [], |row| {
            row.get::<_, i64>(0)
        })?;
        if board_count <= 1 {
            return Err(DbError::LastBoard);
        }
        transaction.execute("DELETE FROM boards WHERE id = ?1", params![board_id])?;
        transaction.execute(
            "DELETE FROM app_state WHERE key = ?1 AND value = ?2",
            params![LAST_BOARD_STATE_KEY, board_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn save_board(&mut self, board: &mut Board) -> Result<(), DbError> {
        let pending_events = std::mem::take(&mut board.pending_events);
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO boards
             (id, name, created_at, updated_at, next_card_id, next_column_id, next_tag_id,
              next_checklist_item_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               created_at = excluded.created_at,
               updated_at = excluded.updated_at,
               next_card_id = excluded.next_card_id,
               next_column_id = excluded.next_column_id,
               next_tag_id = excluded.next_tag_id,
               next_checklist_item_id = excluded.next_checklist_item_id",
            params![
                board.id,
                board.name,
                board.created_at,
                board.updated_at,
                board.next_card_id,
                board.next_column_id,
                board.next_tag_id,
                board.next_checklist_item_id
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
                 (id, board_id, name, position, created_at, updated_at, wip_limit)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                   board_id = excluded.board_id,
                   name = excluded.name,
                   position = excluded.position,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at,
                   wip_limit = excluded.wip_limit",
                params![
                    column.id,
                    board.id,
                    column.name,
                    column.position,
                    column.created_at,
                    column.updated_at,
                    column.wip_limit
                ],
            )?;
            for card in &column.cards {
                transaction.execute(
                    "INSERT INTO cards
                     (id, column_id, title, description, position, created_at, updated_at,
                      due_date, archived_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT(id) DO UPDATE SET
                       column_id = excluded.column_id,
                       title = excluded.title,
                       description = excluded.description,
                       position = excluded.position,
                       created_at = excluded.created_at,
                       updated_at = excluded.updated_at,
                       due_date = excluded.due_date,
                       archived_at = excluded.archived_at",
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
                        card.archived_at
                    ],
                )?;
            }
        }

        for card in &board.archived_cards {
            transaction.execute(
                "INSERT INTO cards
                 (id, column_id, title, description, position, created_at, updated_at,
                  due_date, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                   column_id = excluded.column_id,
                   title = excluded.title,
                   description = excluded.description,
                   position = excluded.position,
                   created_at = excluded.created_at,
                   updated_at = excluded.updated_at,
                   due_date = excluded.due_date,
                   archived_at = excluded.archived_at",
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
                    card.archived_at
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
        Ok(())
    }

    fn migrate(&mut self) -> Result<(), DbError> {
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
                params![CURRENT_SCHEMA_VERSION, now()],
            )?;
            transaction.commit()?;
        }
        Ok(())
    }

    fn seed_if_empty(&mut self) -> Result<(), DbError> {
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

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock is before UNIX epoch")
        .as_millis() as i64
}

fn board_scoped_id(board_id: BoardId) -> i64 {
    board_id
        .checked_shl(BOARD_ID_NAMESPACE_SHIFT)
        .and_then(|id| id.checked_add(1))
        .expect("board ID namespace overflowed")
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rusqlite::Connection;
    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        save_board_snapshot, Database, FilterState, WindowBoundsState, CURRENT_SCHEMA_VERSION,
    };
    use crate::model::{Board, ChecklistItemDraft};

    /// カードの入ったボードを持つデータベースを開く。
    ///
    /// 初回のシードは空の 3 カラムだけ（`Board::first_run`）なので、中身が要る
    /// テストはここを通してテスト用の盤面を載せる。載せるのはファイルが新しい
    /// ときだけ。保存したデータベースを開き直して中身を確かめるテストが多く、
    /// 開くたびに載せ直すと、そのテストが自分で保存した内容を潰す。
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
        changed.move_card(card_id, 3, 0).unwrap();
        database.save_board(&mut changed).unwrap();

        assert_eq!(database.load_board().unwrap(), changed);
    }

    #[test]
    fn saves_a_detached_board_snapshot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.add_card(1, "バックグラウンド保存", "").unwrap();
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
        let tag_id = board.add_tag("書き出し", "#ef4444").unwrap();

        board
            .update_card_details_with_checklist(
                card_id,
                "書き出し対象",
                "日本語の説明",
                Some(NaiveDate::from_ymd_opt(2026, 12, 24).unwrap()),
                vec![tag_id],
                vec![ChecklistItemDraft {
                    id: None,
                    text: "確認する".to_string(),
                    checked: true,
                }],
            )
            .unwrap();
        board.archive_card(card_id).unwrap();
        database.save_board(&mut board).unwrap();

        let document: Value =
            serde_json::from_str(&database.export_board_json(&board).unwrap()).unwrap();
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

        let first_card = first.add_card(1, "一枚目", "").unwrap();
        let second_card = second.add_card(second.columns[0].id, "二枚目", "").unwrap();
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
            Err(super::DbError::NoBoard)
        ));
        assert!(matches!(
            database.delete_board(second.id),
            Err(super::DbError::LastBoard)
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
            Err(super::DbError::EmptyBoardName)
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

        board
            .update_card(edited_id, "編集済み", "新しい説明")
            .unwrap();
        board.remove_card(deleted_id).unwrap();
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

        board.move_card(card_id, 3, 0).unwrap();
        database.save_board(&mut board).unwrap();

        let reloaded = database.load_board().unwrap();
        let moved_card = reloaded.columns[2]
            .cards
            .iter()
            .find(|card| card.id == card_id)
            .unwrap();
        assert_eq!(moved_card.created_at, created_at);
    }

    #[test]
    fn saves_undo_and_redo_without_repeating_lifecycle_events() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.columns[0].cards[0].id;
        let initial_event_count = lifecycle_event_count(&database, card_id);

        board.move_card(card_id, 2, 0).unwrap();
        database.save_board(&mut board).unwrap();
        board.undo().unwrap();
        database.save_board(&mut board).unwrap();
        assert_eq!(board.columns[0].cards[0].id, card_id);
        assert_eq!(
            lifecycle_event_count(&database, card_id),
            initial_event_count + 1
        );

        board.redo().unwrap();
        database.save_board(&mut board).unwrap();
        assert_eq!(board.columns[1].cards[0].id, card_id);
        assert_eq!(
            lifecycle_event_count(&database, card_id),
            initial_event_count + 1
        );
    }

    #[test]
    fn saves_card_lifecycle_events_and_clears_pending_events() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let card_id = board.add_card(1, "履歴を記録", "").unwrap();
        board.move_card(card_id, 2, 0).unwrap();
        board.archive_card(card_id).unwrap();

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
        let card_id = board.add_card(1, "削除するカード", "").unwrap();
        board.delete_card(card_id).unwrap();

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

        assert_eq!(board.archive_column(1).unwrap(), card_ids.len());
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
        let card_id = board.add_card(1, "保存に失敗するカード", "").unwrap();
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
                .execute("DELETE FROM schema_migrations WHERE version = ?1", [10])
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

        board
            .set_card_due_date(card_with_due_date, Some(due_date))
            .unwrap();
        database.save_board(&mut board).unwrap();
        let reloaded = database.load_board().unwrap();
        assert_eq!(reloaded.columns[0].cards[0].due_date, Some(due_date));
        assert_eq!(reloaded.columns[0].cards[1].due_date, None);

        let mut reloaded = reloaded;
        reloaded
            .set_card_due_date(card_with_due_date, None)
            .unwrap();
        reloaded
            .set_card_due_date(card_without_due_date, Some(due_date))
            .unwrap();
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
    fn round_trips_wip_limits() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();

        board.set_column_wip_limit(1, Some(5)).unwrap();
        database.save_board(&mut board).unwrap();

        assert_eq!(database.load_board().unwrap().columns[0].wip_limit, Some(5));
    }

    #[test]
    fn round_trips_tags_and_card_assignments() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = open_with_cards(&path);
        let mut board = database.load_board().unwrap();
        let tag_id = board.add_tag("重要", "#ef4444").unwrap();
        let card_id = board.columns[0].cards[0].id;
        board.set_card_tags(card_id, vec![tag_id]).unwrap();
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
        board
            .update_card_details_with_checklist(
                card_id,
                "チェックリスト付き",
                "説明",
                None,
                Vec::new(),
                vec![
                    ChecklistItemDraft {
                        id: None,
                        text: "一つ目".to_string(),
                        checked: false,
                    },
                    ChecklistItemDraft {
                        id: None,
                        text: "二つ目".to_string(),
                        checked: true,
                    },
                ],
            )
            .unwrap();
        database.save_board(&mut board).unwrap();

        let reloaded = database.load_board().unwrap();
        let card = &reloaded.columns[0].cards[0];
        assert_eq!(card.checklist_items.len(), 2);
        assert_eq!(card.checklist_items[1].text, "二つ目");
        assert!(card.checklist_items[1].checked);

        let mut reloaded = reloaded;
        reloaded.delete_card(card_id).unwrap();
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
        let tag_id = board.add_tag("保管", "#64748b").unwrap();
        board.set_card_tags(card_id, vec![tag_id]).unwrap();

        board.archive_card(card_id).unwrap();
        database.save_board(&mut board).unwrap();

        let mut reloaded = database.load_board().unwrap();
        assert_eq!(reloaded.columns[0].cards.len(), 1);
        assert_eq!(reloaded.archived_cards[0].id, card_id);
        assert!(reloaded.archived_cards[0].archived_at.is_some());
        assert_eq!(reloaded.archived_cards[0].tag_ids, vec![tag_id]);

        reloaded.remove_tag(tag_id).unwrap();
        database.save_board(&mut reloaded).unwrap();
        reloaded = database.load_board().unwrap();
        assert!(reloaded.archived_cards[0].tag_ids.is_empty());

        reloaded.restore_card(card_id).unwrap();
        database.save_board(&mut reloaded).unwrap();
        let restored = database.load_board().unwrap();
        assert!(restored.archived_cards.is_empty());
        assert!(restored.columns[0]
            .cards
            .iter()
            .any(|card| card.id == card_id));
    }

    fn lifecycle_event_count(database: &Database, card_id: i64) -> i64 {
        database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM card_events WHERE card_id = ?1",
                [card_id],
                |row| row.get(0),
            )
            .unwrap()
    }
}
