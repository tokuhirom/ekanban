//! コマンドが返す形（`docs/TAURI-MIGRATION.md` §2）。
//!
//! **盤面を変えるコマンドは、変更後のスナップショットを丸ごと返します。** 差分は
//! 返しません。差分にすると、適用の順序と欠落を webview の側で面倒みることに
//! なります。大きさが問題になったら、そのときに測ってから、高頻度のものだけ
//! 差分に落とします（§13）。

use chrono::NaiveDate;
use ekanban_core::db::WindowBoundsState;
use ekanban_core::model::{due_status, Board, BoardId, BoardSummary, CardId, ColumnId, DueStatus};
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
    /// 期限を持つカードの、いま時点での状態。
    ///
    /// `due_status` は `model.rs` の純粋関数で、判定は Rust に残します（§5）。
    /// カードそのものには載せられません——`Card` はデータベースから来るもので、
    /// 「今日が何日か」を知らないからです。
    ///
    /// **日付をまたぐと古くなります。** 開きっぱなしで日が変わると、次に何か
    /// コマンドを呼ぶまで昨日の判定が出たままになります。`today` を一緒に返す
    /// のはそのためで、webview は手元の日付とずれたら読み直せます。
    pub due_statuses: Vec<CardDueStatus>,
    /// `due_statuses` を出したときの日付。
    pub today: NaiveDate,
}

/// カード 1 枚の期限の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CardDueStatus {
    pub card_id: CardId,
    pub status: DueStatus,
}

/// 盤面の全部のカード（アーカイブを含む）について、期限の状態を出す。
pub(crate) fn due_statuses_of(board: &Board, today: NaiveDate) -> Vec<CardDueStatus> {
    board
        .columns
        .iter()
        .flat_map(|column| column.cards.iter())
        .chain(board.archived_cards.iter())
        .filter(|card| card.due_date.is_some())
        .map(|card| CardDueStatus {
            card_id: card.id,
            status: due_status(card.due_date, today),
        })
        .collect()
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

/// 動いている OS。
///
/// **webview に自分で当てさせません。** キーの割り当ては OS ごとに違い
/// （§7、[ADR 0009]）、`secondary` が Cmd か Ctrl かを取り違えると割り当てが
/// 丸ごと効かなくなります。`navigator.userAgent` は webview が書き換えられる
/// 文字列で、実際 Playwright の Safari 模擬は Linux 上で `Macintosh` を名乗り
/// ます。ここは Rust がコンパイル時に知っていることなので、そちらから渡します。
///
/// [ADR 0009]: ../../../docs/adr/0009-per-platform-key-bindings.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
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
    /// 動いている OS。キーの割り当てを決めるのに使います。
    pub platform: Platform,
    pub filter: ekanban_core::db::FilterState,
    pub window_bounds: Option<WindowBoundsState>,
    pub theme: ThemePreference,
    pub sidebar_collapsed: bool,
    pub capture_target: Option<CaptureTarget>,
    /// 保存されている割り当て。登録できるかどうかは別の話（§9）。
    pub quick_capture_shortcut: Option<String>,
}
