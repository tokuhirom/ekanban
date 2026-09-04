use std::path::Path;

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

use crate::model::{Board, Card, Column, Tag};

const CURRENT_SCHEMA_VERSION: i64 = 7;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("no board exists in the database")]
    NoBoard,
}

pub struct Database {
    connection: Connection,
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
        let (id, name, created_at, updated_at, next_card_id, next_column_id, next_tag_id) = self
            .connection
            .query_row(
                "SELECT id, name, created_at, updated_at, next_card_id, next_column_id,
                        next_tag_id
                 FROM boards ORDER BY id LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
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
        }

        Ok(Board {
            id,
            name,
            created_at,
            updated_at,
            next_card_id,
            next_column_id,
            next_tag_id,
            tags,
            archived_cards,
            columns,
            pending_events: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn save_board(&mut self, board: &mut Board) -> Result<(), DbError> {
        let pending_events = std::mem::take(&mut board.pending_events);
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO boards
             (id, name, created_at, updated_at, next_card_id, next_column_id, next_tag_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               created_at = excluded.created_at,
               updated_at = excluded.updated_at,
               next_card_id = excluded.next_card_id,
               next_column_id = excluded.next_column_id,
               next_tag_id = excluded.next_tag_id",
            params![
                board.id,
                board.name,
                board.created_at,
                board.updated_at,
                board.next_card_id,
                board.next_column_id,
                board.next_tag_id
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
            let mut board = Board::demo();
            self.save_board(&mut board)?;
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::{save_board_snapshot, Database};
    use crate::model::Board;

    #[test]
    fn creates_schema_and_round_trips_a_board() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = Database::open(&path).unwrap();
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
        let database = Database::open(&path).unwrap();
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

        let reloaded = Database::open(&path).unwrap().load_board().unwrap();
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
        let database = Database::open(&path).unwrap();
        let first = database.load_board().unwrap();
        drop(database);

        let database = Database::open(&path).unwrap();
        assert_eq!(database.load_board().unwrap(), first);
    }

    #[test]
    fn saves_a_new_local_board_snapshot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = Database::open(&path).unwrap();
        let mut board = Board::demo();
        board.name = "日本語ボード".to_string();
        database.save_board(&mut board).unwrap();

        assert_eq!(database.load_board().unwrap().name, "日本語ボード");
    }

    #[test]
    fn round_trips_edited_and_deleted_cards() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = Database::open(&path).unwrap();
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
        let mut database = Database::open(&path).unwrap();
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
        let mut database = Database::open(&path).unwrap();
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
        let mut database = Database::open(&path).unwrap();
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
        let mut database = Database::open(&path).unwrap();
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
        let mut database = Database::open(&path).unwrap();
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
        let mut database = Database::open(&path).unwrap();
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

        let database = Database::open(&path).unwrap();
        let board = database.load_board().unwrap();

        assert_eq!(board.next_card_id, 35);
        assert_eq!(board.next_column_id, 13);

        let version = database
            .connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!(version, 7);
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
        let mut database = Database::open(&path).unwrap();
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
    fn round_trips_wip_limits() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = Database::open(&path).unwrap();
        let mut board = database.load_board().unwrap();

        board.set_column_wip_limit(1, Some(5)).unwrap();
        database.save_board(&mut board).unwrap();

        assert_eq!(database.load_board().unwrap().columns[0].wip_limit, Some(5));
    }

    #[test]
    fn round_trips_tags_and_card_assignments() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = Database::open(&path).unwrap();
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
    fn round_trips_archived_cards_and_restoration() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("board.sqlite3");
        let mut database = Database::open(&path).unwrap();
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
