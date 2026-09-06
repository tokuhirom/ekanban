//! 起動のたびに、その日ぶんの控えを 1 つ残す。
//!
//! 守る相手は人的ミスとアプリのバグ。Undo はセッション限りで、手で取るコピーは
//! 思い出した人しか取らないので、それだけでは「カラムを消したまま終了した」を
//! 戻せない。
//!
//! 世代を**起動ごと**ではなく**日ごと**に刻むのが、この module の肝になっている。
//! 起動ごとに取ると、壊したあとに何度か起動し直した時点で、無事だった世代が
//! 押し出されて消える。バックアップが一番要る場面でローテーションが正本を食う。
//! 日付を名前に入れておけば、同じ日に何度起動しても残るのはその日の最初の 1 回
//! （＝まだ壊していない状態）で、前の日ぶんも無傷のまま残る。
//!
//! 控えの中身を作るのは `Database::backup_to`（`VACUUM INTO`）で、ここが決めるのは
//! いつ取るか・どこに置くか・いくつ残すかだけ。SQL は `src/db/` に閉じたままにする。

use std::path::{Path, PathBuf};

use chrono::NaiveDate;

use crate::db::{Database, DbError};

/// 残す世代の数。1 日に 1 つしか取らないので、毎日起動する人で 7 日ぶん。
pub const GENERATIONS: usize = 7;

/// 控えのファイル名に入れる日付の書式。
const DATE_FORMAT: &str = "%Y-%m-%d";

/// 控えの拡張子。データベースと同じにして、そのまま `EKANBAN_DATABASE` に
/// 渡せるようにする。
const EXTENSION: &str = "sqlite3";

/// 書きかけの控えに付ける拡張子。`EXTENSION` と違うので、世代としては数えない。
const PARTIAL_EXTENSION: &str = "part";

/// データベースのファイル名が読めなかったときに使う名前。
const FALLBACK_STEM: &str = "ekanban";

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not copy the database: {0}")]
    Database(#[from] DbError),
}

fn io_error(path: &Path, source: std::io::Error) -> BackupError {
    BackupError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// 控えを置くディレクトリ。データベースの隣の `backups/`。
///
/// 同じディスクに置くので、ディスク障害や誤ったディレクトリ削除からは守れない。
/// 守るのは人的ミスとアプリのバグまで（`docs/DESIGN.md`）。
pub fn directory(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// 控えの名前の頭。`EKANBAN_DATABASE` で別のファイルを使っていても、その名前に
/// 揃える。
fn stem(database_path: &Path) -> String {
    database_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| FALLBACK_STEM.to_string())
}

fn file_name(stem: &str, date: NaiveDate) -> String {
    format!("{stem}-{}.{EXTENSION}", date.format(DATE_FORMAT))
}

/// ファイル名から世代の日付を読む。命名規則に合わないものは `None`。
///
/// 並べ替えに mtime を使わないのは、ファイルをコピーすると当てにならなくなる
/// ため。名前から読めるほうが、テストでも人の目でも決定的になる。
fn date_of(file_name: &str, stem: &str) -> Option<NaiveDate> {
    let date = file_name
        .strip_prefix(stem)?
        .strip_prefix('-')?
        .strip_suffix(&format!(".{EXTENSION}"))?;
    NaiveDate::parse_from_str(date, DATE_FORMAT).ok()
}

/// その日ぶんの控えをまだ取っていなければ取り、古い世代を落とす。
///
/// 返すのは取った控えの場所。その日ぶんが既にあれば `None`。
pub fn run_daily(database_path: &Path, today: NaiveDate) -> Result<Option<PathBuf>, BackupError> {
    let directory = directory(database_path);
    let stem = stem(database_path);
    let destination = directory.join(file_name(&stem, today));

    if destination.exists() {
        // その日の最初の 1 回だけを残す。2 回目以降の起動で取り直すと、
        // 壊した状態で無事な控えを上書きすることになる。
        prune(&directory, &stem, GENERATIONS)?;
        return Ok(None);
    }

    std::fs::create_dir_all(&directory).map_err(|source| io_error(&directory, source))?;

    // 途中でアプリが終わっても中途半端なファイルが世代として残らないよう、
    // 書き終えてから名前を付ける。`VACUUM INTO` は出力先が既にあると失敗するので、
    // 前回の書きかけが残っていれば先に消す。
    let partial = destination.with_extension(PARTIAL_EXTENSION);
    if partial.exists() {
        std::fs::remove_file(&partial).map_err(|source| io_error(&partial, source))?;
    }
    Database::open(database_path)?.backup_to(&partial)?;
    std::fs::rename(&partial, &destination).map_err(|source| io_error(&destination, source))?;

    prune(&directory, &stem, GENERATIONS)?;
    Ok(Some(destination))
}

/// 日付の新しい順に `keep` 世代だけ残す。命名規則に合わないファイルは触らない。
fn prune(directory: &Path, stem: &str, keep: usize) -> Result<(), BackupError> {
    let mut found = generations(directory, stem)?;
    found.sort_by_key(|(date, _)| std::cmp::Reverse(*date));
    for (_, path) in found.into_iter().skip(keep) {
        std::fs::remove_file(&path).map_err(|source| io_error(&path, source))?;
    }
    Ok(())
}

/// ディレクトリにある世代を、日付とともに集める。まだ 1 つも無ければ空。
fn generations(directory: &Path, stem: &str) -> Result<Vec<(NaiveDate, PathBuf)>, BackupError> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(io_error(directory, source)),
    };

    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| io_error(directory, source))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if let Some(date) = date_of(name, stem) {
            found.push((date, entry.path()));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::{tempdir, TempDir};

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, DATE_FORMAT).expect("the test date parses")
    }

    /// 中身のあるデータベースと、その置き場所。
    fn seeded_database() -> (TempDir, PathBuf) {
        let directory = tempdir().expect("a temporary directory is available");
        let path = directory.path().join("ekanban.sqlite3");
        Database::open(&path).expect("a new database is created");
        (directory, path)
    }

    /// 控えのファイル名を日付順に並べたもの。
    fn stored_generations(database_path: &Path) -> Vec<String> {
        let mut found = generations(&directory(database_path), &stem(database_path))
            .expect("the backup directory can be read");
        found.sort_by_key(|(date, _)| *date);
        found
            .into_iter()
            .map(|(_, path)| {
                path.file_name()
                    .expect("a backup has a file name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn takes_one_backup_per_day() {
        let (_directory, path) = seeded_database();

        let first = run_daily(&path, date("2026-09-05")).expect("the first backup is taken");
        assert!(first.is_some(), "the first launch of the day takes a copy");

        let second = run_daily(&path, date("2026-09-05")).expect("the second launch is fine");
        assert!(
            second.is_none(),
            "a later launch on the same day leaves the morning's copy alone"
        );
        assert_eq!(stored_generations(&path), ["ekanban-2026-09-05.sqlite3"]);

        run_daily(&path, date("2026-09-06")).expect("the next day takes another copy");
        assert_eq!(
            stored_generations(&path),
            ["ekanban-2026-09-05.sqlite3", "ekanban-2026-09-06.sqlite3"]
        );
    }

    #[test]
    fn keeps_only_the_newest_generations() {
        let (_directory, path) = seeded_database();

        for day in 1..=(GENERATIONS + 3) {
            run_daily(&path, date(&format!("2026-09-{day:02}")))
                .expect("each day takes its own copy");
        }

        let kept = stored_generations(&path);
        assert_eq!(
            kept.len(),
            GENERATIONS,
            "the oldest generations are dropped"
        );
        assert_eq!(
            kept.first().map(String::as_str),
            Some("ekanban-2026-09-04.sqlite3"),
            "what remains starts after the dropped days: {kept:?}"
        );
        assert_eq!(
            kept.last().map(String::as_str),
            Some("ekanban-2026-09-10.sqlite3"),
            "and ends with the most recent one: {kept:?}"
        );
    }

    #[test]
    fn the_backup_opens_as_the_board_it_was_taken_from() {
        let (_directory, path) = seeded_database();

        let taken = {
            let mut database = Database::open(&path).expect("the database opens");
            let mut board = database.load_board().expect("the seeded board loads");
            let column_id = board.columns[0].id;
            board
                .add_card(column_id, "控えに入るカード", "")
                .expect("the column takes a card");
            database.save_board(&mut board).expect("the card is stored");
            board
        };

        let backup = run_daily(&path, date("2026-09-05"))
            .expect("the backup is taken")
            .expect("the first launch of the day takes a copy");

        let restored = Database::open(&backup)
            .expect("the backup opens as a database")
            .load_board()
            .expect("the backup holds the board");
        assert_eq!(
            restored, taken,
            "the copy holds the board as it was when it was taken"
        );
    }

    #[test]
    fn leaves_unrelated_files_in_the_backup_directory_alone() {
        let (_directory, path) = seeded_database();
        let backups = directory(&path);
        std::fs::create_dir_all(&backups).expect("the backup directory is created");

        let unrelated = [
            backups.join("メモ.txt"),
            backups.join("ekanban.sqlite3"),
            backups.join("ekanban-2026-09-05.sqlite3.old"),
            backups.join("ekanban-いつか.sqlite3"),
        ];
        for path in &unrelated {
            std::fs::write(path, b"keep me").expect("the unrelated file is written");
        }

        for day in 1..=(GENERATIONS + 3) {
            run_daily(&path, date(&format!("2026-10-{day:02}"))).expect("the backup is taken");
        }

        for path in &unrelated {
            assert!(
                path.exists(),
                "a file that is not a generation is left where it is: {path:?}"
            );
        }
        assert_eq!(stored_generations(&path).len(), GENERATIONS);
    }

    #[test]
    fn replaces_a_leftover_partial_file() {
        let (_directory, path) = seeded_database();
        let backups = directory(&path);
        std::fs::create_dir_all(&backups).expect("the backup directory is created");
        let partial = backups.join("ekanban-2026-09-05.part");
        std::fs::write(&partial, b"a launch that was cut short")
            .expect("the partial file is written");

        run_daily(&path, date("2026-09-05")).expect("a leftover partial does not block the backup");

        assert!(
            !partial.exists(),
            "the partial file is gone once the backup is named"
        );
        assert_eq!(stored_generations(&path), ["ekanban-2026-09-05.sqlite3"]);
    }

    #[test]
    fn names_the_backups_after_the_database_in_use() {
        let directory = tempdir().expect("a temporary directory is available");
        let path = directory.path().join("試し.sqlite3");
        Database::open(&path).expect("a new database is created");

        run_daily(&path, date("2026-09-05")).expect("the backup is taken");

        assert_eq!(stored_generations(&path), ["試し-2026-09-05.sqlite3"]);
    }
}
