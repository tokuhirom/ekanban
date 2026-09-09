//! 置き場所をどこに開くか（[ADR 0039]）。
//!
//! **盤面は持ちません。** 持っているのは webview で、こちらは読み書きを頼まれる
//! 側です。ここに残っているのは「どのファイルか」だけです。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

use std::path::{Path, PathBuf};

use ekanban_core::store::{Store, StoreError};

/// 置き場所への入口。
pub struct AppState {
    source: Source,
}

/// 盤面をどこに置くか。
///
/// **開くたびに開き直します。** クイックキャプチャの窓が閉じている間にも書ける
/// ので、メモリ上の写しを抱え続けると、そちらの変更が見えません。
///
/// 置き場所が SQLite だけになったのは、ブラウザで動くときの置き場所が
/// TypeScript に移ったからです（[ADR 0042]）。
///
/// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md
#[derive(Clone)]
pub enum Source {
    Sqlite(PathBuf),
}

impl Source {
    /// 置き場所を開く。
    pub fn open(&self) -> Result<Store, StoreError> {
        match self {
            Self::Sqlite(path) => Ok(Store::Sqlite(ekanban_core::db::Database::open(path)?)),
        }
    }

    /// 盤面がどこにあるか。画面（「ekanban について」）に出す文言。
    pub fn place(&self) -> String {
        match self {
            Self::Sqlite(path) => path.to_string_lossy().into_owned(),
        }
    }

    /// SQLite のファイルの場所。
    pub fn path(&self) -> &Path {
        match self {
            Self::Sqlite(path) => path,
        }
    }
}

impl AppState {
    /// 置き場所を決める。**盤面は載せません**——持っているのは webview です
    /// （[ADR 0039]）。
    ///
    /// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
    pub fn open(source: Source) -> Self {
        Self { source }
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    /// SQLite のファイルの場所。
    pub fn database_path(&self) -> &Path {
        self.source.path()
    }

    /// 置き場所を開く。
    pub(crate) fn store(&self) -> Result<Store, StoreError> {
        self.source.open()
    }
}
