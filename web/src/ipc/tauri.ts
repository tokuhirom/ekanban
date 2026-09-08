// Tauri 越しの実装。
//
// 引数の名前は Rust 側の `#[tauri::command]` の引数名を camelCase にしたもの
// です（Tauri がそう変換します）。返る型は `ts-rs` が Rust から書き出した
// ものなので、ここで型を書き直しません。

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { APP_ACTION, BOARD_CHANGED } from "./events";
import type { Ipc } from "./index";
import type { AppAction } from "./types/AppAction";
import type { CaptureTarget } from "./types/CaptureTarget";
import type { QuickCaptureStatus } from "./types/QuickCaptureStatus";
import type { BoardDocument } from "./types/BoardDocument";
import type { SavedBoard } from "./types/SavedBoard";
import type { StartupState } from "./types/StartupState";

export const tauriIpc: Ipc = {
  startupState: () => invoke<StartupState>("startup_state"),
  loadDocuments: () => invoke<BoardDocument[]>("load_documents"),
  saveDocument: (document, events) => invoke<SavedBoard>("save_document", { document, events }),
  setOpenBoard: async (boardId) => {
    await invoke("set_open_board", { boardId });
  },
  createBoard: (name) => invoke<BoardDocument>("create_board", { name }),
  deleteBoard: async (boardId) => {
    await invoke("delete_board", { boardId });
  },
  setFilterState: async (filter) => {
    await invoke("set_filter_state", { filter });
  },
  setSidebarCollapsed: async (collapsed) => {
    await invoke("set_sidebar_collapsed", { collapsed });
  },
  setThemePreference: async (theme) => {
    await invoke("set_theme_preference", { preference: theme });
  },
  setWindowTitle: async (title) => {
    await invoke("set_window_title", { title });
  },
  onAppAction: (handler) => {
    // 購読が張れるまでは往復が 1 回あります。張り終える前に外されたときに
    // 取りこぼさないよう、外したことを覚えておいて張った直後に外します。
    let stop: (() => void) | null = null;
    let stopped = false;
    void listen<AppAction>(APP_ACTION, (event) => {
      handler(event.payload);
    }).then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    });
    return () => {
      stopped = true;
      stop?.();
    };
  },
  chooseSavePath: (fileName) =>
    invoke<string | null>("choose_save_path", { fileName }),
  writeTextFile: (destination, extension, contents) =>
    invoke<string>("write_text_file", { destination, extension, contents }),
  exportBoardJson: (boardId, destination) =>
    invoke<string>("export_board_json", { boardId, destination }),
  backupDatabase: (destination) =>
    invoke<string>("backup_database", { destination }),
  databaseLocation: () => invoke<string>("database_location"),
  canRevealPaths: true,
  revealPath: async (path) => {
    await invoke("reveal_path", { path });
  },
  revealDatabase: async () => {
    await invoke("reveal_database");
  },
  revealBackups: async () => {
    await invoke("reveal_backups");
  },
  openUrl: async (url) => {
    await invoke("open_url", { url });
  },
  captureTarget: () => invoke<CaptureTarget | null>("capture_target"),
  setCaptureTarget: async (target) => {
    await invoke("set_capture_target", {
      boardId: target?.boardId ?? null,
      columnId: target?.columnId ?? null,
    });
  },
  quickCaptureStatus: () => invoke<QuickCaptureStatus>("quick_capture_status"),
  setMenuAcceleratorsActive: async (active) => {
    await invoke("set_menu_accelerators_active", { active });
  },
  setQuickCaptureShortcut: (shortcut) =>
    invoke<string | null>("set_quick_capture_shortcut", { shortcut }),
  closeCaptureWindow: async (focusBoard) => {
    await invoke("close_capture_window", { focusBoard });
  },
  onBoardChanged: (handler) => {
    let stop: (() => void) | null = null;
    let stopped = false;
    void listen(BOARD_CHANGED, () => {
      handler();
    }).then((unlisten) => {
      if (stopped) unlisten();
      else stop = unlisten;
    });
    return () => {
      stopped = true;
      stop?.();
    };
  },
  logFrontendError: async (message) => {
    await invoke("log_frontend_error", { message });
  },
};
