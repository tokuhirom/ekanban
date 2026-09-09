//! メニューバーと、そこに付くキーの割り当て（`docs/DESIGN.md`「メニューとキー割り当て」）。
//!
//! **構成はここにありません**（[ADR 0043]）。何がどう並ぶかを決めるのは
//! `web/src/shell/menu.ts` で、webview が起動の最初に `set_menu` で渡します。
//! ここに残っているのは、受け取った構成を Tauri のメニューに変換するところと、
//! 押された id の引き当てです——ウェブアプリなら画面の中に描いていたものを、
//! ここでは OS に描かせているだけなので、構成をサーバ側に置く理由がありません
//! （[ADR 0039]）。
//!
//! [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
//! [ADR 0043]: ../../../docs/adr/0043-the-menu-is-described-by-the-webview.md
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

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::menu::{
    AboutMetadata, Menu, MenuItemBuilder, MenuItemKind, PredefinedMenuItem, Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Runtime};
use ts_rs::TS;

/// webview が受け取るメニューの操作。`app:action` の積荷です。
///
/// 名前は TypeScript 側と 1 対 1 で、`ts-rs` が書き出します。**手で 2 か所に
/// 書きません**（`docs/DESIGN.md`「コマンドとイベント」）。dispatcher が網羅しているかどうかは、この型から作った
/// `Record` を TypeScript の型検査が見ます。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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
///
/// 名前は `web/src/shell/menu.ts` の `Predefined` と 1 対 1 です。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
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

/// メニュー 1 項目。**webview から届く形**（[ADR 0043]）。
///
/// [ADR 0043]: ../../../docs/adr/0043-the-menu-is-described-by-the-webview.md
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Item {
    /// webview へ流す項目。押されたら `app:action` で戻します。
    #[serde(rename_all = "camelCase")]
    App {
        action: AppAction,
        /// 文言。**使えない項目では理由まで入ります**——灰色の項目は押せず、
        /// 理由を出す先が無いためです。
        label: String,
        /// muda の書き方（`"CmdOrCtrl+N"`）。
        accelerator: Option<String>,
        /// 押せるか。使えない環境の項目は灰色にして、消しはしません。
        enabled: bool,
    },
    /// 殻が自分で行う項目。ウィンドウそのものの操作で、webview に手が届きません。
    #[serde(rename_all = "camelCase")]
    Window {
        action: WindowAction,
        label: String,
        accelerator: Option<String>,
    },
    /// OS が持っている項目。
    #[serde(rename_all = "camelCase")]
    Predefined {
        item: Predefined,
    },
    Separator,
}

/// メニューバーの 1 つぶん。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Section {
    pub name: String,
    pub items: Vec<Item>,
}

/// webview が渡した構成を Tauri のメニューに変換する。
pub fn build<R: Runtime>(
    app_handle: &AppHandle<R>,
    sections: &[Section],
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app_handle)?;
    for section in sections {
        menu.append(&submenu(app_handle, section)?)?;
    }
    Ok(menu)
}

/// いま掛かっているメニューの構成。
///
/// **アクセラレータを付け直すのに要ります**（[ADR 0030]）。割り当てを捕まえて
/// いる間は外し、終わったら戻す——戻す形を控えから作らず、掛けたときの構成から
/// 組み直します。
///
/// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md
#[derive(Default)]
pub struct CurrentMenu(std::sync::Mutex<Vec<Section>>);

impl CurrentMenu {
    pub fn set(&self, sections: Vec<Section>) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = sections;
    }

    pub fn get(&self) -> Vec<Section> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// webview が繋がるまで置いておくメニュー（[ADR 0043]）。
///
/// **macOS だけ中身があります。** アプリメニューは OS が要求するもので、無いと
/// メニューバーそのものが出ません。ほかの環境は空のバーで、webview が
/// `set_menu` を呼んだ時点で本物に差し替わります。
///
/// [ADR 0043]: ../../../docs/adr/0043-the-menu-is-described-by-the-webview.md
pub fn placeholder() -> Vec<Section> {
    if !cfg!(target_os = "macos") {
        return Vec::new();
    }
    vec![Section {
        name: "ekanban".to_string(),
        items: vec![
            Item::Predefined {
                item: Predefined::About,
            },
            Item::Separator,
            Item::Predefined {
                item: Predefined::Quit,
            },
        ],
    }]
}

/// メニューに付いているキーの割り当てを、付け外しする。
///
/// **クイックキャプチャの割り当てを捕まえている間は外します**（`docs/DESIGN.md`
/// 「クイックキャプチャ」、[ADR 0030]）。メニューのアクセラレータは webview より
/// 先に押されたキーを取るので、付いたままだと `Cmd+N` のような組み合わせが
/// `keydown` として画面に届かず、**押しても何も起きないダイアログ**になります。
///
/// 触るのは自分で持っている項目だけです。[`Predefined`] の項目（`Cmd+Q`、
/// `Cmd+W`、`Cmd+C` など）は OS のもので、そこに割り当てるものでもありません。
///
/// [ADR 0030]: ../../../docs/adr/0030-capturing-a-shortcut-needs-the-menu-out-of-the-way.md
pub fn set_accelerators_active<R: Runtime>(
    app: &AppHandle<R>,
    sections: &[Section],
    active: bool,
) -> tauri::Result<()> {
    let Some(menu) = app.menu() else {
        // メニューを組んでいなければ、外すものも戻すものもない。
        return Ok(());
    };
    let bindings = bindings(sections);
    apply_bindings(&menu.items()?, active.then_some(&bindings))
}

/// メニューの項目が、押されたキーをどう取るか。
struct Binding {
    accelerator: Option<String>,
    enabled: bool,
}

/// 渡された構成が決めている、項目ごとの割り当てと押せるかどうか。
///
/// 付け直す先をここから作るので、**外したあとに戻す形は組み立てたときと同じ**
/// です。控えを持ち回すと、控えを取り損ねた経路が 1 つでもあれば割り当てが
/// 消えたままになります。`enabled` も同じで、もともと灰色だった項目（使えない
/// 環境のクイックキャプチャ）が戻すときに押せるようになりません。
fn bindings(sections: &[Section]) -> HashMap<String, Binding> {
    sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter_map(|item| match item {
            Item::App {
                action,
                accelerator,
                enabled,
                ..
            } => Some((
                action.id().to_string(),
                Binding {
                    accelerator: accelerator.clone(),
                    enabled: *enabled,
                },
            )),
            Item::Window {
                action,
                accelerator,
                ..
            } => Some((
                action.id().to_string(),
                Binding {
                    accelerator: accelerator.clone(),
                    enabled: true,
                },
            )),
            _ => None,
        })
        .collect()
}

/// `wanted` が `None` なら外し、`Some` ならその形に戻す。
///
/// **外すのに 2 つ必要です。** muda はアクセラレータを外せる環境と外せない環境が
/// あり（macOS の `set_key_accelerator(None)` は `NSMenuItem` に触りません）、
/// そこは無効な項目が key equivalent を実行しないことで止まります。GTK と
/// Windows はアクセラレータの側が実際に外れます。片方だけでは、どちらかの環境で
/// キーがダイアログに届きません。
fn apply_bindings<R: Runtime>(
    items: &[MenuItemKind<R>],
    wanted: Option<&HashMap<String, Binding>>,
) -> tauri::Result<()> {
    for item in items {
        match item {
            MenuItemKind::Submenu(submenu) => apply_bindings(&submenu.items()?, wanted)?,
            MenuItemKind::MenuItem(entry) => match wanted {
                Some(bindings) => {
                    // 知らない id は触りません。組み立てていない項目の押せる／
                    // 押せないを、ここが勝手に決める理由がありません。
                    if let Some(binding) = bindings.get(entry.id().as_ref() as &str) {
                        entry.set_accelerator(binding.accelerator.as_deref())?;
                        entry.set_enabled(binding.enabled)?;
                    }
                }
                None => {
                    entry.set_accelerator(None::<&str>)?;
                    entry.set_enabled(false)?;
                }
            },
            _ => {}
        }
    }
    Ok(())
}

fn submenu<R: Runtime>(app_handle: &AppHandle<R>, section: &Section) -> tauri::Result<Submenu<R>> {
    let mut builder = SubmenuBuilder::new(app_handle, &section.name);
    for item in &section.items {
        builder = match item {
            Item::Separator => builder.separator(),
            Item::App {
                action,
                label,
                accelerator,
                enabled,
            } => builder.item(&entry(
                app_handle,
                action.id(),
                label,
                accelerator,
                *enabled,
            )?),
            Item::Window {
                action,
                label,
                accelerator,
            } => builder.item(&entry(app_handle, action.id(), label, accelerator, true)?),
            Item::Predefined { item } => builder.item(&predefined_item(app_handle, *item)?),
        };
    }
    builder.build()
}

fn entry<R: Runtime>(
    app_handle: &AppHandle<R>,
    id: &str,
    label: &str,
    accelerator: &Option<String>,
    enabled: bool,
) -> tauri::Result<tauri::menu::MenuItem<R>> {
    let mut item = MenuItemBuilder::with_id(id, label).enabled(enabled);
    if let Some(accelerator) = accelerator {
        item = item.accelerator(accelerator);
    }
    item.build(app_handle)
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
            Some("ekanban について"),
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

    /// 押された id から引き当てられること。`from_id` が読む一覧に漏れがあると、
    /// メニューを押しても黙って何も起きない。
    #[test]
    fn finds_every_action_by_its_id() {
        for action in ACTIONS {
            assert_eq!(
                Action::from_id(action.id()),
                Some(*action),
                "{} is in ACTIONS but cannot be looked up",
                action.id()
            );
        }
    }

    /// id は境界を越える名前と同じでなければならない。
    ///
    /// ずれると、**webview が組んだ構成の名前を殻が読めません**（`set_menu` は
    /// この名前で受けます）し、押されたときに飛ぶ id も dispatcher に届きません。
    #[test]
    fn names_each_action_the_way_the_webview_sees_it() {
        for action in ACTIONS {
            let (id, serialized) = match action {
                Action::App(app) => (
                    app.id(),
                    serde_json::to_string(app).expect("an action serializes"),
                ),
                Action::Window(window) => (
                    window.id(),
                    serde_json::to_string(window).expect("an action serializes"),
                ),
            };
            assert_eq!(serialized, format!("\"{id}\""));
        }
    }

    /// webview が組んだ構成を、そのまま読めること。
    ///
    /// **形が食い違うとメニューが掛かりません。** 名前は
    /// `web/src/shell/menu.ts` の `Item` と 1 対 1 です。
    #[test]
    fn reads_the_shape_the_webview_sends() {
        let sections: Vec<Section> = serde_json::from_str(
            r#"[{
                "name": "ファイル",
                "items": [
                    {"kind": "app", "action": "addCard", "label": "カードを追加",
                     "accelerator": "CmdOrCtrl+N", "enabled": true},
                    {"kind": "separator"},
                    {"kind": "window", "action": "quit", "label": "終了",
                     "accelerator": "CmdOrCtrl+Q"},
                    {"kind": "predefined", "item": "closeWindow"}
                ]
            }]"#,
        )
        .expect("the webview's shape reads back");

        assert_eq!(sections.len(), 1);
        assert_eq!(
            sections[0].items[0],
            Item::App {
                action: AppAction::AddCard,
                label: "カードを追加".to_string(),
                accelerator: Some("CmdOrCtrl+N".to_string()),
                enabled: true,
            }
        );
        assert_eq!(sections[0].items[1], Item::Separator);
        assert_eq!(
            sections[0].items[2],
            Item::Window {
                action: WindowAction::Quit,
                label: "終了".to_string(),
                accelerator: Some("CmdOrCtrl+Q".to_string()),
            }
        );
        assert_eq!(
            sections[0].items[3],
            Item::Predefined {
                item: Predefined::CloseWindow
            }
        );
    }
}
