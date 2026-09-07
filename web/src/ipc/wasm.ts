// ブラウザだけで動く組み立ての口（[ADR 0035]）。
//
// 呼んでいるのは `crates/web` の wasm モジュールで、その中で動いているのは
// **本物の `ekanban-core`** です。ここに盤面の論理を書きません——書いた瞬間に
// 2 つ目の実装ができ、デモの中でだけ正しいものが生まれます（[ADR 0021]）。
// `harness.ts` が HTTP を運ぶだけなのと同じで、ここは wasm を呼ぶだけです。
//
// 本物と違うのは、環境に無いものだけです。
//
// - **保存ダイアログがありません。** 名前を決めて、そのままダウンロードにします
// - **ファイル管理を開けません。** `canRevealPaths` が `false` で、書き出しの
//   知らせに「場所を開く」が出ません
// - **メニューバーは OS ではなくページが描きます**（`shell/MenuBar.tsx`）。
//   `onAppAction` は口を開けるだけで、押したことを配るのはページです
//
// [ADR 0021]: ../../../docs/adr/0021-two-layer-testing-for-the-webview.md
// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md

import type { Ipc } from "./index";
import type { AppAction } from "./types/AppAction";
import type { CaptureTarget } from "./types/CaptureTarget";
import type { ExportFormat } from "./types/ExportFormat";
import type { Platform } from "./types/Platform";
import type { QuickCaptureStatus } from "./types/QuickCaptureStatus";
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";
import type { WebSection } from "./types/WebSection";
import type { DueDatePreview } from "./types/DueDatePreview";
import { invoke as callWasm, startWasm } from "./wasmModule";

/// wasm を読み込み、`localStorage` にある盤面で起動する。
///
/// `platform` をページが渡すのは、**`wasm32-unknown-unknown` に「どの OS か」が
/// 無いから**です（[ADR 0035]）。配るアプリでは Rust がコンパイル時に知って
/// いるので、この引数はここにしかありません。
export async function startWasmIpc(platform: Platform): Promise<StartupState> {
  return (await startWasm(platform)) as StartupState;
}

/// この組み立てのメニューバー。中身を決めるのは Rust（`menu::web_sections`）。
export function wasmMenuSections(platform: Platform): WebSection[] {
  return callWasm("menu_sections", { platform }) as WebSection[];
}

// wasm の呼び出しは同期です。**それでも Promise で返します**——`Ipc` の形を
// 揃えておかないと、画面の側に「ここだけ同期」という分岐が生まれます。
//
// `Promise.resolve().then(...)` を挟むのは間合いのためではなく、**wasm が投げた
// `AppError` を reject に変えるため**です。同期で投げると、呼ぶ側が `try` と
// `.catch()` の 2 通りを書くことになります。Tauri の `invoke` は reject で
// 返すので、そちらに揃えます。
async function call<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const answer: unknown = await Promise.resolve().then(() => callWasm(command, args));
  return answer as T;
}

/// ページからファイルを 1 つ受け取らせる。
///
/// ブラウザに「保存する場所を選ぶ」はありません。名前だけ決めて、あとは
/// ダウンロードとして渡します。
function download(fileName: string, contents: BlobPart, type: string): void {
  const url = URL.createObjectURL(new Blob([contents], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  // **文書に入れてから押します。** 外に置いたままだと、`download` に付けた
  // 名前が読まれずに `download` という名前で落ちてくるブラウザがあります。
  document.body.append(link);
  link.click();
  link.remove();
  // 押した直後に外すと、まだ読まれていないことがある。次の間合いで捨てる。
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 0);
}

const CONTENT_TYPE: Record<ExportFormat, string> = {
  json: "application/json",
  markdown: "text/markdown",
};

/// メニューを押したことを配る先。ページが `fireAppAction` で呼びます。
let menuHandler: ((action: AppAction) => void) | null = null;

/// ページのメニューバーから呼ぶ。購読していなければ何も起きません。
export function fireAppAction(action: AppAction): void {
  menuHandler?.(action);
}

export const wasmIpc: Ipc = {
  startupState: () => call<StartupState>("startup_state"),
  snapshot: () => call<Snapshot>("snapshot"),
  switchBoard: (boardId) => call<Snapshot>("switch_board", { boardId }),
  createBoard: (name) => call<Snapshot>("create_board", { name }),
  renameBoard: (name) => call<Snapshot>("rename_board", { name }),
  deleteBoard: (boardId) => call<Snapshot>("delete_board", { boardId }),
  addCard: (columnId, title, description, dueDate, tagIds, checklist) =>
    call<Snapshot>("add_card", {
      columnId,
      title,
      description,
      dueDate,
      tagIds,
      checklist,
    }),
  updateCard: (cardId, title, description, dueDate, tagIds, checklist) =>
    call<Snapshot>("update_card", {
      cardId,
      title,
      description,
      dueDate,
      tagIds,
      checklist,
    }),
  copyCard: (cardId) => call<Snapshot>("copy_card", { cardId }),
  deleteCard: (cardId) => call<Snapshot>("delete_card", { cardId }),
  archiveCard: (cardId) => call<Snapshot>("archive_card", { cardId }),
  restoreCard: (cardId) => call<Snapshot>("restore_card", { cardId }),
  setCardTags: (cardId, tagIds) => call<Snapshot>("set_card_tags", { cardId, tagIds }),
  setCardDueDate: (cardId, dueDate) => call<Snapshot>("set_card_due_date", { cardId, dueDate }),
  addColumn: (name) => call<Snapshot>("add_column", { name }),
  renameColumn: (columnId, name) => call<Snapshot>("rename_column", { columnId, name }),
  removeColumn: (columnId) => call<Snapshot>("remove_column", { columnId }),
  archiveColumn: (columnId) => call<Snapshot>("archive_column", { columnId }),
  addTag: (name, color) => call<Snapshot>("add_tag", { name, color }),
  renameTag: (tagId, name) => call<Snapshot>("rename_tag", { tagId, name }),
  setTagColor: (tagId, color) => call<Snapshot>("set_tag_color", { tagId, color }),
  removeTag: (tagId) => call<Snapshot>("remove_tag", { tagId }),
  moveCard: (cardId, toColumnId, toIndex) =>
    call<Snapshot>("move_card", { cardId, toColumnId, toIndex }),
  moveColumn: (columnId, toIndex) => call<Snapshot>("move_column", { columnId, toIndex }),
  undo: () => call<Snapshot>("undo"),
  redo: () => call<Snapshot>("redo"),
  filterCards: (query, tagId) => call<number[]>("filter_cards", { query, tagId }),
  setFilterState: async (filter) => {
    await call("set_filter_state", { filter });
  },
  setSidebarCollapsed: async (collapsed) => {
    await call("set_sidebar_collapsed", { collapsed });
  },
  setThemePreference: async (theme) => {
    await call("set_theme_preference", { preference: theme });
  },
  setWindowTitle: (title) => {
    // ブラウザにウィンドウのタイトルバーが無いので、タブの見出しに出す。
    document.title = title;
    return Promise.resolve();
  },
  onAppAction: (handler) => {
    // メニューバーを描くのはページです（`shell/MenuBar.tsx`）。ここは配り先を
    // 覚えるだけで、押されたことは `fireAppAction` から入ります。
    menuHandler = handler;
    return () => {
      menuHandler = null;
    };
  },
  suggestedExportName: (format) => call<string>("suggested_export_name", { format }),
  // 選ぶところがありません。既定の名前をそのまま「行き先」にします。
  chooseSavePath: (fileName) => Promise.resolve(fileName),
  exportBoard: async (format, destination) => {
    const contents = await call<string>("export_board_contents", { format });
    download(destination, contents, CONTENT_TYPE[format]);
    return destination;
  },
  backupDatabase: async (destination) => {
    const bytes = await call<number[]>("database_bytes");
    download(destination, new Uint8Array(bytes), "application/vnd.sqlite3");
    return destination;
  },
  databaseLocation: () => call<string>("database_location"),
  // 開く相手（OS のファイル管理）がいません。`canRevealPaths` が `false` なので、
  // ここに来る導線そのものが出ません。
  canRevealPaths: false,
  revealPath: () => Promise.resolve(),
  revealDatabase: () => Promise.resolve(),
  revealBackups: () => Promise.resolve(),
  dueDatePreview: (value) => call<DueDatePreview | null>("due_date_preview", { value }),
  openUrl: async (url) => {
    // **開いてよい URL かどうかは Rust が決めます**（`commands::openable_url`）。
    // 断られたら `null` が返り、何も言わずに終わります。
    const allowed = await call<string | null>("open_url", { url });
    if (allowed === null) return;
    window.open(allowed, "_blank", "noopener,noreferrer");
  },
  captureTarget: () => call<CaptureTarget | null>("capture_target"),
  setCaptureColumn: (columnId) => call<Snapshot>("set_capture_column", { columnId }),
  captureCard: (title) => call<Snapshot>("capture_card", { title }),
  quickCaptureStatus: () => call<QuickCaptureStatus>("quick_capture_status"),
  // ネイティブのメニューがありません。外すものがない。
  setMenuAcceleratorsActive: () => Promise.resolve(),
  // ページの外まで届く割り当ては作れません。メニューの項目が灰色なので、
  // ここへ来る導線はありません。
  setQuickCaptureShortcut: () => Promise.resolve(null),
  // 閉じる窓がありません。
  closeCaptureWindow: () => Promise.resolve(),
  onBoardChanged: () => {
    // 窓が 1 つしかないので、ほかから盤面が変わることがありません。
    return () => {
      // 外すものもない。
    };
  },
  logFrontendError: async (message) => {
    await call("log_frontend_error", { message });
  },
};
