//! コマンドが失敗を伝える形（`docs/DESIGN.md`「コマンドとイベント」）。
//!
//! [ADR 0016](../../../docs/adr/0016-where-the-app-says-things.md) の「アプリが
//! 伝えることを行き先ごとに分ける」はそのまま生きます。**行き先を決める材料を、
//! コマンドの側が付けて返します**——`Validation` は入力欄の脇に、それ以外は
//! ダイアログに。**文言もここで決めます**。`rusqlite` や `BoardError` の
//! 文字列をそのまま出すと、次に何をすればよいかが伝わりません。
//!
//! 拒否・キャンセル・変更なしは、いまと同じく**何も言いません**。だからこれらは
//! `AppError` にならず、`Ok` のまま返ります。

use ekanban_core::model::BoardError;
use ekanban_core::store::StoreError;
use serde::Serialize;
use ts_rs::TS;

/// 失敗をどこに出すか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorKind {
    /// 保存に失敗した。盤面への変更は捨ててある。
    Save,
    /// ボードを読む・作る・消すのに失敗した。
    BoardIo,
    /// 書き出し・控えの保存に失敗した。
    Export,
    /// クイックキャプチャの割り当てを登録できなかった。
    Shortcut,
    /// 入力が受け取れない。入力欄の脇に出す。
    Validation,
}

/// 入力欄に返す失敗の行き先。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Field {
    CardTitle,
    DueDate,
    ChecklistItem,
    ColumnName,
    TagName,
    BoardName,
}

/// コマンドの `Err`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppError {
    pub kind: ErrorKind,
    /// ダイアログの見出し。
    pub title: String,
    /// 使う人が手を打てる言葉に直したもの。
    pub detail: String,
    /// 入力欄に返す場合だけ。`kind` が `Validation` のときに入る。
    pub field: Option<Field>,
    /// 入力欄に返すとき、拒否された値。打ち直せるように返す。
    pub value: Option<String>,
}

impl AppError {
    pub fn new(kind: ErrorKind, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            detail: detail.into(),
            field: None,
            value: None,
        }
    }

    /// 盤面の操作が拒否された。入力の間違いなら入力欄に、それ以外はダイアログに。
    pub fn from_board(title: &str, error: &BoardError) -> Self {
        match field_for(error) {
            Some((field, detail, value)) => Self {
                kind: ErrorKind::Validation,
                title: title.to_string(),
                detail: detail.to_string(),
                field: Some(field),
                value,
            },
            None => Self::new(ErrorKind::BoardIo, title, board_detail(error)),
        }
    }

    /// 保存に失敗した。盤面への変更は捨ててあるので、画面は何も戻さなくてよい。
    pub fn from_save(error: &StoreError) -> Self {
        Self::new(ErrorKind::Save, "保存に失敗しました", db_detail(error))
    }

    pub fn from_db(kind: ErrorKind, title: &str, error: &StoreError) -> Self {
        Self::new(kind, title, db_detail(error))
    }
}

/// 入力欄の脇に出す失敗。出さないものは `None`。
///
/// 入力の間違いは、押した場所のそばで直せるほうが速い。ダイアログに出すと、
/// 閉じてから打ち直すことになる。
fn field_for(error: &BoardError) -> Option<(Field, &'static str, Option<String>)> {
    let (field, message, value) = match error {
        BoardError::EmptyCardTitle => (Field::CardTitle, "タイトルを入力してください", None),
        BoardError::InvalidDueDate(value) => (
            Field::DueDate,
            "日付として読めません。9/12、明日、金、+3 のように入力してください（空欄で期限なし）",
            Some(value.clone()),
        ),
        BoardError::EmptyColumnName => (Field::ColumnName, "カラム名を入力してください", None),
        BoardError::EmptyTagName => (Field::TagName, "タグ名を入力してください", None),
        BoardError::DuplicateTagName(_) => (
            Field::TagName,
            "同じ名前のタグがすでにあります。別の名前を入力してください",
            None,
        ),
        BoardError::EmptyChecklistItemText => {
            (Field::ChecklistItem, "チェック項目を入力してください", None)
        }
        BoardError::EmptyBoardName => (Field::BoardName, "ボード名を入力してください", None),
        _ => return None,
    };
    Some((field, message, value))
}

/// 盤面の操作が失敗した理由を、使う人が手を打てる言葉にする。
fn board_detail(error: &BoardError) -> String {
    match error {
        BoardError::EmptyBoardName => "ボード名を入力してください".to_string(),
        BoardError::ColumnNotFound(column_id) => {
            format!("カラム #{column_id} が見つかりません。画面を更新してください")
        }
        BoardError::CardNotFound(card_id) => {
            format!("カード #{card_id} が見つかりません。画面を更新してください")
        }
        BoardError::EmptyCardTitle => "タイトルを入力してください".to_string(),
        BoardError::EmptyColumnName => "カラム名を入力してください".to_string(),
        BoardError::InvalidDueDate(value) => {
            format!("期限「{value}」を日付として読めません。9/12、明日、金、+3 のように入力してください")
        }
        BoardError::EmptyTagName => "タグ名を入力してください".to_string(),
        BoardError::TagNotFound(tag_id) => {
            format!("タグ #{tag_id} が見つかりません。画面を更新してください")
        }
        BoardError::DuplicateTagName(name) => {
            format!("タグ「{name}」はすでに存在します。別の名前を入力してください")
        }
        BoardError::EmptyChecklistItemText => "チェック項目を入力してください".to_string(),
        BoardError::ChecklistItemNotFound(item_id, card_id) => {
            format!("カード #{card_id} のチェック項目 #{item_id} が見つかりません")
        }
        BoardError::LastColumn => "最後のカラムは削除できません".to_string(),
        // 使う人の入力の間違いではなく、画面の側の食い違い（ADR 0040）。
        // **理由をそのまま出します**——打ち直して直るものではないので、
        // 言い換えても手の打ちようが増えません。追う手がかりのほうが要ります。
        BoardError::Inconsistent(reason) => {
            format!("盤面を保存できる形になっていません（{reason}）。画面を更新してください")
        }
    }
}

/// 置き場所の失敗を、使う人が手を打てる言葉にする。
///
/// エラーコードごとに「次に何をすればよいか」を書くのが要点で、`rusqlite` の
/// 文言をそのまま出さない。
///
/// **SQLite の枝は SQLite を積んでいるときだけ**あります（[ADR 0036]）。
/// ブラウザ版の置き場所は JSON なので、ディスクや権限の話が出てきません。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
fn db_detail(error: &StoreError) -> String {
    match error {
        #[cfg(feature = "shell")]
        StoreError::Sqlite(error) => match error {
            rusqlite::Error::SqliteFailure(sqlite_error, message) => {
                let reason = message.as_deref().unwrap_or("詳細情報なし");
                match sqlite_error.code {
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                        "データベースが使用中です。ほかの操作が終わってから再試行してください"
                            .to_string()
                    }
                    rusqlite::ErrorCode::ReadOnly | rusqlite::ErrorCode::PermissionDenied => {
                        "データベースに書き込めません。保存先の権限を確認してください".to_string()
                    }
                    rusqlite::ErrorCode::DiskFull => {
                        "ディスク容量が不足しています。空き容量を確保してください".to_string()
                    }
                    rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                        "データベースが壊れているか、SQLite データベースではありません。バックアップを確認してください"
                            .to_string()
                    }
                    rusqlite::ErrorCode::CannotOpen => {
                        "データベースを開けません。保存先のパスと権限を確認してください".to_string()
                    }
                    _ => format!("SQLite の処理に失敗しました（{reason}）"),
                }
            }
            _ => format!("SQLite の処理に失敗しました（{error}）"),
        },
        StoreError::NoBoard => "ボードが見つかりません。画面を更新してください".to_string(),
        // ほかの窓が先に書いた（ADR 0040）。**打ち直す必要はありません**——
        // 読み直せば、そちらの変更が入った盤面が出ます。
        StoreError::Conflict { .. } => {
            "このボードはほかのウィンドウで変更されました。画面を更新してください".to_string()
        }
        StoreError::LastBoard => "最後のボードは削除できません".to_string(),
        StoreError::EmptyBoardName => "ボード名を入力してください".to_string(),
        StoreError::AlreadyOpen => {
            "盤面の置き場所を二重に開きました。開き直してください".to_string()
        }
        StoreError::InvalidAppState => {
            "保存されたアプリ状態を読み取れません。ボードを選び直してください".to_string()
        }
        StoreError::Json(error) => format!("ボードデータの変換に失敗しました（{error}）"),
    }
}
