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
  createBoard: (name) => invoke<Snapshot>("create_board", { name }),
  renameBoard: (name) => invoke<Snapshot>("rename_board", { name }),
  deleteBoard: (boardId) => invoke<Snapshot>("delete_board", { boardId }),
  addCard: (columnId, title, description) =>
    invoke<Snapshot>("add_card", { columnId, title, description }),
  updateCard: (cardId, title, description, dueDate, tagIds, checklist) =>
    invoke<Snapshot>("update_card", { cardId, title, description, dueDate, tagIds, checklist }),
  copyCard: (cardId) => invoke<Snapshot>("copy_card", { cardId }),
  deleteCard: (cardId) => invoke<Snapshot>("delete_card", { cardId }),
  archiveCard: (cardId) => invoke<Snapshot>("archive_card", { cardId }),
  setCardTags: (cardId, tagIds) => invoke<Snapshot>("set_card_tags", { cardId, tagIds }),
  addColumn: (name) => invoke<Snapshot>("add_column", { name }),
  renameColumn: (columnId, name) => invoke<Snapshot>("rename_column", { columnId, name }),
  removeColumn: (columnId) => invoke<Snapshot>("remove_column", { columnId }),
  setColumnWipLimit: (columnId, wipLimit) =>
    invoke<Snapshot>("set_column_wip_limit", { columnId, wipLimit }),
  sortColumnByDueDate: (columnId) => invoke<Snapshot>("sort_column_by_due_date", { columnId }),
  archiveColumn: (columnId) => invoke<Snapshot>("archive_column", { columnId }),
  addTag: (name, color) => invoke<Snapshot>("add_tag", { name, color }),
  renameTag: (tagId, name) => invoke<Snapshot>("rename_tag", { tagId, name }),
  setTagColor: (tagId, color) => invoke<Snapshot>("set_tag_color", { tagId, color }),
  removeTag: (tagId) => invoke<Snapshot>("remove_tag", { tagId }),
  moveCard: (cardId, toColumnId, toIndex) =>
    invoke<Snapshot>("move_card", { cardId, toColumnId, toIndex }),
  moveColumn: (columnId, toIndex) => invoke<Snapshot>("move_column", { columnId, toIndex }),
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
