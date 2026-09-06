//! ekanban の中核。盤面のモデル、SQLite、控え、置き場所、多重起動の防止。
//!
//! ここには UI が入らない。gpui にも tauri にも依存しないので、テストは GUI の
//! ランタイム無しで走り、Tauri のアプリと開発用のハーネスが同じコードを使える
//! （`docs/TAURI-MIGRATION.md` §1）。

pub mod backup;
pub mod db;
pub mod diagnostics;
pub mod instance;
pub mod model;
pub mod paths;

use std::path::PathBuf;

/// ウィンドウタイトルやバンドルに使うアプリ名。`script/bundle-mac` の `APP_NAME` と揃える。
pub const APP_NAME: &str = "Ekanban";

/// デスクトップ環境がウィンドウをアプリに結びつけるための識別子。
/// `script/bundle-mac` の `BUNDLE_ID` と揃える。
pub const APP_ID: &str = "dev.tokuhirom.ekanban";

/// データベースの置き場所を決める。
///
/// GUI から起動するとカレントディレクトリが当てにならないため、相対パスは使わない。
/// `EKANBAN_DATABASE` が指定されていればそれを、なければ OS ごとの標準の場所を使う。
pub fn database_path() -> PathBuf {
    if let Some(path) = std::env::var_os("EKANBAN_DATABASE") {
        return PathBuf::from(path);
    }
    paths::data_dir().join("ekanban.sqlite3")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linux のデスクトップエントリと `APP_ID` の対応（#50）。
    ///
    /// `StartupWMClass` はデスクトップ環境が「このウィンドウはこのエントリのもの」
    /// と判断するための印で、ウィンドウに渡すアプリ識別子と同じでなければ結びつかない。
    /// 食い違うと、アプリ一覧からは起動できるのにタスクバーのアイコンと名前が
    /// 汎用のものに戻る。ファイル名も `<APP_ID>.desktop` である必要があり、
    /// こちらは `include_str!` のパスがコンパイル時に見ている。
    #[test]
    fn the_linux_desktop_entry_points_at_the_app_id() {
        // 見たいのは中身であって行末ではない。`.gitattributes` が LF に固定して
        // いるが、それが外れたときにここが落ちても理由が読み取れないので、
        // 読んだ時点でそろえる。
        let entry =
            include_str!("../../../assets/dev.tokuhirom.ekanban.desktop").replace("\r\n", "\n");
        assert!(
            entry.contains(&format!("\nStartupWMClass={APP_ID}\n")),
            "StartupWMClass must match the app id given to the window"
        );
        assert!(
            entry.contains(&format!("\nIcon={APP_ID}\n")),
            "the icon name must match the files under assets/icons"
        );
        assert!(entry.contains(&format!("\nName={APP_NAME}\n")));
    }
}
