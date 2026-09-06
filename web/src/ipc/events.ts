// Rust から届くイベントの名前（`crates/app/src/events.rs`）。
//
// **ここだけが文字列を持ちます。** 型と違って `ts-rs` は定数を書き出さないので、
// 名前は 2 か所にあります。散らさないために、受け取る側はこの定数を使います。

/** メニューが押された。積荷は `AppAction`。 */
export const APP_ACTION = "app:action";

/** 盤面が別のところで変わった。積荷は `Snapshot`。 */
export const BOARD_CHANGED = "board:changed";

/** クイックキャプチャの保存が終わった。積荷は `CaptureResult`。 */
export const CAPTURE_RESULT = "capture:result";
