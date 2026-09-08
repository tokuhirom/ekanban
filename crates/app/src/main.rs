// Windows のリリースビルドでコンソールウィンドウを出さない。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 殻（`shell`）を外した組み立て（`crates/web`、ADR 0035）に `run` はありません。
// 実行ファイルごと `required-features` で外すと、Tauri の CLI が実行ファイルの
// 名前をパッケージ名（`ekanban-app`）だと思い込み、出来上がった `ekanban` を
// 見つけられずに `tauri build` が rename で落ちます。だから外さず、中身を空に
// します。
#[cfg(feature = "shell")]
fn main() {
    ekanban_app::run();
}

#[cfg(not(feature = "shell"))]
fn main() {}
