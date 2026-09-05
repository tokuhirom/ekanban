use gpui_kit::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

use crate::hotkey::platform_support;

use crate::actions::{
    About, AddBoard, AddCard, AddColumn, AddTag, BackupDatabase, CancelEdit, ClearSearch,
    CloseWindow, DeleteBoard, ExportBoardJson, ExportBoardMarkdown, FocusSearch, HideApplication,
    HideOtherApplications, ManageTags, Quit, Redo, RenameBoard, RevealDatabase, SaveEdit,
    SetQuickCaptureShortcut, ShowAllApplications, ToggleArchiveView, ToggleBoardList,
    ToggleFullscreen, Undo, UseDarkTheme, UseLightTheme, UseSystemTheme,
};

pub fn install(cx: &mut App) {
    // `secondary` は macOS では Cmd、Linux と Windows では Ctrl になる。アプリ独自の
    // 割り当てはこれで定義する。`cmd-` を残すのは 2 種類だけ:
    //   - `cmd-q` `cmd-h` `cmd-alt-h` は macOS のシステムメニューの割り当て
    //   - `cmd-ctrl-*` は `secondary-ctrl-*` にすると非 macOS で Ctrl が重なって
    //     `secondary-s` / `secondary-f` に潰れ、保存や検索と衝突する
    cx.bind_keys([
        KeyBinding::new("secondary-shift-b", AddBoard, Some("Board")),
        KeyBinding::new("secondary-n", AddCard, Some("Board")),
        KeyBinding::new("secondary-shift-n", AddColumn, Some("Board")),
        KeyBinding::new("secondary-shift-t", AddTag, Some("Board")),
        KeyBinding::new("secondary-f", FocusSearch, Some("Board")),
        KeyBinding::new("secondary-s", SaveEdit, Some("Board")),
        KeyBinding::new("secondary-shift-a", ToggleArchiveView, Some("Board")),
        KeyBinding::new("cmd-ctrl-s", ToggleBoardList, Some("Board")),
        KeyBinding::new("secondary-shift-f", ClearSearch, Some("Board")),
        KeyBinding::new("secondary-w", CloseWindow, Some("Board")),
        KeyBinding::new("cmd-ctrl-f", ToggleFullscreen, Some("Board")),
        KeyBinding::new("secondary-z", Undo, Some("Board")),
        KeyBinding::new("secondary-shift-z", Redo, Some("Board")),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", HideApplication, None),
        KeyBinding::new("cmd-alt-h", HideOtherApplications, None),
    ]);

    cx.set_menus([
        Menu::new("ekanban").items([
            MenuItem::action("ekanbanについて", About),
            MenuItem::separator(),
            quick_capture_menu_item(),
            MenuItem::separator(),
            MenuItem::os_submenu("サービス", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("ekanbanを隠す", HideApplication),
            MenuItem::action("ほかを隠す", HideOtherApplications),
            MenuItem::action("すべてを表示", ShowAllApplications),
            MenuItem::separator(),
            MenuItem::action("ekanbanを終了", Quit),
        ]),
        Menu::new("ファイル").items([
            MenuItem::action("ボードを追加", AddBoard),
            MenuItem::action("カードを追加", AddCard),
            MenuItem::action("カラムを追加", AddColumn),
            MenuItem::action("タグを追加", AddTag),
            MenuItem::action("ボードを書き出す（JSON）", ExportBoardJson),
            MenuItem::action("ボードを書き出す（Markdown）", ExportBoardMarkdown),
            MenuItem::separator(),
            MenuItem::action("保存", SaveEdit),
            MenuItem::action("ウインドウを閉じる", CloseWindow),
        ]),
        Menu::new("編集").items([
            MenuItem::os_action("元に戻す", Undo, OsAction::Undo),
            MenuItem::os_action("やり直す", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("カット", gpui_kit::NoAction, OsAction::Cut),
            MenuItem::os_action("コピー", gpui_kit::NoAction, OsAction::Copy),
            MenuItem::os_action("ペースト", gpui_kit::NoAction, OsAction::Paste),
            MenuItem::os_action("すべてを選択", gpui_kit::NoAction, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action("編集をキャンセル", CancelEdit),
            MenuItem::action("検索をクリア", ClearSearch),
        ]),
        Menu::new("ボード").items([
            MenuItem::action("ボード名を変更", RenameBoard),
            MenuItem::action("現在のボードを削除", DeleteBoard),
            MenuItem::separator(),
            MenuItem::action("タグを整理…", ManageTags),
        ]),
        Menu::new("表示").items([
            MenuItem::action("検索にフォーカス", FocusSearch),
            MenuItem::separator(),
            MenuItem::action("ボード一覧の表示を切り替え", ToggleBoardList),
            MenuItem::action("アーカイブ表示を切り替え", ToggleArchiveView),
            MenuItem::separator(),
            MenuItem::action("ライトモード", UseLightTheme),
            MenuItem::action("ダークモード", UseDarkTheme),
            MenuItem::action("システムに合わせる", UseSystemTheme),
            MenuItem::action("フルスクリーンにする", ToggleFullscreen),
        ]),
        Menu::new("ウインドウ").items([MenuItem::action("ウインドウを閉じる", CloseWindow)]),
        Menu::new("ヘルプ").items([
            MenuItem::action("データベースをコピー…", BackupDatabase),
            MenuItem::action("データベースの場所をFinderで開く", RevealDatabase),
            MenuItem::separator(),
            MenuItem::action("ekanbanについて", About),
        ]),
    ]);
}

/// 「クイックキャプチャのショートカット…」。
///
/// 使えない環境では灰色にする。灰色の項目は押せず理由を出す先が無いので、理由は
/// 文言に入れる。判定は起動中に変わらないので、ここで 1 回決めれば足りる。
fn quick_capture_menu_item() -> MenuItem {
    match platform_support() {
        Ok(()) => MenuItem::action(
            "クイックキャプチャのショートカット…",
            SetQuickCaptureShortcut,
        ),
        Err(reason) => MenuItem::action(
            format!("クイックキャプチャのショートカット…（{reason}）"),
            SetQuickCaptureShortcut,
        )
        .disabled(true),
    }
}
