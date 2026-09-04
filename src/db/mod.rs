use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

use crate::model::{Board, Card, Column};

const CURRENT_SCHEMA_VERSION: i64 = 1;

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
        let (id, name, created_at, updated_at) = self
            .connection
            .query_row(
                "SELECT id, name, created_at, updated_at
                 FROM boards ORDER BY id LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or(DbError::NoBoard)?;

        let mut column_statement = self.connection.prepare(
            "SELECT id, board_id, name, position, created_at, updated_at
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
                cards: Vec::new(),
            })
        })?;

        let mut columns = Vec::new();
        for row in column_rows {
            let mut column = row?;
            let mut card_statement = self.connection.prepare(
                "SELECT id, column_id, title, description, position, created_at, updated_at
                 FROM cards WHERE column_id = ?1 ORDER BY position, id",
            )?;
            column.cards = card_statement
                .query_map(params![column.id], |row| {
                    Ok(Card {
                        id: row.get(0)?,
                        column_id: row.get(1)?,
                        title: row.get(2)?,
                        description: row.get(3)?,
                        position: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            columns.push(column);
        }

        Ok(Board {
            id,
            name,
            created_at,
            updated_at,
            columns,
        })
    }

    pub fn save_board(&mut self, board: &Board) -> Result<(), DbError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO boards (id, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               created_at = excluded.created_at,
               updated_at = excluded.updated_at",
            params![board.id, board.name, board.created_at, board.updated_at],
        )?;
        transaction.execute(
            "DELETE FROM cards
             WHERE column_id IN (SELECT id FROM columns WHERE board_id = ?1)",
            params![board.id],
        )?;
        transaction.execute("DELETE FROM columns WHERE board_id = ?1", params![board.id])?;

        for column in &board.columns {
            transaction.execute(
                "INSERT INTO columns
                 (id, board_id, name, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    column.id,
                    board.id,
                    column.name,
                    column.position,
                    column.created_at,
                    column.updated_at
                ],
            )?;
            for card in &column.cards {
                transaction.execute(
                    "INSERT INTO cards
                     (id, column_id, title, description, position, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        card.id,
                        column.id,
                        card.title,
                        card.description,
                        card.position,
                        card.created_at,
                        card.updated_at
                    ],
                )?;
            }
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

        if version < CURRENT_SCHEMA_VERSION {
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
            let board = Board::demo();
            self.save_board(&board)?;
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
    use tempfile::tempdir;

    use super::Database;
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
        database.save_board(&changed).unwrap();

        assert_eq!(database.load_board().unwrap(), changed);
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
        database.save_board(&board).unwrap();

        assert_eq!(database.load_board().unwrap().name, "日本語ボード");
    }
}
