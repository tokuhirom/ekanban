//! コマンドが返す形（`docs/DESIGN.md`「コマンドとイベント」）。
//!
//! **盤面はここに出てきません。** 持っているのは webview なので（[ADR 0039]）、
//! ここに並ぶのは起動のときに一度だけ渡す「どこから始めるか」と、置き場所に
//! 覚えてある表示の設定です。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

use ekanban_core::model::{BoardId, ColumnId};
use ekanban_core::store::WindowBoundsState;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
/// **例外はブラウザ版です**（[ADR 0042]）。あちらには訊く相手の Rust が
/// いないので、入口（`web/src/ipc/local.ts`）が 1 度だけ見て、以降は同じ
/// ものを配ります。配るアプリの経路は変わりません。
///
/// [ADR 0009]: ../../../docs/adr/0009-per-platform-key-bindings.md
/// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
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
/// 型がここにあるのは、**ブラウザ版も同じものを返す必要がある**ためです
/// （[ADR 0042]）。中身を埋めるのは殻を持っている側（`capture::status`）で、
/// ブラウザでは「使えない理由」だけが入ります。
///
/// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
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
///
/// **名前は入っていません。** どのボードのどのカラムかを覚えているのが
/// 置き場所の仕事で、それを「〇〇ボード / △△カラム」と読ませるのは画面の
/// 仕事です（[ADR 0039]）。盤面は webview が全部持っているので、引くのに
/// 往復が要りません。指している先が消えていたときに既定へ落とすのも、
/// そちらで済みます（[ADR 0028]）。
///
/// [ADR 0028]: ../../../docs/adr/0028-a-single-default-quick-capture-target.md
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CaptureTarget {
    pub board_id: BoardId,
    pub column_id: ColumnId,
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
    /// 最初に開くボード。**盤面そのものは渡しません**——webview が
    /// `load_documents` で全部読みます（[ADR 0039]）。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    pub open_board_id: BoardId,
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
