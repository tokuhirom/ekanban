//! コマンドが返す形（`docs/TAURI-MIGRATION.md` §2）。
//!
//! **盤面を変えるコマンドは、変更後のスナップショットを丸ごと返します。** 差分は
//! 返しません。差分にすると、適用の順序と欠落を webview の側で面倒みることに
//! なります。大きさが問題になったら、そのときに測ってから、高頻度のものだけ
//! 差分に落とします（§13）。

use ekanban_core::db::WindowBoundsState;
use ekanban_core::model::{Board, BoardId, BoardSummary, ColumnId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 盤面を変えるコマンドが返すもの。
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snapshot {
    pub board: Board,
    /// 期限の件数つきのボード一覧。サイドバーがこれを描く。
    pub boards: Vec<BoardSummary>,
    pub can_undo: bool,
    pub can_redo: bool,
}

/// テーマの設定。`app_state` に文字列で入っている。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePreference {
    /// 保存されている文字列から読む。読めない値は既定に戻す。起動を妨げない。
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::System,
        }
    }

    /// `app_state` に入れる文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// クイックキャプチャが書き込む先。アプリ全体で 1 つ（`docs/DESIGN.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CaptureTarget {
    pub board_id: BoardId,
    pub column_id: ColumnId,
    /// 表示用に覚えておく名前。別のボードのカラムでも「どこに入るか」を出せるように。
    pub board_name: String,
    pub column_name: String,
}

/// 起動のときと、ウィンドウを開き直すときに読むもの。
///
/// メモリ上の値を抱えて使い回しません。ウィンドウを閉じている間もクイック
/// キャプチャはカードを足せるので、閉じたときの値で開き直すと古い盤面が出ます
/// （`docs/DESIGN.md`）。
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StartupState {
    /// 盤面そのもの。`board:changed` で届くのと同じ形なので、webview は
    /// 起動でもイベントでも同じ 1 本の経路で差し替えられます（§4）。
    pub snapshot: Snapshot,
    pub filter: ekanban_core::db::FilterState,
    pub window_bounds: Option<WindowBoundsState>,
    pub theme: ThemePreference,
    pub sidebar_collapsed: bool,
    pub capture_target: Option<CaptureTarget>,
    /// 保存されている割り当て。登録できるかどうかは別の話（§9）。
    pub quick_capture_shortcut: Option<String>,
}
