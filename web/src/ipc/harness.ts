// 開発用ハーネス越しの実装（`docs/DESIGN.md`「テスト」）。
//
// `ekanban-harness` が `crates/app` のコマンドをそのまま HTTP に出しているので、
// **通っているのは本物の `ekanban-core`** です。偽物のバックエンドを
// TypeScript で書くのはやめる、という ADR 0021 の決めごとがここに掛かって
// います——ここで盤面の論理を再現しはじめたら、テストの中でだけ正しいものが
// できあがります。
//
// これを使うのはブラウザで開いたときだけです。Tauri の中では `tauri.ts` が
// 使われます（`main.tsx` が見分けます）。

import type { Ipc } from "./index";
import type { AppAction } from "./types/AppAction";
import type { CaptureTarget } from "./types/CaptureTarget";
import type { QuickCaptureStatus } from "./types/QuickCaptureStatus";
import type { BoardDocument } from "./types/BoardDocument";
import type { SavedBoard } from "./types/SavedBoard";
import type { StartupState } from "./types/StartupState";

/// ハーネスの居場所。`?harness=http://127.0.0.1:1421` で差し替えられます。
export function harnessUrl(): string | null {
  const fromQuery = new URLSearchParams(location.search).get("harness");
  if (fromQuery !== null && fromQuery !== "")
    return fromQuery.replace(/\/$/, "");
  return null;
}

async function call<T>(
  base: string,
  command: string,
  args: unknown = {},
): Promise<T> {
  const response = await fetch(`${base}/invoke/${command}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
  const payload: unknown = await response.json();
  // Tauri の `invoke` は失敗を reject で返す。同じ形にしないと、画面の側で
  // 分岐が増える。
  if (!response.ok) throw payload;
  return payload as T;
}

declare global {
  interface Window {
    /** ハーネスのときだけ生える、メニューを押したことにする口。 */
    ekanbanMenu?: (action: AppAction) => void;
    /** 同じく、`board:changed` が届いたことにする口。 */
    ekanbanBoardChanged?: () => void;
  }
}

export function harnessIpc(base: string): Ipc {
  return {
    startupState: () => call<StartupState>(base, "startup_state"),
    loadDocuments: () => call<BoardDocument[]>(base, "load_documents"),
    saveDocument: (document, events) =>
      call<SavedBoard>(base, "save_document", { document, events }),
    setOpenBoard: async (boardId) => {
      await call(base, "set_open_board", { boardId });
    },
    createBoard: (name) => call<BoardDocument>(base, "create_board", { name }),
    deleteBoard: async (boardId) => {
      await call(base, "delete_board", { boardId });
    },
    setFilterState: async (filter) => {
      await call(base, "set_filter_state", { filter });
    },
    setSidebarCollapsed: async (collapsed) => {
      await call(base, "set_sidebar_collapsed", { collapsed });
    },
    setThemePreference: async (theme) => {
      await call(base, "set_theme_preference", { preference: theme });
    },
    setWindowTitle: async (title) => {
      // ブラウザにはウィンドウのタイトルバーが無いので、タブの見出しに出す。
      document.title = title;
      return Promise.resolve();
    },
    onAppAction: (handler) => {
      // ハーネスにメニューはありません。**押されたことにする口**だけ開けて、
      // メニューの行き先（`shell/actions.ts` の配り先）を Playwright から
      // 確かめられるようにします。本物のメニューバーが出ることは、殻の煙
      // テストが見ます（`docs/DESIGN.md`「テスト」）。
      window.ekanbanMenu = handler;
      return () => {
        delete window.ekanbanMenu;
      };
    },
    // ブラウザに OS の保存ダイアログはありません。ハーネスがデータベースの隣の
    // パスを返すので、書き出しの経路はそのまま通ります（選ぶところだけが
    // 本物ではない、と分かる形にしてあります）。
    chooseSavePath: (fileName) =>
      call<string | null>(base, "choose_save_path", { fileName }),
    writeTextFile: (destination, extension, contents) =>
      call<string>(base, "write_text_file", { destination, extension, contents }),
    exportBoardJson: (boardId, destination) =>
      call<string>(base, "export_board_json", { boardId, destination }),
    backupDatabase: (destination) =>
      call<string>(base, "backup_database", { destination }),
    databaseLocation: () => call<string>(base, "database_location"),
    // ハーネスは本物と同じ経路を通します。開く相手がいないので何も起きない、
    // という違いだけが残ります（本物では OS のファイル管理が開きます）。
    canRevealPaths: true,
    revealPath: async (path) => {
      await call(base, "reveal_path", { path });
    },
    revealDatabase: async () => {
      await call(base, "reveal_database");
    },
    revealBackups: async () => {
      await call(base, "reveal_backups");
    },
    openUrl: async (url) => {
      await call(base, "open_url", { url });
    },
    captureTarget: () => call<CaptureTarget | null>(base, "capture_target"),
    setCaptureTarget: async (target) => {
      await call(base, "set_capture_target", {
        boardId: target?.boardId ?? null,
        columnId: target?.columnId ?? null,
      });
    },
    quickCaptureStatus: () =>
      call<QuickCaptureStatus>(base, "quick_capture_status"),
    // ブラウザにネイティブのメニューはないので、外すものがない。
    setMenuAcceleratorsActive: () => Promise.resolve(),
    setQuickCaptureShortcut: (press) =>
      call<string | null>(base, "set_quick_capture_shortcut", { press }),
    // ブラウザに閉じる窓がありません。ハーネスでは何も起きないことだけが違い。
    closeCaptureWindow: () => Promise.resolve(),
    onBoardChanged: (handler) => {
      // ハーネスにイベントの経路はありません。**届いたことにする口**だけ開けて、
      // 受け取ったあとの差し替えを Playwright から確かめられるようにします。
      window.ekanbanBoardChanged = handler;
      return () => {
        delete window.ekanbanBoardChanged;
      };
    },
    logFrontendError: async (message) => {
      await call(base, "log_frontend_error", { message });
    },
  };
}
