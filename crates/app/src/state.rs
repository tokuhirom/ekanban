//! 置き場所をどこに開くか（[ADR 0039]）。
//!
//! **盤面は持ちません。** 持っているのは webview で、こちらは読み書きを頼まれる
//! 側です。ここに残っているのは「どのファイルか」「ブラウザのどこか」だけです。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

#[cfg(feature = "shell")]
use std::path::Path;
use std::sync::{Arc, Mutex, TryLockError};

use ekanban_core::store::{JsonStore, Store, StoreError};

/// 置き場所への入口。
pub struct AppState {
    source: Source,
}

/// 盤面をどこに置くか（[ADR 0036]）。
///
/// **開くたびに開き直します。** クイックキャプチャの窓が閉じている間にも書ける
/// ので、メモリ上の写しを抱え続けると、そちらの変更が見えません。JSON の側は
/// ページに 1 つしかないので、同じものを借り直します。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
#[derive(Clone)]
pub enum Source {
    #[cfg(feature = "shell")]
    Sqlite(std::path::PathBuf),
    Json {
        store: Arc<Mutex<JsonStore>>,
        /// 「どこにあるか」を人が読む形で。**パスではありません**——ブラウザに
        /// ファイルシステム上の居場所は無いので、置いた側が名乗ります。
        place: String,
    },
}

impl Source {
    /// 置き場所を開く。
    pub fn open(&self) -> Result<Store<'_>, StoreError> {
        match self {
            #[cfg(feature = "shell")]
            Self::Sqlite(path) => Ok(Store::Sqlite(ekanban_core::db::Database::open(path)?)),
            // **待ちません。** 取れないのは、同じ置き場所を開いたまま
            // もう 1 つ開いたときだけです（`AppState::store` の注意書き）。
            // 待つと、相手が自分なので二度と空きません。
            Self::Json { store, .. } => match store.try_lock() {
                Ok(guard) => Ok(Store::Json(guard)),
                Err(TryLockError::Poisoned(poisoned)) => Ok(Store::Json(poisoned.into_inner())),
                Err(TryLockError::WouldBlock) => Err(StoreError::AlreadyOpen),
            },
        }
    }

    /// 盤面がどこにあるか。画面（「ekanban について」）に出す文言。
    pub fn place(&self) -> String {
        match self {
            #[cfg(feature = "shell")]
            Self::Sqlite(path) => path.to_string_lossy().into_owned(),
            Self::Json { place, .. } => place.clone(),
        }
    }

    /// SQLite のファイルの場所。JSON の置き場所には無い。
    #[cfg(feature = "shell")]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Sqlite(path) => Some(path),
            Self::Json { .. } => None,
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

    /// SQLite のファイルの場所。**殻を持っているときだけ**あります。
    #[cfg(feature = "shell")]
    pub fn database_path(&self) -> &Path {
        self.source
            .path()
            .expect("殻の側は必ず SQLite のファイルを持つ")
    }

    /// 置き場所を開く。
    ///
    /// **開いたまま、もう 1 つ開かないでください。** SQLite は接続を 2 つ持てる
    /// ので気づけませんが、ブラウザ版の置き場所はページに 1 つで、同じ
    /// `Mutex` を 2 度取ると落ちます（[ADR 0036]）。持っている置き場所から
    /// 組み立てるか、先に `drop` してください。
    ///
    /// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
    pub(crate) fn store(&self) -> Result<Store<'_>, StoreError> {
        self.source.open()
    }
}
