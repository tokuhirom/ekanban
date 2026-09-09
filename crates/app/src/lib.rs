//! ekanban のコマンド層（`docs/DESIGN.md`「状態の持ち主」「コマンドとイベント」）。
//!
//! **盤面は webview が持ちます**（[ADR 0039]）。ここに残るのは、このアプリを
//! ウェブアプリとして書いたとしてサーバ側に置くだろうもの——置き場所の読み書き
//! （[ADR 0040]）、覚えておく設定、OS に頼むこと——だけです。
//!
//! `commands` は `tauri` を知りません。`ipc` の `#[tauri::command]` は、その関数を
//! 呼ぶだけの包みです。**判断を包みの側に置かない**ので、窓を開けずに全部の
//! コマンドを試せます（`crates/app/tests/commands.rs`）。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
//! [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md

pub mod commands;
pub mod error;
pub mod events;
pub mod menu;
pub mod shortcut;
pub mod snapshot;
pub mod state;

// Tauri の殻。**ここに並ぶものだけが Tauri を知っています。**
pub mod capture;
pub mod ipc;
pub mod run;
pub mod window;

pub use error::{AppError, ErrorKind, Field};
pub use menu::{Action, AppAction, WindowAction};
pub use run::run;
pub use snapshot::{CaptureTarget, Platform, QuickCaptureStatus, StartupState, ThemePreference};
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
                "BoardDocument",
                crate::commands::BoardDocument::inline(&config),
            ),
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
