use gpui_kit::{Action, App, KeyBinding, Menu, MenuItem, OsAction, SharedString, SystemMenuType};

use crate::hotkey::platform_support;

use crate::actions::{
    About, AddBoard, AddCard, AddColumn, AddTag, BackupDatabase, CancelEdit, ClearSearch,
    CloseWindow, DeleteBoard, ExportBoardJson, ExportBoardMarkdown, FocusSearch, HideApplication,
    HideOtherApplications, ManageTags, Quit, Redo, RenameBoard, RevealBackups, RevealDatabase,
    SaveEdit, SetQuickCaptureShortcut, ShowAllApplications, ToggleArchiveView, ToggleBoardList,
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

    cx.set_menus(menus());
}

/// ネイティブのメニューバー。
///
/// これを OS が出してくれるのは macOS だけ。ほかの環境では `cx.set_menus` が
/// 何もしないので、[`app_menu`] を画面の中に出す。
pub fn menus() -> Vec<Menu> {
    vec![
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
            MenuItem::action("バックアップの場所をFinderで開く", RevealBackups),
            MenuItem::separator(),
            MenuItem::action("ekanbanについて", About),
        ]),
    ]
}

/// ネイティブのメニューバーがあるかどうか。
///
/// 出してくれるのは macOS だけで、Linux と Windows では `cx.set_menus` が何も
/// しない。無い環境では、同じ項目を [`app_menu`] としてヘッダの `≡` から出す。
pub fn shows_in_app_menu() -> bool {
    !cfg!(target_os = "macos")
}

/// 画面の中に出すメニューの 1 項目。
pub struct AppMenuEntry {
    pub label: SharedString,
    /// 押したときに投げるアクション。メニューバーと同じものを投げるので、
    /// 操作の実体は `BoardView` の `on_action` 1 か所で済む。
    pub action: Box<dyn Action>,
    /// 押せない項目。灰色の項目は押せず理由を出す先が無いので、理由は
    /// `label` に含める。
    pub disabled: bool,
    /// 消える操作。`danger`（赤）で出す。
    pub danger: bool,
}

impl AppMenuEntry {
    fn new(label: impl Into<SharedString>, action: impl Action) -> Self {
        Self {
            label: label.into(),
            action: Box::new(action),
            disabled: false,
            danger: false,
        }
    }

    fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// 画面の中に出すメニューの見出しと、その下の項目。
pub struct AppMenuSection {
    pub title: SharedString,
    pub entries: Vec<AppMenuEntry>,
}

/// ネイティブのメニューバーが無い環境で、ヘッダの `≡` から出すメニュー。
///
/// [`menus`] と同じ操作をたどれるようにする。片方にだけ項目が増えていないかは
/// このモジュールのテストが見ている。
pub fn app_menu() -> Vec<AppMenuSection> {
    let (quick_capture_label, quick_capture_disabled) = quick_capture_item();
    vec![
        AppMenuSection {
            title: "ボード".into(),
            entries: vec![
                AppMenuEntry::new("ボードを追加", AddBoard),
                AppMenuEntry::new("ボード名を変更", RenameBoard),
                AppMenuEntry::new("現在のボードを削除", DeleteBoard).danger(),
            ],
        },
        AppMenuSection {
            title: "カード".into(),
            entries: vec![
                AppMenuEntry::new("カードを追加", AddCard),
                AppMenuEntry::new("カラムを追加", AddColumn),
                AppMenuEntry::new("タグを追加", AddTag),
                AppMenuEntry::new("タグを整理…", ManageTags),
            ],
        },
        AppMenuSection {
            title: "編集".into(),
            entries: vec![
                AppMenuEntry::new("元に戻す", Undo),
                AppMenuEntry::new("やり直す", Redo),
            ],
        },
        AppMenuSection {
            title: "表示".into(),
            entries: vec![
                AppMenuEntry::new("検索にフォーカス", FocusSearch),
                AppMenuEntry::new("検索をクリア", ClearSearch),
                AppMenuEntry::new("ボード一覧の表示を切り替え", ToggleBoardList),
                AppMenuEntry::new("アーカイブ表示を切り替え", ToggleArchiveView),
                AppMenuEntry::new("ライトモード", UseLightTheme),
                AppMenuEntry::new("ダークモード", UseDarkTheme),
                AppMenuEntry::new("システムに合わせる", UseSystemTheme),
                AppMenuEntry::new("フルスクリーンにする", ToggleFullscreen),
            ],
        },
        AppMenuSection {
            title: "データ".into(),
            entries: vec![
                AppMenuEntry::new("ボードを書き出す（JSON）", ExportBoardJson),
                AppMenuEntry::new("ボードを書き出す（Markdown）", ExportBoardMarkdown),
                AppMenuEntry::new("データベースをコピー…", BackupDatabase),
                AppMenuEntry::new("データベースの場所を開く", RevealDatabase),
                AppMenuEntry::new("バックアップの場所を開く", RevealBackups),
            ],
        },
        AppMenuSection {
            title: "その他".into(),
            entries: vec![
                AppMenuEntry::new(quick_capture_label, SetQuickCaptureShortcut)
                    .disabled(quick_capture_disabled),
                AppMenuEntry::new("ekanbanについて", About),
            ],
        },
    ]
}

/// 「クイックキャプチャのショートカット…」の文言と、押せるかどうか。
///
/// 使えない環境では灰色にする。灰色の項目は押せず理由を出す先が無いので、理由は
/// 文言に入れる。判定は起動中に変わらないので、呼ぶたびに数えても同じ答えになる。
fn quick_capture_item() -> (String, bool) {
    match platform_support() {
        Ok(()) => ("クイックキャプチャのショートカット…".to_string(), false),
        Err(reason) => (
            format!("クイックキャプチャのショートカット…（{reason}）"),
            true,
        ),
    }
}

fn quick_capture_menu_item() -> MenuItem {
    let (label, disabled) = quick_capture_item();
    MenuItem::action(label, SetQuickCaptureShortcut).disabled(disabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// メニューバーには出すが、画面内メニューには出さないアクション。
    ///
    /// - クリップボードと「すべてを選択」は OS の仕事で、`NoAction` を割り当てた
    ///   見出しでしかない
    /// - 「保存」「編集をキャンセル」は編集中しか意味がなく、そのときは入力欄が
    ///   キーを持っている。`Cmd/Ctrl+S` と `Escape` で足りる
    /// - 「ウインドウを閉じる」「終了」「隠す」「すべてを表示」はウィンドウ
    ///   マネージャと OS 側の操作
    fn kept_out_of_app_menu() -> Vec<&'static str> {
        vec![
            gpui_kit::NoAction.name(),
            SaveEdit.name(),
            CancelEdit.name(),
            CloseWindow.name(),
            Quit.name(),
            HideApplication.name(),
            HideOtherApplications.name(),
            ShowAllApplications.name(),
        ]
    }

    fn menu_bar_action_names() -> Vec<&'static str> {
        fn collect(items: &[MenuItem], names: &mut Vec<&'static str>) {
            for item in items {
                match item {
                    MenuItem::Action { action, .. } => names.push(action.name()),
                    MenuItem::Submenu(menu) => collect(&menu.items, names),
                    MenuItem::Separator | MenuItem::SystemMenu(_) => {}
                }
            }
        }

        let mut names = Vec::new();
        for menu in menus() {
            collect(&menu.items, &mut names);
        }
        names
    }

    fn app_menu_action_names() -> Vec<&'static str> {
        app_menu()
            .iter()
            .flat_map(|section| section.entries.iter())
            .map(|entry| entry.action.name())
            .collect()
    }

    #[test]
    fn offers_every_menu_bar_action_in_the_app_menu() {
        let excluded = kept_out_of_app_menu();
        let in_app = app_menu_action_names();
        let missing = menu_bar_action_names()
            .into_iter()
            .filter(|name| !excluded.contains(name))
            .filter(|name| !in_app.contains(name))
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "these menu bar actions cannot be reached without a menu bar: {missing:?}"
        );
    }

    #[test]
    fn keeps_the_app_menu_free_of_duplicates() {
        let mut names = app_menu_action_names();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len(), "the app menu lists an action twice");
    }
}
