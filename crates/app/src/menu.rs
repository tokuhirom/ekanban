//! メニューバーと、そこに付くキーの割り当て（`docs/DESIGN.md`「メニューとキー割り当て」）。
//!
//! **メニューは先に「データ」として組み、あとで Tauri のメニューに変換します。**
//! `tauri::menu::Menu` を作るにはアプリのハンドルが要り、そこだけで組むと構成を
//! 確かめるのに窓を開ける羽目になります。[`sections`] は値を返すだけなので、
//! テストが 3 つの OS 分の構成をそのまま読めます。
//!
//! 押されたときの行き先は 2 つあります。
//!
//! - [`AppAction`] は webview へ流します（`app:action`、`docs/DESIGN.md`「コマンドとイベント」）。盤面に触るもの、
//!   下書きに触るもの、表示の状態を変えるものはすべてこちら——**判断は画面が
//!   持っているから**です
//! - [`WindowAction`] は Rust が行います。ウィンドウそのものの操作で、webview
//!   には手が届きません
//!
//! テキスト編集（カット・コピー・ペースト・すべてを選択）と macOS のシステム
//! 項目は [`Predefined`] に任せます。OS が持っている操作を自分で書き直しません。

use serde::Serialize;
use tauri::menu::{
    AboutMetadata, Menu, MenuItemBuilder, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Runtime};
use ts_rs::TS;

/// webview が受け取るメニューの操作。`app:action` の積荷です。
///
/// 名前は TypeScript 側と 1 対 1 で、`ts-rs` が書き出します。**手で 2 か所に
/// 書きません**（`docs/DESIGN.md`「コマンドとイベント」）。dispatcher が網羅しているかどうかは、この型から作った
/// `Record` を TypeScript の型検査が見ます。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AppAction {
    AddBoard,
    AddCard,
    AddColumn,
    AddTag,
    ExportBoardJson,
    ExportBoardMarkdown,
    SaveEdit,
    Undo,
    Redo,
    CancelEdit,
    ClearSearch,
    RenameBoard,
    DeleteBoard,
    ManageTags,
    FocusSearch,
    ToggleBoardList,
    ToggleArchiveView,
    SetQuickCaptureShortcut,
    UseLightTheme,
    UseDarkTheme,
    UseSystemTheme,
    BackupDatabase,
    RevealDatabase,
    RevealBackups,
    About,
}

/// Rust が行うウィンドウの操作。
///
/// macOS では同じことを [`Predefined`] が持っているので、こちらに出てくるのは
/// macOS 以外だけです（`Alt+F4` や `Ctrl+Q` を OS 側の項目が持っていない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    CloseWindow,
    ToggleFullscreen,
    Quit,
}

/// メニューを押されたときに起きること。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    App(AppAction),
    Window(WindowAction),
}

impl Action {
    /// メニュー項目の id。webview へ流すときの積荷でもあります。
    pub fn id(self) -> &'static str {
        match self {
            Self::App(action) => action.id(),
            Self::Window(action) => action.id(),
        }
    }

    /// 押された項目の id から引き当てる。知らない id は `None`。
    pub fn from_id(id: &str) -> Option<Self> {
        ACTIONS.iter().copied().find(|action| action.id() == id)
    }
}

impl AppAction {
    pub fn id(self) -> &'static str {
        match self {
            Self::AddBoard => "addBoard",
            Self::AddCard => "addCard",
            Self::AddColumn => "addColumn",
            Self::AddTag => "addTag",
            Self::ExportBoardJson => "exportBoardJson",
            Self::ExportBoardMarkdown => "exportBoardMarkdown",
            Self::SaveEdit => "saveEdit",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::CancelEdit => "cancelEdit",
            Self::ClearSearch => "clearSearch",
            Self::RenameBoard => "renameBoard",
            Self::DeleteBoard => "deleteBoard",
            Self::ManageTags => "manageTags",
            Self::FocusSearch => "focusSearch",
            Self::ToggleBoardList => "toggleBoardList",
            Self::ToggleArchiveView => "toggleArchiveView",
            Self::SetQuickCaptureShortcut => "setQuickCaptureShortcut",
            Self::UseLightTheme => "useLightTheme",
            Self::UseDarkTheme => "useDarkTheme",
            Self::UseSystemTheme => "useSystemTheme",
            Self::BackupDatabase => "backupDatabase",
            Self::RevealDatabase => "revealDatabase",
            Self::RevealBackups => "revealBackups",
            Self::About => "about",
        }
    }
}

impl WindowAction {
    pub fn id(self) -> &'static str {
        match self {
            Self::CloseWindow => "closeWindow",
            Self::ToggleFullscreen => "toggleFullscreen",
            Self::Quit => "quit",
        }
    }
}

/// 引き当てのもとになる一覧。`from_id` と、テストの網羅がここを読みます。
const ACTIONS: &[Action] = &[
    Action::App(AppAction::AddBoard),
    Action::App(AppAction::AddCard),
    Action::App(AppAction::AddColumn),
    Action::App(AppAction::AddTag),
    Action::App(AppAction::ExportBoardJson),
    Action::App(AppAction::ExportBoardMarkdown),
    Action::App(AppAction::SaveEdit),
    Action::App(AppAction::Undo),
    Action::App(AppAction::Redo),
    Action::App(AppAction::CancelEdit),
    Action::App(AppAction::ClearSearch),
    Action::App(AppAction::RenameBoard),
    Action::App(AppAction::DeleteBoard),
    Action::App(AppAction::ManageTags),
    Action::App(AppAction::FocusSearch),
    Action::App(AppAction::ToggleBoardList),
    Action::App(AppAction::ToggleArchiveView),
    Action::App(AppAction::SetQuickCaptureShortcut),
    Action::App(AppAction::UseLightTheme),
    Action::App(AppAction::UseDarkTheme),
    Action::App(AppAction::UseSystemTheme),
    Action::App(AppAction::BackupDatabase),
    Action::App(AppAction::RevealDatabase),
    Action::App(AppAction::RevealBackups),
    Action::App(AppAction::About),
    Action::Window(WindowAction::CloseWindow),
    Action::Window(WindowAction::ToggleFullscreen),
    Action::Window(WindowAction::Quit),
];

/// OS が持っている項目。ここに並ぶものを自分で書き直しません。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predefined {
    Cut,
    Copy,
    Paste,
    SelectAll,
    /// macOS のシステムメニュー。ほかの環境では出しようがない。
    Services,
    Hide,
    HideOthers,
    ShowAll,
    About,
    Minimize,
    Zoom,
    CloseWindow,
    Fullscreen,
    Quit,
}

/// メニュー 1 項目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// 自分で持つ項目。`accelerator` は muda の書き方（`"CmdOrCtrl+N"`）。
    Action {
        action: Action,
        /// 文言。**使えない項目では理由まで入ります**——灰色の項目は押せず、
        /// 理由を出す先が無いためです。
        label: String,
        accelerator: Option<&'static str>,
        /// 押せるか。使えない環境の項目は灰色にして、消しはしません。
        /// 消すと「この機能はこのアプリに無い」に見えます。
        enabled: bool,
    },
    Predefined(Predefined),
    Separator,
}

fn action(action: Action, label: &str, accelerator: Option<&'static str>) -> Item {
    Item::Action {
        action,
        label: label.to_string(),
        accelerator,
        enabled: true,
    }
}

fn app(kind: AppAction, label: &str, accelerator: Option<&'static str>) -> Item {
    action(Action::App(kind), label, accelerator)
}

/// 「クイックキャプチャのショートカット…」。
///
/// 使えない環境では灰色にし、**理由を文言に入れます**。灰色の項目は押せないので、
/// 押したときに理由を出す道がありません。判定は起動中に変わりません。
fn quick_capture_item() -> Item {
    match crate::shortcut::platform_support() {
        Ok(()) => app(
            AppAction::SetQuickCaptureShortcut,
            "クイックキャプチャのショートカット…",
            None,
        ),
        Err(reason) => Item::Action {
            action: Action::App(AppAction::SetQuickCaptureShortcut),
            label: format!("クイックキャプチャのショートカット…（{reason}）"),
            accelerator: None,
            enabled: false,
        },
    }
}

/// メニューバーの 1 つぶん。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: &'static str,
    pub items: Vec<Item>,
}

/// この OS のメニューバー。
///
/// macOS には OS が描くアプリメニューとウインドウメニューがあり、ほかの環境には
/// ありません。そのぶん「終了」と「ekanbanについて」の置き場所が変わります
/// （[ADR 0015]）。どちらも `cfg` を付けずに定義して、テストがどの OS でも
/// 両方を突き合わせられるようにしてあります。
///
/// [ADR 0015]: ../../../docs/adr/0015-a-menu-bar-on-every-platform.md
pub fn sections() -> Vec<Section> {
    if cfg!(target_os = "macos") {
        macos_sections()
    } else {
        drawn_sections()
    }
}

fn macos_sections() -> Vec<Section> {
    vec![
        Section {
            name: "ekanban",
            items: vec![
                Item::Predefined(Predefined::About),
                Item::Separator,
                quick_capture_item(),
                Item::Separator,
                Item::Predefined(Predefined::Services),
                Item::Separator,
                Item::Predefined(Predefined::Hide),
                Item::Predefined(Predefined::HideOthers),
                Item::Predefined(Predefined::ShowAll),
                Item::Separator,
                Item::Predefined(Predefined::Quit),
            ],
        },
        Section {
            name: "ファイル",
            items: vec![
                app(
                    AppAction::AddBoard,
                    "ボードを追加",
                    Some("CmdOrCtrl+Shift+B"),
                ),
                app(AppAction::AddCard, "カードを追加", Some("CmdOrCtrl+N")),
                app(
                    AppAction::AddColumn,
                    "カラムを追加",
                    Some("CmdOrCtrl+Shift+N"),
                ),
                app(AppAction::AddTag, "タグを追加", Some("CmdOrCtrl+Shift+T")),
                Item::Separator,
                app(AppAction::ExportBoardJson, "ボードを書き出す（JSON）", None),
                app(
                    AppAction::ExportBoardMarkdown,
                    "ボードを書き出す（Markdown）",
                    None,
                ),
                Item::Separator,
                app(AppAction::SaveEdit, "保存", Some("CmdOrCtrl+S")),
                Item::Predefined(Predefined::CloseWindow),
            ],
        },
        Section {
            name: "編集",
            items: edit_items(),
        },
        Section {
            name: "ボード",
            items: board_items(),
        },
        Section {
            name: "表示",
            items: view_items(Some("Cmd+Ctrl+S"), Item::Predefined(Predefined::Fullscreen)),
        },
        // macOS の標準の「ウインドウ」メニュー。`Cmd+M` はメニュー項目が
        // あってはじめて効く。
        Section {
            name: "ウインドウ",
            items: vec![
                Item::Predefined(Predefined::Minimize),
                Item::Predefined(Predefined::Zoom),
                Item::Separator,
                Item::Predefined(Predefined::CloseWindow),
            ],
        },
        Section {
            name: "ヘルプ",
            items: vec![
                app(AppAction::BackupDatabase, "データベースをコピー…", None),
                app(
                    AppAction::RevealDatabase,
                    "データベースの場所をFinderで開く",
                    None,
                ),
                app(
                    AppAction::RevealBackups,
                    "バックアップの場所をFinderで開く",
                    None,
                ),
                Item::Separator,
                Item::Predefined(Predefined::About),
            ],
        },
    ]
}

/// macOS 以外のメニューバー。
fn drawn_sections() -> Vec<Section> {
    vec![
        Section {
            name: "ファイル",
            items: vec![
                app(
                    AppAction::AddBoard,
                    "ボードを追加",
                    Some("CmdOrCtrl+Shift+B"),
                ),
                app(AppAction::AddCard, "カードを追加", Some("CmdOrCtrl+N")),
                app(
                    AppAction::AddColumn,
                    "カラムを追加",
                    Some("CmdOrCtrl+Shift+N"),
                ),
                app(AppAction::AddTag, "タグを追加", Some("CmdOrCtrl+Shift+T")),
                Item::Separator,
                app(AppAction::ExportBoardJson, "ボードを書き出す（JSON）", None),
                app(
                    AppAction::ExportBoardMarkdown,
                    "ボードを書き出す（Markdown）",
                    None,
                ),
                Item::Separator,
                app(AppAction::SaveEdit, "保存", Some("CmdOrCtrl+S")),
                action(
                    Action::Window(WindowAction::CloseWindow),
                    "ウインドウを閉じる",
                    Some("CmdOrCtrl+W"),
                ),
                action(
                    Action::Window(WindowAction::Quit),
                    "終了",
                    Some("CmdOrCtrl+Q"),
                ),
            ],
        },
        Section {
            name: "編集",
            items: edit_items(),
        },
        Section {
            name: "ボード",
            items: board_items(),
        },
        Section {
            name: "表示",
            items: view_items(
                Some("CmdOrCtrl+B"),
                action(
                    Action::Window(WindowAction::ToggleFullscreen),
                    "フルスクリーンにする",
                    Some("F11"),
                ),
            ),
        },
        Section {
            name: "ヘルプ",
            items: vec![
                quick_capture_item(),
                Item::Separator,
                app(AppAction::BackupDatabase, "データベースをコピー…", None),
                app(
                    AppAction::RevealDatabase,
                    "データベースの場所をフォルダで開く",
                    None,
                ),
                app(
                    AppAction::RevealBackups,
                    "バックアップの場所をフォルダで開く",
                    None,
                ),
                Item::Separator,
                app(AppAction::About, "ekanbanについて", None),
            ],
        },
    ]
}

/// どの OS でも同じ「編集」メニュー。
///
/// **元に戻す・やり直すにアクセラレータを付けません**（`docs/DESIGN.md`「メニューとキー割り当て」）。付けると、説明欄を
/// 打っている最中の `Cmd+Z` が盤面を巻き戻します。キーは webview が受け、
/// 入力欄にフォーカスがあれば webview 自身の取り消しへ、無ければ盤面の Undo へ
/// 振り分けます。**ここで OS の Undo（[`Predefined::Cut`] などと同じ既定の項目）を
/// 使わないのも同じ理由**で、あれは webview のテキスト編集にしか届きません。
fn edit_items() -> Vec<Item> {
    vec![
        app(AppAction::Undo, "元に戻す", None),
        app(AppAction::Redo, "やり直す", None),
        Item::Separator,
        Item::Predefined(Predefined::Cut),
        Item::Predefined(Predefined::Copy),
        Item::Predefined(Predefined::Paste),
        Item::Predefined(Predefined::SelectAll),
        Item::Separator,
        app(AppAction::CancelEdit, "編集をキャンセル", None),
        app(
            AppAction::ClearSearch,
            "検索をクリア",
            Some("CmdOrCtrl+Shift+F"),
        ),
    ]
}

fn board_items() -> Vec<Item> {
    vec![
        app(AppAction::RenameBoard, "ボード名を変更", None),
        app(AppAction::DeleteBoard, "現在のボードを削除", None),
        Item::Separator,
        app(AppAction::ManageTags, "タグを整理…", None),
    ]
}

/// どの OS でも同じ「表示」メニュー。
///
/// ボード一覧の割り当てだけ OS ごとに違います。macOS は `Cmd+Ctrl+S`、ほかは
/// `Ctrl+B`。フルスクリーンは、macOS では OS の項目（`Cmd+Ctrl+F`）、ほかでは
/// 自分の項目（`F11`）なので、呼ぶ側から渡します。**ここで `cfg!` を見ません**
/// ——見ると、テストがどの OS でも両方のメニューバーを突き合わせられなくなります。
fn view_items(board_list: Option<&'static str>, fullscreen: Item) -> Vec<Item> {
    vec![
        app(
            AppAction::FocusSearch,
            "検索にフォーカス",
            Some("CmdOrCtrl+F"),
        ),
        Item::Separator,
        app(
            AppAction::ToggleBoardList,
            "ボード一覧の表示を切り替え",
            board_list,
        ),
        app(
            AppAction::ToggleArchiveView,
            "アーカイブ表示を切り替え",
            Some("CmdOrCtrl+Shift+A"),
        ),
        Item::Separator,
        app(AppAction::UseLightTheme, "ライトモード", None),
        app(AppAction::UseDarkTheme, "ダークモード", None),
        app(AppAction::UseSystemTheme, "システムに合わせる", None),
        fullscreen,
    ]
}

/// [`sections`] を Tauri のメニューに変換する。
pub fn build<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app_handle)?;
    for section in sections() {
        menu.append(&submenu(app_handle, &section)?)?;
    }
    Ok(menu)
}

fn submenu<R: Runtime>(app_handle: &AppHandle<R>, section: &Section) -> tauri::Result<Submenu<R>> {
    let mut builder = SubmenuBuilder::new(app_handle, section.name);
    for item in &section.items {
        builder = match item {
            Item::Separator => builder.separator(),
            Item::Action {
                action,
                label,
                accelerator,
                enabled,
            } => {
                let mut item = MenuItemBuilder::with_id(action.id(), label).enabled(*enabled);
                if let Some(accelerator) = accelerator {
                    item = item.accelerator(*accelerator);
                }
                builder.item(&item.build(app_handle)?)
            }
            Item::Predefined(predefined) => {
                builder.item(&predefined_item(app_handle, *predefined)?)
            }
        };
    }
    builder.build()
}

fn predefined_item<R: Runtime>(
    app_handle: &AppHandle<R>,
    predefined: Predefined,
) -> tauri::Result<PredefinedMenuItem<R>> {
    match predefined {
        Predefined::Cut => PredefinedMenuItem::cut(app_handle, Some("カット")),
        Predefined::Copy => PredefinedMenuItem::copy(app_handle, Some("コピー")),
        Predefined::Paste => PredefinedMenuItem::paste(app_handle, Some("ペースト")),
        Predefined::SelectAll => PredefinedMenuItem::select_all(app_handle, Some("すべてを選択")),
        Predefined::Services => PredefinedMenuItem::services(app_handle, Some("サービス")),
        Predefined::Hide => PredefinedMenuItem::hide(app_handle, Some("ekanbanを隠す")),
        Predefined::HideOthers => PredefinedMenuItem::hide_others(app_handle, Some("ほかを隠す")),
        Predefined::ShowAll => PredefinedMenuItem::show_all(app_handle, Some("すべてを表示")),
        Predefined::About => PredefinedMenuItem::about(
            app_handle,
            Some("ekanbanについて"),
            Some(AboutMetadata {
                name: Some("ekanban".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                comments: Some("ローカル SQLite で動作する Kanban アプリです。".into()),
                ..Default::default()
            }),
        ),
        Predefined::Minimize => PredefinedMenuItem::minimize(app_handle, Some("しまう")),
        Predefined::Zoom => PredefinedMenuItem::maximize(app_handle, Some("拡大／縮小")),
        Predefined::CloseWindow => {
            PredefinedMenuItem::close_window(app_handle, Some("ウインドウを閉じる"))
        }
        Predefined::Fullscreen => {
            PredefinedMenuItem::fullscreen(app_handle, Some("フルスクリーンにする"))
        }
        Predefined::Quit => PredefinedMenuItem::quit(app_handle, Some("ekanbanを終了")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// macOS のメニューバーにしか出さないもの。
    ///
    /// システムメニューの項目と、macOS のウィンドウ操作です。ほかの環境では
    /// 最小化も最大化もウィンドウマネージャの仕事で、アプリのメニューに出す
    /// 意味がありません。
    const MACOS_ONLY: &[Predefined] = &[
        Predefined::Services,
        Predefined::Hide,
        Predefined::HideOthers,
        Predefined::ShowAll,
        Predefined::Minimize,
        Predefined::Zoom,
    ];

    fn actions_of(sections: &[Section]) -> Vec<Action> {
        sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter_map(|item| match item {
                Item::Action { action, .. } => Some(*action),
                _ => None,
            })
            .collect()
    }

    fn predefined_of(sections: &[Section]) -> Vec<Predefined> {
        sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter_map(|item| match item {
                Item::Predefined(predefined) => Some(*predefined),
                _ => None,
            })
            .collect()
    }

    fn accelerators_of(sections: &[Section]) -> Vec<(&'static str, &'static str)> {
        sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter_map(|item| match item {
                Item::Action {
                    action,
                    accelerator: Some(accelerator),
                    ..
                } => Some((action.id(), *accelerator)),
                _ => None,
            })
            .collect()
    }

    /// **画面が引き受ける操作は、どちらのメニューバーにも出ていること。**
    ///
    /// 足したのに並べ忘れると、dispatcher にだけ手が入って、押す道がどこにも
    /// 無い操作が残ります（実際に「アーカイブ表示を切り替え」でそうなりました）。
    #[test]
    fn puts_every_app_action_on_both_menu_bars() {
        for (bar, sections) in [("macOS", macos_sections()), ("drawn", drawn_sections())] {
            let on_the_bar = actions_of(&sections);
            for action in ACTIONS {
                let Action::App(app_action) = action else {
                    continue;
                };
                // 「ekanbanについて」だけは macOS では OS の項目が出す。
                if *app_action == AppAction::About && bar == "macOS" {
                    continue;
                }
                assert!(
                    on_the_bar.contains(action),
                    "{} is not on the {bar} menu bar",
                    app_action.id()
                );
            }
        }
    }

    /// ウィンドウの操作は、どちらかのメニューバーから届くこと。
    ///
    /// macOS では OS の項目（閉じる・終了・フルスクリーン）が持つので、自前の
    /// 項目は macOS 以外にだけ出ます。
    #[test]
    fn reaches_every_window_action_from_the_drawn_menu_bar() {
        let drawn = actions_of(&drawn_sections());
        for action in ACTIONS {
            if let Action::Window(_) = action {
                assert!(drawn.contains(action), "{} has no menu item", action.id());
            }
        }
    }

    /// 押された id から引き当てられること。`from_id` が読む一覧に漏れがあると、
    /// メニューを押しても黙って何も起きない。
    #[test]
    fn finds_every_action_the_menu_bars_carry() {
        for sections in [macos_sections(), drawn_sections()] {
            for action in actions_of(&sections) {
                assert_eq!(
                    Action::from_id(action.id()),
                    Some(action),
                    "{} is on a menu bar but not in ACTIONS",
                    action.id()
                );
            }
        }
    }

    /// id は `ts-rs` が書き出す名前と同じでなければならない。ずれると、webview の
    /// dispatcher が受け取れない id が飛ぶ。
    #[test]
    fn names_each_action_the_way_the_webview_sees_it() {
        for action in ACTIONS {
            let Action::App(app_action) = action else {
                continue;
            };
            let serialized = serde_json::to_string(app_action).expect("an action serializes");
            assert_eq!(
                serialized,
                format!("\"{}\"", app_action.id()),
                "the id and the serialized name differ"
            );
        }
    }

    /// 受け入れ条件「macOS でたどれる操作は Linux・Windows でもたどれる」（#79）。
    #[test]
    fn offers_every_macos_action_on_the_other_platforms_too() {
        let drawn = actions_of(&drawn_sections());
        let missing = actions_of(&macos_sections())
            .into_iter()
            .filter(|action| !drawn.contains(action))
            .collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "these actions are on the macOS menu bar but nowhere on the drawn one: {missing:?}"
        );

        // 「ekanbanについて」は macOS では OS の項目、ほかでは自前の項目。
        // どちらの経路でも届くことを見る。
        assert!(
            predefined_of(&macos_sections()).contains(&Predefined::About),
            "macOS shows the About item the system draws"
        );
        assert!(
            drawn.contains(&Action::App(AppAction::About)),
            "the drawn menu bar has to draw About itself"
        );
    }

    /// 受け入れ条件「Linux・Windows のメニューに macOS 専用の項目が入らない」（#79）。
    #[test]
    fn keeps_the_macos_system_commands_off_the_drawn_menu_bar() {
        let on_macos = predefined_of(&macos_sections());
        let drawn = predefined_of(&drawn_sections());
        for predefined in MACOS_ONLY {
            assert!(
                on_macos.contains(predefined),
                "{predefined:?} belongs on the macOS menu bar"
            );
            assert!(
                !drawn.contains(predefined),
                "{predefined:?} is a macOS-only command and does not belong on the drawn menu bar"
            );
        }
    }

    /// 終了・フルスクリーン・ボード一覧には、どの OS でも届く手段がある（#53）。
    #[test]
    fn reaches_quit_fullscreen_and_the_board_list_on_every_platform() {
        let macos_predefined = predefined_of(&macos_sections());
        assert!(macos_predefined.contains(&Predefined::Quit));
        assert!(macos_predefined.contains(&Predefined::Fullscreen));

        let drawn = actions_of(&drawn_sections());
        assert!(drawn.contains(&Action::Window(WindowAction::Quit)));
        assert!(drawn.contains(&Action::Window(WindowAction::ToggleFullscreen)));

        for sections in [macos_sections(), drawn_sections()] {
            assert!(
                actions_of(&sections).contains(&Action::App(AppAction::ToggleBoardList)),
                "the board list has no menu item on one of the menu bars"
            );
        }
    }

    /// 入力中の `Cmd+Z` が盤面を巻き戻さないこと（`docs/DESIGN.md`「メニューとキー割り当て」）。
    ///
    /// アクセラレータを付けた時点で、入力欄にフォーカスがあっても先に取られる。
    /// キーを webview で受けて振り分けるという決めごとは、**ここに割り当てを
    /// 書かないこと**で守られる。
    #[test]
    fn leaves_undo_and_redo_without_an_accelerator() {
        for sections in [macos_sections(), drawn_sections()] {
            for (id, accelerator) in accelerators_of(&sections) {
                assert!(
                    id != AppAction::Undo.id() && id != AppAction::Redo.id(),
                    "{id} must not carry {accelerator}"
                );
            }
        }
    }

    #[test]
    fn assigns_each_combination_to_a_single_action() {
        for sections in [macos_sections(), drawn_sections()] {
            let mut combinations = accelerators_of(&sections)
                .into_iter()
                .map(|(_, accelerator)| accelerator)
                .collect::<Vec<_>>();
            let before = combinations.len();
            combinations.sort_unstable();
            combinations.dedup();
            assert_eq!(
                before,
                combinations.len(),
                "two actions share a combination"
            );
        }
    }

    /// 割り当ての書き方が muda の読める形であること。
    ///
    /// 読めない文字列はメニューを組む時点で `Err` になり、**窓が開かないまま
    /// 終わります**。実際に組むにはアプリのハンドルが要るので、ここでは表の側を
    /// 見ます。
    #[test]
    fn writes_every_accelerator_the_way_muda_reads_them() {
        const MODIFIERS: &[&str] = &["CmdOrCtrl", "Cmd", "Ctrl", "Alt", "Shift"];
        for sections in [macos_sections(), drawn_sections()] {
            for (id, accelerator) in accelerators_of(&sections) {
                let mut parts = accelerator.split('+').collect::<Vec<_>>();
                let key = parts.pop().expect("split always yields one part");
                for modifier in &parts {
                    assert!(
                        MODIFIERS.contains(modifier),
                        "{id} carries an unknown modifier {modifier}"
                    );
                }
                let known_key = key.len() == 1 && key.chars().all(|c| c.is_ascii_uppercase())
                    || key.strip_prefix('F').is_some_and(|number| {
                        number
                            .parse::<u8>()
                            .is_ok_and(|number| (1..=24).contains(&number))
                    });
                assert!(known_key, "{id} carries an unknown key {key}");
            }
        }
    }

    /// 1 つのメニューの中に同じ操作が二度出ていないか。
    ///
    /// メニューをまたぐ重なりは数えない。macOS では「ウインドウを閉じる」が
    /// ファイル と ウインドウ に、「ekanbanについて」が ekanban と ヘルプ に
    /// 出るのが作法どおり。
    #[test]
    fn keeps_each_menu_free_of_duplicates() {
        for (bar, sections) in [("macOS", macos_sections()), ("drawn", drawn_sections())] {
            for section in sections {
                let name = section.name;
                let one = vec![section];
                let mut ids = actions_of(&one)
                    .into_iter()
                    .map(|action| action.id().to_string())
                    .collect::<Vec<_>>();
                ids.extend(
                    predefined_of(&one)
                        .into_iter()
                        .map(|predefined| format!("{predefined:?}")),
                );
                let before = ids.len();
                ids.sort_unstable();
                ids.dedup();
                assert_eq!(
                    before,
                    ids.len(),
                    "the {name} menu on the {bar} menu bar lists an action twice"
                );
            }
        }
    }
}
