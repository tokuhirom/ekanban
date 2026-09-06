//! Rust から webview への一方向のイベント（`docs/DESIGN.md`「コマンドとイベント」）。
//!
//! **3 つだけです。** 増やすほど「いつ何が届くか」を webview 側で数えることに
//! なるので、増やすときは `docs/DESIGN.md` の規則を先に直してください。
//!
//! 名前と積荷だけがここにあり、実際に投げるのは `run.rs` と `ipc.rs`
//! （`tauri::Emitter`）です。1 か所に集めておくのは、コマンドの側が「これは
//! 自分の戻り値で返るのか、イベントで届くのか」を迷わないためです。

use ekanban_core::model::CardId;
use serde::Serialize;
use ts_rs::TS;

/// クイックキャプチャが書いたとき、ほかのウィンドウが盤面を変えたとき。
///
/// 受け取った webview はスナップショットを差し替えます。
pub const BOARD_CHANGED: &str = "board:changed";

/// メニューが押されたとき。webview の dispatcher に流します（`docs/DESIGN.md`「メニューとキー割り当て」）。
pub const APP_ACTION: &str = "app:action";

/// キャプチャの保存が終わったとき。ウィンドウを閉じる / 失敗を出す。
pub const CAPTURE_RESULT: &str = "capture:result";

/// `capture:result` の積荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "status"
)]
#[ts(export)]
pub enum CaptureResult {
    /// 書けた。ウィンドウを閉じる。
    Saved { card_id: CardId },
    /// 書けなかった。ウィンドウは開いたまま、理由を出す。
    Failed { detail: String },
}
