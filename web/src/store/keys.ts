// 覚えておく設定の鍵（`app_state`）。
//
// **綴りは SQLite の側と同じ**にします（`crates/core/src/store.rs`）。ブラウザ
// に置いた文字列を、あとから SQLite に持ち込む道を塞がないためです。

export const LAST_BOARD = "last_board_id";
export const NEXT_BOARD = "next_board_id";
export const WINDOW_BOUNDS = "window_bounds";
export const FILTER_SEARCH = "filter_search";
export const FILTER_TAG = "filter_tag_id";
export const THEME_PREFERENCE = "theme_preference";
export const SIDEBAR_COLLAPSED = "sidebar_collapsed";
export const DAY_BOUNDARY_HOUR = "day_boundary_hour";
export const QUICK_CAPTURE_SHORTCUT = "quick_capture_shortcut";
export const CAPTURE_BOARD = "capture_board_id";
export const CAPTURE_COLUMN = "capture_column_id";

/// 新しいボードが使う ID の名前空間の先頭。
///
/// ID は `(boardId, id)` の組ではなく主キー 1 本なので、ボードごとに区画を
/// 取ります。こうしておくと、別々のボードが手元で採番しても衝突しません
/// （`store::board_scoped_id`）。
export function boardScopedId(boardId: number): number {
  return boardId * 2 ** 32 + 1;
}
