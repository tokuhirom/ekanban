//! コマンドが返す形（`docs/DESIGN.md`「コマンドとイベント」）。
//!
//! **盤面を変えるコマンドは、変更後のスナップショットを丸ごと返します。** 差分は
//! 返しません。差分にすると、適用の順序と欠落を webview の側で面倒みることに
//! なります。大きさが問題になったら、そのときに測ってから、高頻度のものだけ
//! 差分に落とします。

use ekanban_core::model::{Board, BoardId, BoardSummary, ColumnId};
use ekanban_core::store::WindowBoundsState;
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
    /// クイックキャプチャの入れ先が、このボードのどのカラムか。
    ///
    /// 設定が無ければ既定（先頭カラム）、別のボードを指していれば `None` です。
    /// **印を出すのは画面ですが、どこが入れ先かを決めるのは Rust**——同じ既定を
    /// TypeScript にもう 1 つ持たせません。
    pub capture_column: Option<ColumnId>,
    /// ウィンドウのタイトル。webview がそのまま `set_window_title` に渡します。
    ///
    /// 組み立てを TypeScript に持たせません。ボード名の扱い（空白だけの名前は
    /// アプリ名だけにする）は表示の判断なので、盤面の判断と同じところに置きます。
    pub window_title: String,
}

/// ウィンドウのタイトル。
pub(crate) fn window_title(board_name: &str) -> String {
    let board_name = board_name.trim();
    if board_name.is_empty() {
        ekanban_core::APP_NAME.to_string()
    } else {
        format!("{board_name} — {}", ekanban_core::APP_NAME)
    }
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
/// （`docs/DESIGN.md`「メニューとキー割り当て」、[ADR 0009]）、`secondary` が Cmd か Ctrl かを取り違えると割り当てが
/// 丸ごと効かなくなります。`navigator.userAgent` は webview が書き換えられる
/// 文字列で、実際 Playwright の Safari 模擬は Linux 上で `Macintosh` を名乗り
/// ます。ここは Rust がコンパイル時に知っていることなので、そちらから渡します。
///
/// **例外はブラウザだけで動く組み立てです**（`crates/web`、[ADR 0035]）。
/// `wasm32-unknown-unknown` は macOS でも Linux でもないので、そこだけは
/// ページが名乗ったものを受け取ります。配るアプリの経路は変わりません。
///
/// [ADR 0009]: ../../../docs/adr/0009-per-platform-key-bindings.md
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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

/// 割り当てのダイアログが開くときに読むもの。
///
/// 型がここにあるのは、**殻を外した組み立てでも返す必要がある**ためです
/// （`crates/web`、[ADR 0035]）。中身を埋めるのは殻を持っている側
/// （`capture::status`）で、ブラウザでは「使えない理由」だけが入ります。
///
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct QuickCaptureStatus {
    /// この環境でグローバルホットキーを使えないなら、その理由。使えるなら `null`。
    pub unavailable: Option<String>,
    /// 保存されているのに登録できていない理由。効いているなら `null`。
    pub failure: Option<String>,
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
    /// 起動でもイベントでも同じ 1 本の経路で差し替えられます（`docs/DESIGN.md`「画面の作り」）。
    pub snapshot: Snapshot,
    /// 動いている OS。キーの割り当てを決めるのに使います。
    pub platform: Platform,
    pub filter: ekanban_core::store::FilterState,
    pub window_bounds: Option<WindowBoundsState>,
    pub theme: ThemePreference,
    pub sidebar_collapsed: bool,
    pub capture_target: Option<CaptureTarget>,
    /// 保存されている割り当て。登録できるかどうかは別の話（`docs/DESIGN.md`「クイックキャプチャ」）。
    pub quick_capture_shortcut: Option<String>,
    /// 動いているアプリの版（#147）。
    ///
    /// **出どころは `CARGO_PKG_VERSION` の 1 つだけ**です。webview 側の
    /// `package.json` の版はアプリの版ではないので、そちらは見ません。
    pub version: String,
    /// いま開いているデータベースのフルパス（#147）。
    ///
    /// `EKANBAN_DATABASE` で差し替えていればそれが入ります。「場所を開く」で
    /// 開けるだけでは、どのファイルを見ているのかを文字で読めませんでした。
    pub database_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_title_shows_the_board_and_the_app() {
        let title = window_title("個人 Kanban");
        assert!(title.contains("個人 Kanban"));
        assert!(title.contains(ekanban_core::APP_NAME));
    }

    /// 名前が空白だけのボードでも、タイトルが区切り記号だけにならないこと。
    #[test]
    fn window_title_falls_back_to_the_app_name_for_a_blank_board() {
        assert_eq!(window_title("   "), ekanban_core::APP_NAME);
    }
}
