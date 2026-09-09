//! 盤面の置き場所（[ADR 0040]）。
//!
//! **SQLite です**（[`crate::db::Database`]）。呼ぶ側（`crates/app` の
//! `commands`）は [`Store`] だけを見て、SQL を知りません。
//!
//! **ここに盤面の判断はありません。** 採番も並べ替えも Undo もアーカイブも
//! `web/src/model/board.ts` にあり（[ADR 0039]）、ここにあるのは **`Board` を
//! どう書き出し、どう読み戻すか**と、受け取ったものが行として成り立っているか
//! の検めだけです。
//!
//! ブラウザで動くときの置き場所は TypeScript にあります
//! （`web/src/store/`、[ADR 0042]）——**アプリと同じコードがそのまま動く**ので、
//! 置き場所を Rust に 2 つ持つ理由がなくなりました。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
//! [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
//! [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::export;
use crate::model::{Board, BoardId, BoardSummary, ColumnId};

/// 置き場所が返す失敗。
///
/// SQLite そのものの失敗は、SQLite を積んでいるときだけあります。**理由を
/// 潰しません**——`crates/app` の `error.rs` がエラーコードごとに「次に何を
/// すればよいか」を出し分けており（ディスクが一杯、読み取り専用、壊れている）、
/// 1 つの文字列に丸めるとそれが全部消えます。
#[derive(Debug, Error)]
pub enum StoreError {
    /// 手元の版が古い。**書かずに断ります**（[ADR 0040]）。
    ///
    /// ボードの窓とキャプチャの窓が、それぞれ手元に盤面の写しを持つので、
    /// 古いほうをそのまま書くと相手の変更が黙って消えます。呼んだ側が読み直します。
    ///
    /// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
    #[error("the board changed elsewhere: expected rev {expected}, found {current}")]
    Conflict { expected: i64, current: i64 },
    #[cfg(feature = "sqlite")]
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("no board exists in the database")]
    NoBoard,
    #[error("cannot delete the last board")]
    LastBoard,
    #[error("a board name cannot be empty")]
    EmptyBoardName,
    #[error("invalid saved application state")]
    InvalidAppState,
    /// 置き場所を、開いたままもう 1 つ開いた。
    ///
    /// **待たずに落とします。** 置き場所がページに 1 つしかないところ
    /// （ブラウザ版）では、待っても相手は自分なので永遠に空きません。
    /// SQLite は接続を 2 つ持てるので、こちらは出てきません（[ADR 0042]）。
    ///
    /// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
    #[error("the store is already open")]
    AlreadyOpen,
    #[error("could not encode board export: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowBoundsState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilterState {
    pub search: String,
    pub tag_id: Option<i64>,
}

// 付随する表示の状態の鍵。SQLite では `app_state` テーブルの、JSON では
// `state` の鍵になります。**同じ綴りを使います**——置き場所を移し替える道を
// 塞がないためです。
pub(crate) const LAST_BOARD_STATE_KEY: &str = "last_board_id";
pub(crate) const NEXT_BOARD_STATE_KEY: &str = "next_board_id";
pub(crate) const WINDOW_BOUNDS_STATE_KEY: &str = "window_bounds";
pub(crate) const FILTER_SEARCH_STATE_KEY: &str = "filter_search";
pub(crate) const FILTER_TAG_STATE_KEY: &str = "filter_tag_id";
pub(crate) const THEME_PREFERENCE_STATE_KEY: &str = "theme_preference";
pub(crate) const SIDEBAR_COLLAPSED_STATE_KEY: &str = "sidebar_collapsed";
pub(crate) const DAY_BOUNDARY_HOUR_STATE_KEY: &str = "day_boundary_hour";
pub(crate) const QUICK_CAPTURE_SHORTCUT_STATE_KEY: &str = "quick_capture_shortcut";
pub(crate) const CAPTURE_BOARD_STATE_KEY: &str = "capture_board_id";
pub(crate) const CAPTURE_COLUMN_STATE_KEY: &str = "capture_column_id";

/// 日付が変わる時刻の既定（[ADR 0048]）。
///
/// 0 時ではありません。深夜に作業している最中に盤面が入れ替わらないように、
/// 午前 4 時から次の日を数えます。
///
/// [ADR 0048]: ../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md
pub const DEFAULT_DAY_BOUNDARY_HOUR: u8 = 4;

pub(crate) const BOARD_ID_NAMESPACE_SHIFT: u32 = 32;

/// 新しいボードが使う ID の名前空間の先頭。
///
/// ID は `(board_id, id)` の組ではなく主キー 1 本なので、ボードごとに区画を
/// 取ります。こうしておくと、ボードを切り替えたあとに別々の `Board` が
/// 採番しても衝突しません。
pub(crate) fn board_scoped_id(board_id: BoardId) -> i64 {
    board_id
        .checked_shl(BOARD_ID_NAMESPACE_SHIFT)
        .and_then(|id| id.checked_add(1))
        .expect("board ID namespace overflowed")
}

/// 置き場所から読んだ盤面 1 つぶん（[ADR 0039]）。
///
/// `Board` に**版**を添えたものです。版は保存の競合を見るためのもので、盤面の
/// 中身ではないので、`Board` の中には入れません。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocument {
    pub board: Board,
    pub rev: i64,
}

/// タグの色を自分で決める前の、かつての既定色（[ADR 0044]）。
///
/// **いまはどこも書き込みません。** 置いてあるものを「色を決めていない」に
/// 戻す移行だけが読みます（`db/mod.rs` の移行 13）。過去の値なので、色を選ぶ側
/// （`web/src/panel/tags.ts`）とは別に、置き場所の側に置いてあります。
///
/// [ADR 0044]: ../../../docs/adr/0044-tags-get-their-colour-automatically.md
pub(crate) const LEGACY_DEFAULT_TAG_COLOR: &str = "#94a3b8";

/// 保存されたカードの履歴の 1 件。
///
/// `model::CardEvent` と違って `id` を持ちます。**並び順が書き出しに出る**ので、
/// 置き場所が振った番号をそのまま運びます。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCardEvent {
    pub id: i64,
    pub card_id: i64,
    pub kind: String,
    pub from_column_id: Option<ColumnId>,
    pub to_column_id: Option<ColumnId>,
    pub at: i64,
}

/// 開いている置き場所。
///
/// `crates/app` の `commands` はこれだけを見ます。**中身の判断をここに書きません**
/// ——盤面の判断は `web/src/model/board.ts` にあり、ここが見るのは行として
/// 成り立っているかだけです（[ADR 0040]）。
///
/// 中身は SQLite だけです。ブラウザで動くときの置き場所は TypeScript に移り
/// ました（[ADR 0042]）——**ブラウザ版はアプリと同じコードがそのまま動く**ので、
/// 置き場所を Rust に 2 つ持つ理由がなくなりました。
///
/// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
/// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
pub enum Store {
    Sqlite(crate::db::Database),
}

impl Store {
    /// 最後に開いていたボード。無ければ先頭。
    pub fn load_board(&self) -> Result<Board, StoreError> {
        if let Some(board_id) = self.load_last_board_id()? {
            match self.load_board_by_id(board_id) {
                Ok(board) => return Ok(board),
                Err(StoreError::NoBoard) => {}
                Err(error) => return Err(error),
            }
        }
        let first = self
            .load_boards()?
            .first()
            .map(|board| board.id)
            .ok_or(StoreError::NoBoard)?;
        self.load_board_by_id(first)
    }

    /// ボード一覧。名前と並びだけです（[ADR 0039]）。
    ///
    /// 並びは `id` の昇順です。サイドバーの並びがこれで決まります。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    pub fn load_boards(&self) -> Result<Vec<BoardSummary>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_boards(),
        }
    }

    pub fn load_board_by_id(&self, id: BoardId) -> Result<Board, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_board_by_id(id),
        }
    }

    /// 盤面を書き込む。`pending_events` は取り込まれて空になる。
    /// 盤面を書く。**版は確かめません**（`db::Database::save_board` と同じ）。
    pub fn save_board(&mut self, board: &mut Board) -> Result<(), StoreError> {
        self.save_board_with_rev(board, None).map(|_| ())
    }

    /// 版を確かめてから書く（[ADR 0040]）。書けたら次の版を返す。
    ///
    /// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
    pub fn save_board_at(&mut self, board: &mut Board, expected: i64) -> Result<i64, StoreError> {
        self.save_board_with_rev(board, Some(expected))
    }

    fn save_board_with_rev(
        &mut self,
        board: &mut Board,
        expected: Option<i64>,
    ) -> Result<i64, StoreError> {
        match self {
            Store::Sqlite(database) => match expected {
                Some(expected) => database.save_board_at(board, expected),
                None => database.save_board(board).map(|()| 0),
            },
        }
    }

    /// 1 つのボードの版だけを読む。作ったばかりのボードを返すときに使います。
    pub fn load_board_rev(&self, id: BoardId) -> Result<i64, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_board_rev(id),
        }
    }

    /// 全部のボードを、webview が持つ形で読む（[ADR 0039]）。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    pub fn load_documents(&self) -> Result<Vec<StoredDocument>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_documents(),
        }
    }

    pub fn create_board(&mut self, name: &str) -> Result<Board, StoreError> {
        match self {
            Store::Sqlite(database) => database.create_board(name),
        }
    }

    /// ボードを消す。**最後の 1 つは消せません。**
    pub fn delete_board(&mut self, board_id: BoardId) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.delete_board(board_id),
        }
    }

    /// 書き出す JSON。組み立ては `export` にあり、ここは履歴を渡すだけ。
    pub fn export_board_json(&self, board: &Board) -> Result<String, StoreError> {
        let events = self.card_events(board.id)?;
        export::render_board_json(board, &events)
    }

    fn card_events(&self, board_id: BoardId) -> Result<Vec<StoredCardEvent>, StoreError> {
        match self {
            Store::Sqlite(database) => database.card_events(board_id),
        }
    }

    /// ボードが 1 つも無ければ、最初の盤面を蒔く。
    pub fn seed_if_empty(&mut self) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => {
                let _ = database;
                Ok(())
            }
        }
    }

    // ------------------------------------------------------------ 付随する状態

    pub fn load_last_board_id(&self) -> Result<Option<BoardId>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_last_board_id(),
        }
    }

    pub fn set_last_board_id(&mut self, board_id: BoardId) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_last_board_id(board_id),
        }
    }

    pub fn load_window_bounds(&self) -> Result<Option<WindowBoundsState>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_window_bounds(),
        }
    }

    pub fn set_window_bounds(&mut self, bounds: WindowBoundsState) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_window_bounds(bounds),
        }
    }

    pub fn load_filter_state(&self) -> Result<FilterState, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_filter_state(),
        }
    }

    pub fn set_filter_state(&mut self, state: &FilterState) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_filter_state(state),
        }
    }

    pub fn load_theme_preference(&self) -> Result<Option<String>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_theme_preference(),
        }
    }

    pub fn set_theme_preference(&mut self, preference: &str) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_theme_preference(preference),
        }
    }

    pub fn load_sidebar_collapsed(&self) -> Result<bool, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_sidebar_collapsed(),
        }
    }

    /// 日付が変わる時刻（0〜23）。置かれていなければ既定の 4。
    pub fn load_day_boundary_hour(&self) -> Result<u8, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_day_boundary_hour(),
        }
    }

    /// 日付が変わる時刻を覚える。0〜23 の外は断ります。
    pub fn set_day_boundary_hour(&mut self, hour: u8) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_day_boundary_hour(hour),
        }
    }

    pub fn set_sidebar_collapsed(&mut self, collapsed: bool) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_sidebar_collapsed(collapsed),
        }
    }

    pub fn load_quick_capture_shortcut(&self) -> Result<Option<String>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_quick_capture_shortcut(),
        }
    }

    pub fn set_quick_capture_shortcut(&mut self, shortcut: Option<&str>) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_quick_capture_shortcut(shortcut),
        }
    }

    pub fn load_capture_target(&self) -> Result<Option<(BoardId, ColumnId)>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_capture_target(),
        }
    }

    pub fn set_capture_target(
        &mut self,
        target: Option<(BoardId, ColumnId)>,
    ) -> Result<(), StoreError> {
        match self {
            Store::Sqlite(database) => database.set_capture_target(target),
        }
    }

    pub fn load_column_name(
        &self,
        board_id: BoardId,
        column_id: ColumnId,
    ) -> Result<Option<String>, StoreError> {
        match self {
            Store::Sqlite(database) => database.load_column_name(board_id, column_id),
        }
    }
}
