// Tauri 越しの実装。
//
// 引数の名前は Rust 側の `#[tauri::command]` の引数名を camelCase にしたもの
// です（Tauri がそう変換します）。返る型は `ts-rs` が Rust から書き出した
// ものなので、ここで型を書き直しません。

import { invoke } from "@tauri-apps/api/core";

import type { Ipc } from "./index";
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";

export const tauriIpc: Ipc = {
  startupState: () => invoke<StartupState>("startup_state"),
  snapshot: () => invoke<Snapshot>("snapshot"),
  switchBoard: (boardId) => invoke<Snapshot>("switch_board", { boardId }),
  filterCards: (query, tagId) => invoke<number[]>("filter_cards", { query, tagId }),
  setFilterState: async (filter) => {
    await invoke("set_filter_state", { filter });
  },
  setSidebarCollapsed: async (collapsed) => {
    await invoke("set_sidebar_collapsed", { collapsed });
  },
  logFrontendError: async (message) => {
    await invoke("log_frontend_error", { message });
  },
};
