//! ekanban のコマンド層（`docs/TAURI-MIGRATION.md` §2・§3）。
//!
//! 盤面は Rust が持ち（[ADR 0018]）、webview はその投影だけを描きます。盤面を
//! 変えるコマンドは、モデルへの適用と SQLite への保存を続けて行い、**両方成功
//! してから**スナップショットを返します。
//!
//! `tauri` にはまだ依存していません。`#[tauri::command]` の包み、ウィンドウ、
//! メニュー、グローバルな割り当ては、画面が出る段階 3 で足します。§10 の開発用
//! ハーネスがこの同じ関数を HTTP に出すので、**中身が Tauri を知らないことは
//! 設計そのもの**です。
//!
//! [ADR 0018]: ../../../docs/adr/0018-rust-owns-the-board-state.md

pub mod commands;
pub mod error;
pub mod events;
pub mod snapshot;
pub mod state;

pub use error::{AppError, ErrorKind, Field};
pub use snapshot::{CaptureTarget, Snapshot, StartupState, ThemePreference};
pub use state::AppState;

#[cfg(test)]
mod tests {
    use ts_rs::TS;

    /// 生成した TypeScript の型に `bigint` が出てこないこと。
    ///
    /// 理由は `ekanban_core` の同名のテストと同じ——値は JSON の数値として渡り、
    /// `JSON.parse` は `number` を返すので、型だけ `bigint` だと実行時と食い違う。
    /// `.cargo/config.toml` の `TS_RS_LARGE_INT` が外れたらここが落ちる。
    #[test]
    fn the_generated_types_never_say_bigint() {
        let config = ts_rs::Config::from_env();
        let declarations = [
            ("AppError", crate::AppError::inline(&config)),
            (
                "CaptureResult",
                crate::events::CaptureResult::inline(&config),
            ),
            ("CaptureTarget", crate::CaptureTarget::inline(&config)),
            (
                "ExportFormat",
                crate::commands::ExportFormat::inline(&config),
            ),
            ("Snapshot", crate::Snapshot::inline(&config)),
            ("StartupState", crate::StartupState::inline(&config)),
        ];
        for (name, declaration) in declarations {
            assert!(
                !declaration.contains("bigint"),
                "{name} に bigint が残っている:\n{declaration}"
            );
        }
    }
}
