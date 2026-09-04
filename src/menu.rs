use gpui_kit::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

use crate::actions::{
    About, AddCard, AddColumn, AddTag, CancelEdit, ClearSearch, CloseWindow, FocusSearch,
    HideApplication, HideOtherApplications, Quit, SaveEdit, ShowAllApplications, ShowAllCards,
    ShowOverdueCards, ShowThisWeekCards, ToggleArchiveView, ToggleFullscreen,
};

pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-n", AddCard, Some("Board")),
        KeyBinding::new("cmd-shift-n", AddColumn, Some("Board")),
        KeyBinding::new("cmd-shift-t", AddTag, Some("Board")),
        KeyBinding::new("cmd-f", FocusSearch, Some("Board")),
        KeyBinding::new("cmd-s", SaveEdit, Some("Board")),
        KeyBinding::new("cmd-shift-a", ToggleArchiveView, Some("Board")),
        KeyBinding::new("cmd-0", ShowAllCards, Some("Board")),
        KeyBinding::new("cmd-1", ShowOverdueCards, Some("Board")),
        KeyBinding::new("cmd-2", ShowThisWeekCards, Some("Board")),
        KeyBinding::new("cmd-shift-f", ClearSearch, Some("Board")),
        KeyBinding::new("cmd-w", CloseWindow, Some("Board")),
        KeyBinding::new("cmd-ctrl-f", ToggleFullscreen, Some("Board")),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", HideApplication, None),
        KeyBinding::new("cmd-alt-h", HideOtherApplications, None),
    ]);

    cx.set_menus([
        Menu::new("ekanban").items([
            MenuItem::action("ekanbanについて", About),
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
            MenuItem::action("カードを追加", AddCard),
            MenuItem::action("カラムを追加", AddColumn),
            MenuItem::action("タグを追加", AddTag),
            MenuItem::separator(),
            MenuItem::action("保存", SaveEdit),
            MenuItem::action("ウインドウを閉じる", CloseWindow),
        ]),
        Menu::new("編集").items([
            MenuItem::os_action("元に戻す", gpui_kit::NoAction, OsAction::Undo).disabled(true),
            MenuItem::os_action("やり直す", gpui_kit::NoAction, OsAction::Redo).disabled(true),
            MenuItem::separator(),
            MenuItem::os_action("カット", gpui_kit::NoAction, OsAction::Cut),
            MenuItem::os_action("コピー", gpui_kit::NoAction, OsAction::Copy),
            MenuItem::os_action("ペースト", gpui_kit::NoAction, OsAction::Paste),
            MenuItem::os_action("すべてを選択", gpui_kit::NoAction, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action("編集をキャンセル", CancelEdit),
            MenuItem::action("検索をクリア", ClearSearch),
        ]),
        Menu::new("表示").items([
            MenuItem::action("検索にフォーカス", FocusSearch),
            MenuItem::action("すべてのカード", ShowAllCards),
            MenuItem::action("期限切れのカード", ShowOverdueCards),
            MenuItem::action("今週までのカード", ShowThisWeekCards),
            MenuItem::separator(),
            MenuItem::action("アーカイブ表示を切り替え", ToggleArchiveView),
            MenuItem::action("フルスクリーンにする", ToggleFullscreen),
        ]),
        Menu::new("ウインドウ").items([MenuItem::action("ウインドウを閉じる", CloseWindow)]),
        Menu::new("ヘルプ").items([MenuItem::action("ekanbanについて", About)]),
    ]);
}
