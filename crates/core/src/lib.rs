//! ekanban の中核。盤面のモデル、SQLite、控え、置き場所、多重起動の防止。
//!
//! ここには UI が入らない。`tauri` に依存しないので、テストは GUI の
//! ランタイム無しで走り、Tauri のアプリと開発用のハーネスが同じコードを使える
//! （`docs/DESIGN.md`「層の分け方」）。

pub mod backup;
pub mod db;
pub mod diagnostics;
pub mod export;
pub mod instance;
pub mod model;
pub mod paths;

use std::path::PathBuf;

/// JavaScript が誤差なく扱える整数の上限（2^53 - 1）。
///
/// ID も時刻も JSON の数値として webview に渡ります（`docs/DESIGN.md`
/// 「境界を越える値」）。ここを超えた値は JavaScript 側で丸められ、**落ちずに別のものを指します**。
/// 上限に当たっていないことは `db` のテストが見ています。
pub const MAX_SAFE_JS_INTEGER: i64 = 9_007_199_254_740_991;

/// ウィンドウタイトルやバンドルに使うアプリ名。`script/bundle-mac` の `APP_NAME` と揃える。
pub const APP_NAME: &str = "Ekanban";

/// デスクトップ環境がウィンドウをアプリに結びつけるための識別子。
/// `script/bundle-mac` の `BUNDLE_ID` と揃える。
pub const APP_ID: &str = "dev.tokuhirom.ekanban";

/// ウィンドウが X11 に載せる `WM_CLASS`（instance のほう）。
///
/// **これは `APP_ID` ではありません。** Tauri（tao）は実行ファイルの名前から
/// 作るので、`ekanban` という実行ファイルは `("ekanban", "Ekanban")` と名乗ります。
/// デスクトップエントリの `StartupWMClass` はこちらと揃える必要があります——
/// 食い違うと、アプリ一覧からは起動できるのにタスクバーのアイコンと名前が
/// 汎用のものに戻ります（[ADR 0013]）。
///
/// instance のほう（小文字）を採るのは、**Tauri のバンドラが `.deb` と
/// `.AppImage` に入れるエントリもそう書くから**です。入れ方が 2 通りあるのに
/// 印が食い違うと、片方だけ結びつきません。
///
/// [ADR 0013]: ../../docs/adr/0013-linux-desktop-integration.md
pub const WM_CLASS: &str = "ekanban";

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

    use ts_rs::TS;

    /// 生成した TypeScript の型に `bigint` が出てこないこと。
    ///
    /// ts-rs は `i64` を既定で `bigint` に落とす。ところが値は `serde_json` が
    /// JSON の数値として書き、`JSON.parse` は `number` を返す。型定義だけ
    /// `bigint` になっていると、**実行時の値と型が食い違ったまま通る**——
    /// `Map<bigint, Card>` の引きが黙って外れる類の壊れ方をする。
    ///
    /// なので `.cargo/config.toml` で `TS_RS_LARGE_INT = "number"` にしてある。
    /// その設定が外れたことをここで捕まえる。番号が `number` に収まるかどうか
    /// ——2^53 に届かないかどうか——は `db` のテストが見ている。
    #[test]
    fn the_generated_types_never_say_bigint() {
        let config = ts_rs::Config::from_env();
        let declarations = [
            ("Board", model::Board::inline(&config)),
            ("BoardSummary", model::BoardSummary::inline(&config)),
            ("Card", model::Card::inline(&config)),
            ("ChecklistItem", model::ChecklistItem::inline(&config)),
            (
                "ChecklistItemDraft",
                model::ChecklistItemDraft::inline(&config),
            ),
            ("Column", model::Column::inline(&config)),
            ("DueCounts", model::DueCounts::inline(&config)),
            ("DueStatus", model::DueStatus::inline(&config)),
            ("FilterState", db::FilterState::inline(&config)),
            ("Tag", model::Tag::inline(&config)),
            ("WindowBoundsState", db::WindowBoundsState::inline(&config)),
        ];
        for (name, declaration) in declarations {
            assert!(
                !declaration.contains("bigint"),
                "{name} に bigint が残っている。`#[ts(type = \"number\")]` を付けること:\n{declaration}"
            );
        }
    }

    /// Linux のデスクトップエントリと、ウィンドウが名乗る名前の対応（#50）。
    ///
    /// `StartupWMClass` はデスクトップ環境が「このウィンドウはこのエントリのもの」
    /// と判断するための印で、ウィンドウの `WM_CLASS` と同じでなければ結びつかない。
    /// 食い違うと、アプリ一覧からは起動できるのにタスクバーのアイコンと名前が
    /// 汎用のものに戻る。ファイル名は `<APP_ID>.desktop` である必要があり、
    /// こちらは `include_str!` のパスがコンパイル時に見ている。
    #[test]
    fn the_linux_desktop_entry_points_at_the_app_id() {
        // 見たいのは中身であって行末ではない。`.gitattributes` が LF に固定して
        // いるが、それが外れたときにここが落ちても理由が読み取れないので、
        // 読んだ時点でそろえる。
        let entry =
            include_str!("../../../assets/dev.tokuhirom.ekanban.desktop").replace("\r\n", "\n");
        assert!(
            entry.contains(&format!("\nStartupWMClass={WM_CLASS}\n")),
            "StartupWMClass must match the WM_CLASS the window announces"
        );
        assert!(
            entry.contains(&format!("\nIcon={APP_ID}\n")),
            "the icon name must match the files under assets/icons"
        );
        assert!(entry.contains(&format!("\nName={APP_NAME}\n")));
    }
}
