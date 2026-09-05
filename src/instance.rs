//! 同じデータベースを 2 つのプロセスに開かせない。
//!
//! ekanban は起動を制限していなかったので、2 つ動かすと両方が同じ SQLite
//! ファイルを開いたまま、それぞれ別のボードをメモリに持つ。保存は差分の UPSERT
//! のあとに「自分の知らない行」を消すので、**あとから保存したほうが、もう片方で
//! 足したカードやカラムを消す**。どちらの画面も自分の状態を出し続けるため、
//! 消えたことにも気づけない。
//!
//! 起動時にロックファイルを 1 つ握り、握れなければ既に動いていると判断する。
//! 印を PID ファイルではなくファイルロックにしているのは、異常終了しても OS が
//! ロックを落とすため。PID ファイルだと残骸が次の起動を永久に塞ぐ。

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// ロックファイルに足す拡張子。
///
/// データベース本体とは別のファイルにする。SQLite のファイルそのものを掴むと、
/// SQLite 自身のロックと混ざる。
const LOCK_EXTENSION: &str = "lock";

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("{path}: {source}", path = path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("another ekanban is already using {}", .0.display())]
    AlreadyRunning(PathBuf),
}

/// 生きているあいだ、そのデータベースを自分のものにしておく印。
///
/// drop するとロックが外れるので、アプリが動いているあいだは落とさずに持ち続ける。
#[must_use = "dropping the lock lets a second ekanban open the same database"]
#[derive(Debug)]
pub struct InstanceLock {
    file: File,
}

impl Drop for InstanceLock {
    fn drop(&mut self) {
        // 閉じれば OS が落とすが、明示しておく。失敗しても打つ手はない。
        let _ = self.file.unlock();
    }
}

/// ロックファイルの場所。データベースの隣に、同じ名前で `.lock` を足したもの。
///
/// データベースのパスから作るので、`EKANBAN_DATABASE` で別のファイルを使えば
/// 別のロックになり、同時に起動できる。試しのデータで並べて動かす使い方
/// （`docs/DEVELOPMENT.md`）を壊さないため。
pub fn lock_path(database_path: &Path) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(".");
    path.push(LOCK_EXTENSION);
    PathBuf::from(path)
}

/// そのデータベースを自分のものにする。
///
/// 既に別のプロセスが握っていれば `AlreadyRunning` を返す。呼ぶのはデータベースを
/// 開くより前。開いてからでは `migrate` と `seed_if_empty` が 2 つのプロセスから
/// 走る。
pub fn acquire(database_path: &Path) -> Result<InstanceLock, InstanceError> {
    let path = lock_path(database_path);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|source| InstanceError::Io {
            path: path.clone(),
            source,
        })?;

    match file.try_lock() {
        Ok(()) => Ok(InstanceLock { file }),
        Err(std::fs::TryLockError::WouldBlock) => {
            Err(InstanceError::AlreadyRunning(database_path.to_path_buf()))
        }
        Err(std::fs::TryLockError::Error(source)) => Err(InstanceError::Io { path, source }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    #[test]
    fn refuses_a_second_lock_on_the_same_database() {
        let directory = tempdir().expect("a temporary directory is available");
        let database = directory.path().join("board.sqlite3");

        let _first = acquire(&database).expect("the first launch takes the database");
        let second = acquire(&database);

        assert!(
            matches!(second, Err(InstanceError::AlreadyRunning(_))),
            "a second launch on the same database is turned away: {second:?}"
        );
    }

    #[test]
    fn allows_a_second_lock_on_a_different_database() {
        let directory = tempdir().expect("a temporary directory is available");

        let _first =
            acquire(&directory.path().join("board.sqlite3")).expect("the first database is taken");
        let _second = acquire(&directory.path().join("試し.sqlite3"))
            .expect("a different database is a different instance");
    }

    #[test]
    fn frees_the_lock_when_the_first_one_goes_away() {
        let directory = tempdir().expect("a temporary directory is available");
        let database = directory.path().join("board.sqlite3");

        let first = acquire(&database).expect("the first launch takes the database");
        drop(first);

        let _second = acquire(&database).expect("the database is free once the first one is gone");
    }

    #[test]
    fn keeps_the_lock_file_next_to_the_database() {
        let directory = tempdir().expect("a temporary directory is available");
        let database = directory.path().join("board.sqlite3");

        let _lock = acquire(&database).expect("the database is taken");

        assert_eq!(
            lock_path(&database),
            directory.path().join("board.sqlite3.lock"),
            "the lock sits beside the database rather than inside it"
        );
        assert!(lock_path(&database).exists(), "and the file is created");
    }
}
