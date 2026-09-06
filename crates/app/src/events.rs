//! Rust から webview への一方向のイベント（`docs/TAURI-MIGRATION.md` §3）。
//!
//! **3 つだけです。** 増やすほど「いつ何が届くか」を webview 側で数えることに
//! なるので、増やすときは §3 の表を先に直してください。
//!
//! 名前と積荷だけをここに置き、実際に投げるのはウィンドウが出る段階 3 です
//! （`tauri::Emitter`）。先に決めておくのは、コマンドの側が「これは自分の
//! 戻り値で返るのか、イベントで届くのか」を迷わないためです。

use ekanban_core::model::CardId;
use serde::Serialize;
use ts_rs::TS;

/// クイックキャプチャが書いたとき、ほかのウィンドウが盤面を変えたとき。
///
/// 受け取った webview はスナップショットを差し替えます。
pub const BOARD_CHANGED: &str = "board:changed";

/// メニューが押されたとき。webview の dispatcher に流します（§7）。
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
