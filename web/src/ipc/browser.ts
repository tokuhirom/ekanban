// 手元の置き場所を相手にする口（[ADR 0041]、[ADR 0042]）。
//
// **ブラウザで動くときは、これが唯一の実装です。** 相手は
// `web/src/store/memory.ts`——配るアプリで SQLite が引き受けているものの
// TypeScript 版で、盤面のコードはアプリと 1 文字も違いません。違うのは差した
// 置き場所と、環境に無いものの扱いだけです。
//
// - **保存ダイアログがありません。** 名前を決めて、そのままダウンロードにします
// - **ファイル管理を開けません。** `canRevealPaths` が `false` なので、書き出しの
//   知らせに「場所を開く」が出ません
// - **ページの外まで届くキーの割り当てを作れません。** 理由を返して、割り当ての
//   ダイアログが「使えない」と出せるようにします
// - **メニューバーは OS ではなくページが描きます**（`shell/MenuBar.tsx`）。
//   `onAppAction` は口を開けるだけで、押したことを配るのはページです
// - **ほかの窓が書いたことは `storage` の出来事で届きます。** 配るアプリで
//   Rust が `board:changed` を投げるのに当たるもので、同じ生まれのページが
//   置き場所を書き換えるとブラウザが教えてくれます
//
// [ADR 0041]: ../../../docs/adr/0041-one-layer-of-screen-tests.md
// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

import type { MemoryStore } from "../store/memory";
import * as keys from "../store/keys";
import type { Ipc } from "./index";
import type { AppAction } from "./types/AppAction";
import type { CaptureTarget } from "./types/CaptureTarget";
import type { Platform } from "./types/Platform";
import type { StartupState } from "./types/StartupState";
import type { ThemePreference } from "./types/ThemePreference";
import type { WindowBoundsState } from "./types/WindowBoundsState";

/// この置き場所がどこにあるか。画面（「ekanban について」）に出す文言。
export interface BrowserIpcOptions {
  /// 動いている OS。**ページが名乗ります**——ブラウザに「どの OS 向けの
  /// ビルドか」は無いので、そこだけは受け取る側になります（ADR 0035）。
  platform: Platform;
  /// 盤面がどこにあるか。パスではありません。
  place: string;
  /// 何かする前に呼ぶ。置いてあるものを取り直すのはここ。**置き場所は
  /// 窓ごとの写しではありません**——同じ生まれのページが同じものを見ます。
  refresh?: () => void;
  /// 書いたあとに呼ぶ。`localStorage` へ写すのはここ。**置き場所は自分が
  /// どこに置かれるかを知りません**（ADR 0036）。
  persist?: () => void;
  /// ファイルを 1 つ受け取らせる。無ければ何も起きません。
  download?: (fileName: string, contents: string, type: string) => void;
  /// この環境でグローバルホットキーを使えない理由。使えるなら `null`。
  shortcutUnavailable?: string | null;
  /// ほかの窓が置き場所を書き換えたら呼ぶ。返すのは購読をやめる関数。
  watch?: (handler: () => void) => () => void;
}

declare global {
  interface Window {
    /// メニューが押されたことにする口。
    ///
    /// **この組み立てではメニューをページが持ちます**（`shell/MenuBar.tsx`）。
    /// 押されたことを入れる口もページ側にあり、デモのメニューバーと画面の
    /// テストが同じここを通ります。
    ekanbanMenu?: (action: AppAction) => void;
  }
}

/// メニューを押したことを配る先。ページが `fireAppAction` で呼びます。
let menuHandler: ((action: AppAction) => void) | null = null;

/// ページのメニューバーから呼ぶ。購読していなければ何も起きません。
export function fireAppAction(action: AppAction): void {
  menuHandler?.(action);
}

/// 拡張子から、ダウンロードに付ける種類を決める。
///
/// 知らない拡張子はただの文字列として渡します。中身が読めないより、種類が
/// 素っ気ないほうが困りません。
function contentType(extension: string): string {
  if (extension === "json") return "application/json";
  if (extension === "md") return "text/markdown";
  return "text/plain";
}

export function browserIpc(store: MemoryStore, options: BrowserIpcOptions): Ipc {
  const refresh = options.refresh ?? (() => undefined);
  const persist = options.persist ?? (() => undefined);
  const download = options.download ?? (() => undefined);

  // 置き場所は同期です。**それでも Promise で返します**——`Ipc` の形を揃えて
  // おかないと、画面の側に「ここだけ同期」という分岐が生まれます。投げた失敗を
  // reject に変えるのもここで、Tauri の `invoke` と同じ受け方になります。
  // **どちらも、置いてあるものを取り直してから始めます。** ほかの窓が書いて
  // いることがあるので、開いたときの写しで判断すると、版が合っているのに
  // 中身が古い、が起きます（[ADR 0040]）。
  //
  // [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
  const read = async <T>(act: () => T): Promise<T> =>
    Promise.resolve().then(() => {
      refresh();
      return act();
    });
  const write = async <T>(act: () => T): Promise<T> =>
    Promise.resolve().then(() => {
      refresh();
      const answer = act();
      persist();
      return answer;
    });

  return {
    startupState: () =>
      write((): StartupState => {
        store.seedIfEmpty();
        const documents = store.loadDocuments();
        const last = Number(store.get(keys.LAST_BOARD) ?? Number.NaN);
        const openBoardId =
          documents.some((document) => document.board.id === last)
            ? last
            : (documents[0]?.board.id ?? 0);
        store.set(keys.LAST_BOARD, String(openBoardId));
        const tagId = store.get(keys.FILTER_TAG);
        const bounds = store.get(keys.WINDOW_BOUNDS);
        return {
          openBoardId,
          platform: options.platform,
          filter: {
            search: store.get(keys.FILTER_SEARCH) ?? "",
            tagId: tagId === null ? null : Number(tagId),
          },
          windowBounds: bounds === null ? null : (JSON.parse(bounds) as WindowBoundsState),
          theme: readTheme(store.get(keys.THEME_PREFERENCE)),
          sidebarCollapsed: store.get(keys.SIDEBAR_COLLAPSED) === "true",
          captureTarget: readCaptureTarget(store),
          quickCaptureShortcut: store.get(keys.QUICK_CAPTURE_SHORTCUT),
          version: __EKANBAN_VERSION__,
          databasePath: options.place,
        };
      }),
    loadDocuments: () => read(() => store.loadDocuments()),
    saveDocument: (document, events) =>
      write(() => ({ rev: store.saveDocument(document, events) })),
    setOpenBoard: (boardId) =>
      write(() => {
        store.set(keys.LAST_BOARD, String(boardId));
      }),
    createBoard: (name) => write(() => store.createBoard(name)),
    deleteBoard: (boardId) =>
      write(() => {
        store.deleteBoard(boardId);
      }),
    setFilterState: (filter) =>
      write(() => {
        store.set(keys.FILTER_SEARCH, filter.search);
        store.set(keys.FILTER_TAG, filter.tagId === null ? null : String(filter.tagId));
      }),
    setSidebarCollapsed: (collapsed) =>
      write(() => {
        store.set(keys.SIDEBAR_COLLAPSED, String(collapsed));
      }),
    setThemePreference: (theme) =>
      write(() => {
        store.set(keys.THEME_PREFERENCE, theme);
      }),
    setWindowTitle: (title) => {
      // ブラウザにウィンドウのタイトルバーが無いので、タブの見出しに出す。
      document.title = title;
      return Promise.resolve();
    },
    // 掛ける相手（OS のメニューバー）がいません。メニューを描くのはページで、
    // 同じ構成を `MenuBar` が読みます（`shell/menu.ts`）。
    setMenu: () => Promise.resolve(),
    onAppAction: (handler) => {
      menuHandler = handler;
      window.ekanbanMenu = handler;
      return () => {
        menuHandler = null;
        delete window.ekanbanMenu;
      };
    },
    // 選ぶところがありません。既定の名前をそのまま「行き先」にします。
    chooseSavePath: (fileName) => Promise.resolve(fileName),
    // 書く先がありません。組み立てられた中身を、そのままダウンロードにします。
    writeTextFile: (destination, extension, contents) => {
      download(destination, contents, contentType(extension));
      return Promise.resolve(destination);
    },
    exportBoardJson: (boardId, destination) =>
      read(() => {
        download(destination, store.exportBoardJson(boardId), "application/json");
        return destination;
      }),
    backupDatabase: (destination) =>
      read(() => {
        // SQLite のファイルがありません（ADR 0036）。置いてあるのは盤面の JSON
        // なので、それをそのまま渡します。メニューでは灰色にしてあります。
        download(destination, store.encode(), "application/json");
        return destination;
      }),
    databaseLocation: () => Promise.resolve(options.place),
    // 開く相手（OS のファイル管理）がいません。`canRevealPaths` が `false` なので、
    // ここに来る導線そのものが出ません。
    canRevealPaths: false,
    revealPath: () => Promise.resolve(),
    revealDatabase: () => Promise.resolve(),
    revealBackups: () => Promise.resolve(),
    openUrl: (url) => {
      // 拾うのは `http(s)://` だけ（ADR 0002）。説明はユーザーが打った文字列
      // なので、`file://` や `javascript:` を混ぜられる場所です。
      if (!url.startsWith("https://") && !url.startsWith("http://")) return Promise.resolve();
      window.open(url, "_blank", "noopener,noreferrer");
      return Promise.resolve();
    },
    captureTarget: () => read(() => readCaptureTarget(store)),
    setCaptureTarget: (target) =>
      write(() => {
        store.set(keys.CAPTURE_BOARD, target === null ? null : String(target.boardId));
        store.set(keys.CAPTURE_COLUMN, target === null ? null : String(target.columnId));
      }),
    quickCaptureStatus: () =>
      Promise.resolve({
        unavailable: options.shortcutUnavailable ?? null,
        failure: null,
      }),
    // ネイティブのメニューがありません。外すものがない。
    setMenuAcceleratorsActive: () => Promise.resolve(),
    // **覚えるところまではします。** 登録できないのは環境の話で
    // （`quickCaptureStatus` がその理由を返します）、割り当てを選べないことでは
    // ありません。この置き場所を持ってアプリ版を開けば、そのまま効きます。
    setQuickCaptureShortcut: (shortcut) =>
      write(() => {
        store.set(keys.QUICK_CAPTURE_SHORTCUT, shortcut);
        return shortcut;
      }),
    // 閉じる窓がありません。
    closeCaptureWindow: () => Promise.resolve(),
    onBoardChanged: (handler) => options.watch?.(handler) ?? (() => undefined),
    logFrontendError: (message) => {
      // 落とす先がありません。開発者コンソールが、この環境のログです。
      console.error(`webview: ${message}`);
      return Promise.resolve();
    },
  };
}

function readTheme(stored: string | null): ThemePreference {
  return stored === "light" || stored === "dark" ? stored : "system";
}

function readCaptureTarget(store: MemoryStore): CaptureTarget | null {
  const boardId = store.get(keys.CAPTURE_BOARD);
  const columnId = store.get(keys.CAPTURE_COLUMN);
  return boardId === null || columnId === null
    ? null
    : { boardId: Number(boardId), columnId: Number(columnId) };
}
