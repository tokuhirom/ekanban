// メニューバーの構成（[ADR 0015]、[ADR 0043]）。
//
// **何がどう並ぶかは画面の話です。** 押されたときに何が起きるかは既にここに
// あり（`shell/actions.ts` の dispatcher）、並びだけが向こうにあると、項目を
// 1 つ足すのに 2 か所を触ることになります。描く相手が OS であることは、構成を
// Rust に置く理由になりません（[ADR 0039]）。
//
// 起動の最初に `setMenu` で殻へ渡すと、殻はこれを Tauri のメニューに変換して
// 掛けます。押されたら `app:action` で戻ってきて、dispatcher が実行します。
// ブラウザだけで動く組み立てでは、同じ構成を `MenuBar` がページに描きます。
//
// **`cfg` にあたるものを途中に置きません。** どの OS かは引数なので、どちらの
// 並びもいつでも組み立てられます——テストがどの環境でも両方を突き合わせられる
// のはそのためです。
//
// [ADR 0015]: ../../../docs/adr/0015-a-menu-bar-on-every-platform.md
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
// [ADR 0043]: ../../../docs/adr/0043-the-menu-is-described-by-the-webview.md

import type { AppAction } from "../ipc/types/AppAction";
import type { Item } from "../ipc/types/Item";
import type { Platform } from "../ipc/types/Platform";
import type { Predefined } from "../ipc/types/Predefined";
import type { Section } from "../ipc/types/Section";
import type { WindowAction } from "../ipc/types/WindowAction";

export type { Item, Predefined, Section, WindowAction };

// 形は `ts-rs` が Rust から書き出したものを使います（`ipc/types/`、
// `docs/DESIGN.md`「境界を越える値」）。**構成を決めるのはこちら**ですが、
// 渡す形は境界を越えるので、手で 2 か所に書きません。
const SEPARATOR: Item = { kind: "separator" };

function app(action: AppAction, label: string, accelerator: string | null = null): Item {
  return { kind: "app", action, label, accelerator, enabled: true };
}

function windowItem(
  action: WindowAction,
  label: string,
  accelerator: string | null = null,
): Item {
  return { kind: "window", action, label, accelerator };
}

function predefined(item: Predefined): Item {
  return { kind: "predefined", item };
}

/// 「設定…」。**アプリ全体の設定への入り口はこれ 1 つ**です（ADR 0047）。
///
/// 置き場所は OS の慣習に従います——macOS はアプリメニュー、ほかは「ファイル」の
/// 末尾（閉じる・終了の上）。割り当ては `CmdOrCtrl+,` で、どちらでも同じ。
///
/// **使えない環境の理由はここに入りません。** グローバルホットキーを作れない
/// 環境でも、テーマは選べます。理由を出すのは設定画面の割り当ての欄で、
/// そこだけが灰色になります（`shell/SettingsDialog.tsx`）。
function settings(): Item {
  return app("openSettings", "設定…", "CmdOrCtrl+,");
}

/// どの OS でも同じ「編集」メニュー。
///
/// **元に戻す・やり直すにアクセラレータを付けません**（`docs/DESIGN.md`
/// 「メニューとキー割り当て」）。付けると、説明欄を打っている最中の `Cmd+Z` が
/// 盤面を巻き戻します。キーは webview が受け、入力欄にフォーカスがあれば
/// webview 自身の取り消しへ、無ければ盤面の Undo へ振り分けます。**ここで OS の
/// Undo（`predefined` の項目）を使わないのも同じ理由**で、あれは webview の
/// テキスト編集にしか届きません。
function editItems(): Item[] {
  return [
    app("undo", "元に戻す"),
    app("redo", "やり直す"),
    SEPARATOR,
    predefined("cut"),
    predefined("copy"),
    predefined("paste"),
    predefined("selectAll"),
    SEPARATOR,
    app("cancelEdit", "編集をキャンセル"),
    app("clearSearch", "検索をクリア", "CmdOrCtrl+Shift+F"),
  ];
}

function boardItems(): Item[] {
  return [
    app("renameBoard", "ボード名を変更"),
    app("deleteBoard", "現在のボードを削除"),
    SEPARATOR,
    app("manageTags", "タグを整理…"),
  ];
}

/// どの OS でも同じ「表示」メニュー。
///
/// ボード一覧の割り当てだけ OS ごとに違います。macOS は `Cmd+Ctrl+S`、ほかは
/// `Ctrl+B`。フルスクリーンは、macOS では OS の項目（`Cmd+Ctrl+F`）、ほかでは
/// 自分の項目（`F11`）なので、呼ぶ側から渡します。
function viewItems(boardList: string | null, fullscreen: Item): Item[] {
  return [
    app("focusSearch", "検索にフォーカス", "CmdOrCtrl+F"),
    SEPARATOR,
    app("toggleBoardList", "ボード一覧の表示を切り替え", boardList),
    app("toggleArchiveView", "アーカイブ表示を切り替え", "CmdOrCtrl+Shift+A"),
    SEPARATOR,
    app("useLightTheme", "ライトモード"),
    app("useDarkTheme", "ダークモード"),
    app("useSystemTheme", "システムに合わせる"),
    fullscreen,
  ];
}

/// 新しいカードとカラムを足す口。どの OS でも同じ並び。
function fileHead(): Item[] {
  return [
    app("addBoard", "ボードを追加", "CmdOrCtrl+Shift+B"),
    app("addCard", "カードを追加", "CmdOrCtrl+N"),
    app("addColumn", "カラムを追加", "CmdOrCtrl+Shift+N"),
    app("addTag", "タグを追加", "CmdOrCtrl+Shift+T"),
    SEPARATOR,
    app("exportBoardJson", "ボードを書き出す（JSON）"),
    app("exportBoardMarkdown", "ボードを書き出す（Markdown）"),
    SEPARATOR,
    app("saveEdit", "保存", "CmdOrCtrl+S"),
  ];
}

function macosSections(): Section[] {
  return [
    {
      name: "ekanban",
      items: [
        predefined("about"),
        SEPARATOR,
        settings(),
        SEPARATOR,
        predefined("services"),
        SEPARATOR,
        predefined("hide"),
        predefined("hideOthers"),
        predefined("showAll"),
        SEPARATOR,
        predefined("quit"),
      ],
    },
    { name: "ファイル", items: [...fileHead(), predefined("closeWindow")] },
    { name: "編集", items: editItems() },
    { name: "ボード", items: boardItems() },
    { name: "表示", items: viewItems("Cmd+Ctrl+S", predefined("fullscreen")) },
    // macOS の標準の「ウインドウ」メニュー。`Cmd+M` はメニュー項目が
    // あってはじめて効く。
    {
      name: "ウインドウ",
      items: [
        predefined("minimize"),
        predefined("zoom"),
        SEPARATOR,
        predefined("closeWindow"),
      ],
    },
    {
      name: "ヘルプ",
      items: [
        app("backupDatabase", "データベースをコピー…"),
        app("revealDatabase", "データベースの場所をFinderで開く"),
        app("revealBackups", "バックアップの場所をFinderで開く"),
        SEPARATOR,
        predefined("about"),
      ],
    },
  ];
}

/// macOS 以外のメニューバー。
function drawnSections(): Section[] {
  return [
    {
      name: "ファイル",
      items: [
        ...fileHead(),
        SEPARATOR,
        settings(),
        SEPARATOR,
        windowItem("closeWindow", "ウインドウを閉じる", "CmdOrCtrl+W"),
        windowItem("quit", "終了", "CmdOrCtrl+Q"),
      ],
    },
    { name: "編集", items: editItems() },
    { name: "ボード", items: boardItems() },
    {
      name: "表示",
      items: viewItems(
        "CmdOrCtrl+B",
        windowItem("toggleFullscreen", "フルスクリーンにする", "F11"),
      ),
    },
    {
      name: "ヘルプ",
      items: [
        app("backupDatabase", "データベースをコピー…"),
        app("revealDatabase", "データベースの場所をフォルダで開く"),
        app("revealBackups", "バックアップの場所をフォルダで開く"),
        SEPARATOR,
        app("about", "ekanban について"),
      ],
    },
  ];
}

/// この OS のメニューバー。
///
/// macOS には OS が描くアプリメニューとウインドウメニューがあり、ほかの環境には
/// ありません。そのぶん「終了」と「ekanban について」の置き場所が変わります
/// （[ADR 0015]）。**「設定…」の置き場所もここで変わります**——macOS は
/// アプリメニュー、ほかは「ファイル」の末尾（ADR 0047）。
export function sectionsFor(platform: Platform): Section[] {
  return platform === "macos" ? macosSections() : drawnSections();
}

/// ページが描くメニューバー（[ADR 0035]）。
///
/// [`sectionsFor`] から、**ブラウザに持っていけないものを落としただけ**の
/// ものです。メニューの構成を 2 か所に書かないための形で、ここに項目を
/// 足しません——足すと、殻のメニューに無いものがブラウザ版にだけ出ます。
///
/// 落とすのは 2 種類です。
///
/// - `predefined`。カット・コピー・ペーストも、隠す・終了も OS のもので、
///   ブラウザの中に相手がいません。テキスト編集はブラウザ自身が持っています
/// - `window`。閉じる・全画面・終了はウィンドウそのものの操作で、ページには
///   手が届きません
///
/// 落とした結果として区切り線が続いたり、端に残ったりするので、そこも
/// ならします。**描く側で「前が区切り線だったか」を数えさせません。**
///
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
export function webSections(platform: Platform): Section[] {
  return sectionsFor(platform)
    .map((section) => ({
      name: section.name,
      items: tidySeparators(
        section.items
          .filter((item) => item.kind === "app" || item.kind === "separator")
          .map((item) => (item.kind === "app" ? browserAvailability(item) : item)),
      ),
    }))
    .filter((section) => section.items.length > 0);
}

/// ブラウザに相手がいない項目を、**灰色にして理由を文言に入れる**。
///
/// 消しません。消すと「この機能はこのアプリに無い」に見えます。灰色の項目は
/// 押せず、押せない以上理由を出す先が無いので、理由のほうを文言に入れます。
///
/// ファイル管理でフォルダを開くのと、データベースの控えがそれです。前者は
/// ブラウザから OS のファイル管理を呼べないため、後者は**ブラウザ版に
/// SQLite のファイルがそもそも無い**ためです（[ADR 0036]）。**盤面の持ち出しは
/// 残ります**——「ボードを書き出す」の 2 つがダウンロードになります。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
function browserAvailability(item: Extract<Item, { kind: "app" }>): Item {
  const unavailable =
    item.action === "revealDatabase" ||
    item.action === "revealBackups" ||
    item.action === "backupDatabase";
  return {
    ...item,
    label: unavailable ? `${item.label}（ブラウザでは使えません）` : item.label,
    enabled: item.enabled && !unavailable,
  };
}

/// 端の区切り線と、続いた区切り線を落とす。
function tidySeparators(items: Item[]): Item[] {
  const tidied: Item[] = [];
  for (const item of items) {
    const previous = tidied.at(-1);
    if (item.kind === "separator" && (previous === undefined || previous.kind === "separator")) {
      continue;
    }
    tidied.push(item);
  }
  if (tidied.at(-1)?.kind === "separator") tidied.pop();
  return tidied;
}
