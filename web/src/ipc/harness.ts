// 開発用ハーネス越しの実装（`docs/TAURI-MIGRATION.md` §10）。
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
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";

/// ハーネスの居場所。`?harness=http://127.0.0.1:1421` で差し替えられます。
export function harnessUrl(): string | null {
  const fromQuery = new URLSearchParams(location.search).get("harness");
  if (fromQuery !== null && fromQuery !== "") return fromQuery.replace(/\/$/, "");
  return null;
}

async function call<T>(base: string, command: string, args: unknown = {}): Promise<T> {
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

export function harnessIpc(base: string): Ipc {
  return {
    startupState: () => call<StartupState>(base, "startup_state"),
    snapshot: () => call<Snapshot>(base, "snapshot"),
    switchBoard: (boardId) => call<Snapshot>(base, "switch_board", { boardId }),
    createBoard: (name) => call<Snapshot>(base, "create_board", { name }),
    renameBoard: (name) => call<Snapshot>(base, "rename_board", { name }),
    deleteBoard: (boardId) => call<Snapshot>(base, "delete_board", { boardId }),
    addCard: (columnId, title, description) =>
      call<Snapshot>(base, "add_card", { columnId, title, description }),
    updateCard: (cardId, title, description, dueDate, tagIds, checklist) =>
      call<Snapshot>(base, "update_card", {
        cardId,
        title,
        description,
        dueDate,
        tagIds,
        checklist,
      }),
    copyCard: (cardId) => call<Snapshot>(base, "copy_card", { cardId }),
    deleteCard: (cardId) => call<Snapshot>(base, "delete_card", { cardId }),
    archiveCard: (cardId) => call<Snapshot>(base, "archive_card", { cardId }),
    setCardTags: (cardId, tagIds) => call<Snapshot>(base, "set_card_tags", { cardId, tagIds }),
    addColumn: (name) => call<Snapshot>(base, "add_column", { name }),
    renameColumn: (columnId, name) => call<Snapshot>(base, "rename_column", { columnId, name }),
    removeColumn: (columnId) => call<Snapshot>(base, "remove_column", { columnId }),
    setColumnWipLimit: (columnId, wipLimit) =>
      call<Snapshot>(base, "set_column_wip_limit", { columnId, wipLimit }),
    sortColumnByDueDate: (columnId) =>
      call<Snapshot>(base, "sort_column_by_due_date", { columnId }),
    archiveColumn: (columnId) => call<Snapshot>(base, "archive_column", { columnId }),
    addTag: (name, color) => call<Snapshot>(base, "add_tag", { name, color }),
    renameTag: (tagId, name) => call<Snapshot>(base, "rename_tag", { tagId, name }),
    setTagColor: (tagId, color) => call<Snapshot>(base, "set_tag_color", { tagId, color }),
    removeTag: (tagId) => call<Snapshot>(base, "remove_tag", { tagId }),
    moveCard: (cardId, toColumnId, toIndex) =>
      call<Snapshot>(base, "move_card", { cardId, toColumnId, toIndex }),
    moveColumn: (columnId, toIndex) => call<Snapshot>(base, "move_column", { columnId, toIndex }),
    filterCards: (query, tagId) => call<number[]>(base, "filter_cards", { query, tagId }),
    setFilterState: async (filter) => {
      await call(base, "set_filter_state", { filter });
    },
    setSidebarCollapsed: async (collapsed) => {
      await call(base, "set_sidebar_collapsed", { collapsed });
    },
    logFrontendError: async (message) => {
      await call(base, "log_frontend_error", { message });
    },
  };
}
