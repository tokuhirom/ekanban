// Tauri 越しの実装。
//
// 引数の名前は Rust 側の `#[tauri::command]` の引数名を camelCase にしたもの
// です（Tauri がそう変換します）。返る型は `ts-rs` が Rust から書き出した
// ものなので、ここで型を書き直しません。

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { APP_ACTION } from "./events";
import type { Ipc } from "./index";
import type { AppAction } from "./types/AppAction";
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";
import type { UrlSpan } from "./types/UrlSpan";

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
  restoreCard: (cardId) => invoke<Snapshot>("restore_card", { cardId }),
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
  undo: () => invoke<Snapshot>("undo"),
  redo: () => invoke<Snapshot>("redo"),
  filterCards: (query, tagId) => invoke<number[]>("filter_cards", { query, tagId }),
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
  suggestedExportName: (format) => invoke<string>("suggested_export_name", { format }),
  chooseSavePath: (fileName) => invoke<string | null>("choose_save_path", { fileName }),
  exportBoard: (format, destination) => invoke<string>("export_board", { format, destination }),
  backupDatabase: (destination) => invoke<string>("backup_database", { destination }),
  databaseLocation: () => invoke<string>("database_location"),
  revealPath: async (path) => {
    await invoke("reveal_path", { path });
  },
  revealDatabase: async () => {
    await invoke("reveal_database");
  },
  revealBackups: async () => {
    await invoke("reveal_backups");
  },
  descriptionLinks: (text) => invoke<UrlSpan[]>("description_links", { text }),
  openUrl: async (url) => {
    await invoke("open_url", { url });
  },
  logFrontendError: async (message) => {
    await invoke("log_frontend_error", { message });
  },
};
