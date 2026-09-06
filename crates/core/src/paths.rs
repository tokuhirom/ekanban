//! アプリのデータとログの置き場所を OS ごとに決める。
//!
//! `.app` などの GUI 起動ではカレントディレクトリが当てにならないため、
//! 相対パスは使わず、必ずユーザーのホーム以下の絶対パスを組み立てる。

use std::path::PathBuf;

const APP_NAME: &str = "ekanban";

/// ホームディレクトリ。取得できない場合のみ `None`。
fn home_dir() -> Option<PathBuf> {
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 環境変数で指定されたディレクトリを、絶対パスのときだけ採用する。
///
/// XDG の仕様では相対パスは無視することになっている。
fn env_dir(key: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(key).filter(|value| !value.is_empty())?);
    path.is_absolute().then_some(path)
}

/// データベースなど、消えると困るファイルを置くディレクトリ。
///
/// - macOS: `~/Library/Application Support/ekanban`
/// - Windows: `%APPDATA%\ekanban`
/// - その他 (Linux/BSD): `$XDG_DATA_HOME/ekanban` または `~/.local/share/ekanban`
pub fn data_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            return home.join("Library/Application Support").join(APP_NAME);
        }
    } else if cfg!(windows) {
        if let Some(dir) = env_dir("APPDATA") {
            return dir.join(APP_NAME);
        }
    } else {
        if let Some(dir) = env_dir("XDG_DATA_HOME") {
            return dir.join(APP_NAME);
        }
        if let Some(home) = home_dir() {
            return home.join(".local/share").join(APP_NAME);
        }
    }

    fallback_dir()
}

/// 起動失敗の記録を残すログファイル。
///
/// - macOS: `~/Library/Logs/ekanban.log`
/// - Windows: `%LOCALAPPDATA%\ekanban\ekanban.log`
/// - その他 (Linux/BSD): `$XDG_STATE_HOME/ekanban/ekanban.log` または
///   `~/.local/state/ekanban/ekanban.log`
pub fn log_file() -> PathBuf {
    let name = format!("{APP_NAME}.log");

    if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            return home.join("Library/Logs").join(name);
        }
    } else if cfg!(windows) {
        if let Some(dir) = env_dir("LOCALAPPDATA") {
            return dir.join(APP_NAME).join(name);
        }
    } else {
        if let Some(dir) = env_dir("XDG_STATE_HOME") {
            return dir.join(APP_NAME).join(name);
        }
        if let Some(home) = home_dir() {
            return home.join(".local/state").join(APP_NAME).join(name);
        }
    }

    fallback_dir().join(name)
}

/// ホームも環境変数も取れないときの最後の手段。
///
/// カレントディレクトリより一時ディレクトリのほうが書ける見込みが高い。
fn fallback_dir() -> PathBuf {
    std::env::temp_dir().join(APP_NAME)
}

#[cfg(test)]
mod tests {
    use super::{data_dir, log_file};

    #[test]
    fn paths_are_absolute_and_namespaced() {
        for path in [data_dir(), log_file()] {
            assert!(path.is_absolute(), "{} が絶対パスではない", path.display());
            assert!(
                path.to_string_lossy().contains("ekanban"),
                "{} がアプリ名を含まない",
                path.display()
            );
        }
    }

    #[test]
    fn log_file_is_a_file_not_a_directory() {
        let path = log_file();
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("log"));
        assert!(path.parent().is_some_and(|parent| parent.is_absolute()));
    }
}
