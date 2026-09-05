use gpui_kit::component::GlobalState;
use gpui_kit::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType};

use crate::hotkey::platform_support;

use crate::actions::{
    About, AddBoard, AddCard, AddColumn, AddTag, BackupDatabase, CancelEdit, ClearSearch,
    CloseWindow, DeleteBoard, ExportBoardJson, ExportBoardMarkdown, FocusSearch, HideApplication,
    HideOtherApplications, ManageTags, MinimizeWindow, Quit, Redo, RenameBoard, RevealBackups,
    RevealDatabase, SaveEdit, SetQuickCaptureShortcut, ShowAllApplications, ToggleArchiveView,
    ToggleBoardList, ToggleFullscreen, Undo, UseDarkTheme, UseLightTheme, UseSystemTheme,
    ZoomWindow,
};

pub fn install(cx: &mut App) {
    cx.bind_keys(shared_key_bindings());
    cx.bind_keys(platform_key_bindings());

    cx.set_menus(menus());
    publish_menus_to_the_component_layer(cx);
}

/// 同じメニューを `AppMenuBar` からも読めるようにする。
///
/// `AppMenuBar` が読むのは `GlobalState::app_menus` で、`cx.set_menus` はそこまでは
/// 運ばない。橋渡しはアプリの仕事。
///
/// `cx.get_menus()` で読み返さずに [`menus`] をもう一度組むのは、しまうかどうかが
/// プラットフォーム任せだから。macOS・Linux・Windows は返すが、テストの
/// プラットフォームは何も返さず、メニューバーが空のまま出る。
fn publish_menus_to_the_component_layer(cx: &mut App) {
    let menus = menus().into_iter().map(Menu::owned).collect();
    GlobalState::global_mut(cx).set_app_menus(menus);
}

/// どの OS でも同じ割り当て。
///
/// `secondary` は macOS では Cmd、Linux と Windows では Ctrl になる。アプリ独自の
/// 割り当てはこれで定義する。
fn shared_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("secondary-shift-b", AddBoard, Some("Board")),
        KeyBinding::new("secondary-n", AddCard, Some("Board")),
        KeyBinding::new("secondary-shift-n", AddColumn, Some("Board")),
        KeyBinding::new("secondary-shift-t", AddTag, Some("Board")),
        KeyBinding::new("secondary-f", FocusSearch, Some("Board")),
        KeyBinding::new("secondary-s", SaveEdit, Some("Board")),
        KeyBinding::new("secondary-shift-a", ToggleArchiveView, Some("Board")),
        KeyBinding::new("secondary-shift-f", ClearSearch, Some("Board")),
        KeyBinding::new("secondary-w", CloseWindow, Some("Board")),
        KeyBinding::new("secondary-z", Undo, Some("Board")),
        KeyBinding::new("secondary-shift-z", Redo, Some("Board")),
    ]
}

/// macOS だけの割り当て。
///
/// gpui の `cmd-` は platform 修飾キーなので、macOS では Cmd、Linux と Windows では
/// Super（Windows キー）になる。ここに置くのは Super では意味を成さないものだけ:
///
/// - `cmd-ctrl-*` は macOS の標準の組み合わせ。`secondary-ctrl-*` にすると非 macOS で
///   `Ctrl` が重なって `secondary-s` / `secondary-f` と衝突するので、そもそも共通には
///   できない
/// - `cmd-q` `cmd-h` `cmd-alt-h` は macOS のシステムメニューの割り当て
/// - `cmd-m`（しまう）は、macOS ではメニュー項目があってはじめて効く。ほかの環境では
///   最小化はウィンドウマネージャの仕事
#[cfg(target_os = "macos")]
fn platform_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-ctrl-s", ToggleBoardList, Some("Board")),
        KeyBinding::new("cmd-ctrl-f", ToggleFullscreen, Some("Board")),
        KeyBinding::new("cmd-m", MinimizeWindow, Some("Board")),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", HideApplication, None),
        KeyBinding::new("cmd-alt-h", HideOtherApplications, None),
    ]
}

/// macOS 以外の割り当て。
///
/// macOS 向けの `cmd-*` をそのまま持ち込むと Super に落ちる。Super はデスクトップ
/// 環境が先に取るので、終了もフルスクリーンもボード一覧も、届く手段が無くなる（#53）。
/// 同じ操作に、その OS の慣習どおりの割り当てを足す。
///
/// `F11` は X11 / Windows のフルスクリーンの慣習。`Ctrl+Q` は終了、`Ctrl+B` は脇の
/// 一覧の開け閉め。いずれも [`shared_key_bindings`] と重ならない。
#[cfg(not(target_os = "macos"))]
fn platform_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("f11", ToggleFullscreen, Some("Board")),
        KeyBinding::new("secondary-b", ToggleBoardList, Some("Board")),
        KeyBinding::new("secondary-q", Quit, None),
    ]
}

/// メニューバーの中身。
///
/// OS が描いてくれるのは macOS だけ。ほかの環境では [`crate::views::menu_bar`] が
/// 同じ定義を読んで自分で描く（[ADR 0015](../docs/adr/0015-a-menu-bar-on-every-platform.md)）。
///
/// 構成は OS ごとに違う。macOS のシステムメニューとウィンドウ操作は、ほかの環境では
/// 意味を成さないため。どちらも `cfg` を付けずに定義して、テストがどの OS でも両方を
/// 突き合わせられるようにする。
pub fn menus() -> Vec<Menu> {
    if cfg!(target_os = "macos") {
        macos_menus()
    } else {
        drawn_menus()
    }
}

/// macOS のネイティブなメニューバー。
fn macos_menus() -> Vec<Menu> {
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
        Menu::new("編集").items(edit_menu_items()),
        Menu::new("ボード").items(board_menu_items()),
        Menu::new("表示").items(view_menu_items()),
        // macOS の標準の Window メニューは gpui からは組めない（`SystemMenuType` は
        // `Services` しか持たない）ので、項目を自分で並べる。`Cmd+M` はメニューに
        // 項目があってはじめて効く。
        Menu::new("ウインドウ").items([
            MenuItem::action("しまう", MinimizeWindow),
            MenuItem::action("拡大／縮小", ZoomWindow),
            MenuItem::separator(),
            MenuItem::action("ウインドウを閉じる", CloseWindow),
        ]),
        Menu::new("ヘルプ").items([
            MenuItem::action("データベースをコピー…", BackupDatabase),
            MenuItem::action("データベースの場所をFinderで開く", RevealDatabase),
            MenuItem::action("バックアップの場所をFinderで開く", RevealBackups),
            MenuItem::separator(),
            MenuItem::action("ekanbanについて", About),
        ]),
    ]
}

/// Linux と Windows で、アプリが自分で描くメニューバー。
///
/// macOS との違いはアプリメニューとウインドウメニューが無いことで、そのぶん
/// 「終了」は ファイル に、「ekanbanについて」とクイックキャプチャの設定は
/// ヘルプ に移す。macOS のシステム項目（サービス・隠す・すべてを表示）と
/// ウィンドウ操作（しまう・拡大／縮小）は、ほかの環境ではウィンドウマネージャの
/// 仕事なので出さない。
fn drawn_menus() -> Vec<Menu> {
    vec![
        Menu::new("ファイル").items([
            MenuItem::action("ボードを追加", AddBoard),
            MenuItem::action("カードを追加", AddCard),
            MenuItem::action("カラムを追加", AddColumn),
            MenuItem::action("タグを追加", AddTag),
            MenuItem::separator(),
            MenuItem::action("ボードを書き出す（JSON）", ExportBoardJson),
            MenuItem::action("ボードを書き出す（Markdown）", ExportBoardMarkdown),
            MenuItem::separator(),
            MenuItem::action("保存", SaveEdit),
            MenuItem::action("ウインドウを閉じる", CloseWindow),
            MenuItem::action("終了", Quit),
        ]),
        Menu::new("編集").items(edit_menu_items()),
        Menu::new("ボード").items(board_menu_items()),
        Menu::new("表示").items(view_menu_items()),
        Menu::new("ヘルプ").items([
            quick_capture_menu_item(),
            MenuItem::separator(),
            MenuItem::action("データベースをコピー…", BackupDatabase),
            MenuItem::action("データベースの場所をフォルダで開く", RevealDatabase),
            MenuItem::action("バックアップの場所をフォルダで開く", RevealBackups),
            MenuItem::separator(),
            MenuItem::action("ekanbanについて", About),
        ]),
    ]
}

/// どの OS でも同じ「編集」メニュー。
fn edit_menu_items() -> Vec<MenuItem> {
    vec![
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
    ]
}

/// どの OS でも同じ「ボード」メニュー。
fn board_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::action("ボード名を変更", RenameBoard),
        MenuItem::action("現在のボードを削除", DeleteBoard),
        MenuItem::separator(),
        MenuItem::action("タグを整理…", ManageTags),
    ]
}

/// どの OS でも同じ「表示」メニュー。
fn view_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::action("検索にフォーカス", FocusSearch),
        MenuItem::separator(),
        MenuItem::action("ボード一覧の表示を切り替え", ToggleBoardList),
        MenuItem::action("アーカイブ表示を切り替え", ToggleArchiveView),
        MenuItem::separator(),
        MenuItem::action("ライトモード", UseLightTheme),
        MenuItem::action("ダークモード", UseDarkTheme),
        MenuItem::action("システムに合わせる", UseSystemTheme),
        MenuItem::action("フルスクリーンにする", ToggleFullscreen),
    ]
}

/// メニューバーを OS が描くかどうか。
///
/// 描いてくれるのは macOS だけ。ほかの環境では [`crate::views::menu_bar`] が
/// [`menus`] を読んで画面の中に描く。
pub fn draws_its_own_menu_bar() -> bool {
    !cfg!(target_os = "macos")
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

    use gpui_kit::Action as _;

    /// macOS のメニューバーにしか出さないアクション。
    ///
    /// - 「サービス」「隠す」「ほかを隠す」「すべてを表示」は macOS のシステム
    ///   メニューの項目
    /// - 「しまう」「拡大／縮小」は macOS のウィンドウ操作。macOS の `Cmd+M` は
    ///   メニュー項目があってはじめて効くので置いているが、ほかの環境では最小化も
    ///   最大化もウィンドウマネージャの仕事で、アプリのメニューに出す意味がない
    fn macos_only() -> Vec<&'static str> {
        vec![
            HideApplication.name(),
            HideOtherApplications.name(),
            ShowAllApplications.name(),
            MinimizeWindow.name(),
            ZoomWindow.name(),
        ]
    }

    fn action_names(menus: Vec<Menu>) -> Vec<&'static str> {
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
        for menu in menus {
            collect(&menu.items, &mut names);
        }
        names
    }

    /// 受け入れ条件「macOS でたどれる操作は Linux・Windows でもたどれる」（#79）。
    ///
    /// 定義が 2 本ある以上、片方にだけ項目が増える事故は起きる。両方を `cfg` 無しで
    /// 定義してあるので、この照合はどの OS の CI でも同じように働く。
    #[test]
    fn offers_every_macos_action_on_the_other_platforms_too() {
        let macos_only = macos_only();
        let drawn = action_names(drawn_menus());
        let missing = action_names(macos_menus())
            .into_iter()
            .filter(|name| !macos_only.contains(name))
            .filter(|name| !drawn.contains(name))
            .collect::<Vec<_>>();

        assert!(
            missing.is_empty(),
            "these actions are on the macOS menu bar but nowhere on the drawn one: {missing:?}"
        );
    }

    /// 受け入れ条件「Linux・Windows のメニューに macOS 専用の項目が入らない」（#79）。
    #[test]
    fn keeps_the_macos_system_commands_off_the_drawn_menu_bar() {
        let on_macos = action_names(macos_menus());
        let drawn = action_names(drawn_menus());
        for command in macos_only() {
            assert!(
                on_macos.contains(&command),
                "{command} belongs on the macOS menu bar"
            );
            assert!(
                !drawn.contains(&command),
                "{command} is a macOS-only command and does not belong on the drawn menu bar"
            );
        }
    }

    /// 「サービス」は macOS のシステムメニューで、ほかの環境では出しようがない。
    #[test]
    fn keeps_the_services_submenu_off_the_drawn_menu_bar() {
        fn has_system_menu(items: &[MenuItem]) -> bool {
            items.iter().any(|item| match item {
                MenuItem::SystemMenu(_) => true,
                MenuItem::Submenu(menu) => has_system_menu(&menu.items),
                _ => false,
            })
        }

        assert!(
            macos_menus()
                .iter()
                .any(|menu| has_system_menu(&menu.items)),
            "the macOS application menu carries the Services submenu"
        );
        assert!(
            !drawn_menus()
                .iter()
                .any(|menu| has_system_menu(&menu.items)),
            "there is no system menu to hand these items to outside macOS"
        );
    }

    /// メニューバーが無い環境でも、終了とアプリについては手が届く必要がある（#79）。
    #[test]
    fn reaches_quit_and_about_from_the_drawn_menu_bar() {
        let drawn = action_names(drawn_menus());
        for action in [Quit.name(), About.name(), SetQuickCaptureShortcut.name()] {
            assert!(
                drawn.contains(&action),
                "{action} has no home on the drawn menu bar"
            );
        }
    }

    fn all_key_bindings() -> Vec<KeyBinding> {
        let mut bindings = shared_key_bindings();
        bindings.extend(platform_key_bindings());
        bindings
    }

    /// 割り当てを `"ctrl-cmd-f"` のような文字列にする。`cmd` は gpui の platform
    /// 修飾キーで、macOS では Cmd、それ以外では Super になる。
    fn written_out(binding: &KeyBinding) -> String {
        binding
            .keystrokes()
            .iter()
            .map(|keystroke| {
                // 表示用ではなく gpui の表現のほうを見る。Windows では
                // `modifiers()` が表示用に書き換わることがある。
                let modifiers = &keystroke.inner().modifiers;
                let mut text = String::new();
                for (held, name) in [
                    (modifiers.control, "ctrl-"),
                    (modifiers.alt, "alt-"),
                    (modifiers.shift, "shift-"),
                    (modifiers.platform, "cmd-"),
                    (modifiers.function, "fn-"),
                ] {
                    if held {
                        text.push_str(name);
                    }
                }
                text.push_str(&keystroke.inner().key);
                text
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn keystrokes_for(action: &str) -> Vec<String> {
        all_key_bindings()
            .iter()
            .filter(|binding| binding.action().name() == action)
            .map(written_out)
            .collect()
    }

    /// 終了・フルスクリーン・ボード一覧には、どの OS でも届く手段がある（#53）。
    #[test]
    fn binds_quit_fullscreen_and_the_board_list_on_every_platform() {
        for action in [Quit.name(), ToggleFullscreen.name(), ToggleBoardList.name()] {
            assert!(
                !keystrokes_for(action).is_empty(),
                "{action} has no key binding on this platform"
            );
        }
    }

    #[test]
    fn assigns_each_combination_to_a_single_action() {
        let mut written = all_key_bindings()
            .iter()
            .map(written_out)
            .collect::<Vec<_>>();
        let before = written.len();
        written.sort();
        written.dedup();
        assert_eq!(before, written.len(), "two actions share a combination");
    }

    /// 受け入れ条件「Linux で `F11` がフルスクリーン、`Ctrl+Q` が終了になる」（#53）。
    ///
    /// gpui の platform 修飾キーは macOS 以外では Super（Windows キー）になり、
    /// デスクトップ環境が先に取る。届かない割り当てを唯一の手段にしない。
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn reaches_every_command_without_the_super_key_outside_macos() {
        for binding in all_key_bindings() {
            for keystroke in binding.keystrokes() {
                assert!(
                    !keystroke.inner().modifiers.platform,
                    "{} falls onto Super outside macOS",
                    binding.action().name()
                );
            }
        }

        assert_eq!(keystrokes_for(ToggleFullscreen.name()), vec!["f11"]);
        assert_eq!(keystrokes_for(Quit.name()), vec!["ctrl-q"]);
        assert_eq!(keystrokes_for(ToggleBoardList.name()), vec!["ctrl-b"]);
    }

    /// 受け入れ条件「macOS の既存の割り当ては変わらない」（#53）。
    #[cfg(target_os = "macos")]
    #[test]
    fn keeps_the_macos_combinations_as_they_were() {
        assert_eq!(keystrokes_for(ToggleFullscreen.name()), vec!["ctrl-cmd-f"]);
        assert_eq!(keystrokes_for(ToggleBoardList.name()), vec!["ctrl-cmd-s"]);
        assert_eq!(keystrokes_for(Quit.name()), vec!["cmd-q"]);
        assert_eq!(keystrokes_for(MinimizeWindow.name()), vec!["cmd-m"]);
        assert_eq!(keystrokes_for(HideApplication.name()), vec!["cmd-h"]);
        assert_eq!(
            keystrokes_for(HideOtherApplications.name()),
            vec!["alt-cmd-h"]
        );
    }

    /// 1 つのメニューの中に同じ操作が二度出ていないか。
    ///
    /// メニューをまたぐ重なりは数えない。macOS では「ウインドウを閉じる」が
    /// ファイル と ウインドウ に、「ekanbanについて」が ekanban と ヘルプ に出るのが
    /// 作法どおり。`NoAction` はクリップボード操作に付けた見出しで、アクションでは
    /// ないので除く。
    #[test]
    fn keeps_each_menu_free_of_duplicates() {
        for (bar, menus) in [("macOS", macos_menus()), ("drawn", drawn_menus())] {
            for menu in menus {
                let name = menu.name.clone();
                let mut names = action_names(vec![menu])
                    .into_iter()
                    .filter(|action| *action != gpui_kit::NoAction.name())
                    .collect::<Vec<_>>();
                let before = names.len();
                names.sort_unstable();
                names.dedup();
                assert_eq!(
                    before,
                    names.len(),
                    "the {name} menu on the {bar} menu bar lists an action twice"
                );
            }
        }
    }
}
