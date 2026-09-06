// Rust のコマンドを呼ぶ口。
//
// ここを通すのは、実装を差し替えられるようにするためです。Tauri の
// `invoke` を画面のあちこちから直に呼ぶと、`ekanban-harness` （開発用に
// core を HTTP へ出すもの、`docs/TAURI-MIGRATION.md` §10）を挟めなくなり、
// 画面の振る舞いを Playwright から確かめる道が閉じます。

import type { AppAction } from "./types/AppAction";
import type { ChecklistItemDraft } from "./types/ChecklistItemDraft";
import type { FilterState } from "./types/FilterState";
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";
import type { ThemePreference } from "./types/ThemePreference";

/// 画面が呼べるコマンド。Rust の `crates/app/src/commands.rs` に 1 対 1。
///
/// 段階ごとに必要なぶんだけ増やします。使うあてのない口を先に並べても、
/// 合っているかどうかを確かめる方法がありません。
export interface Ipc {
  /** 起動のときに読む、盤面と付随する表示の状態。 */
  startupState(): Promise<StartupState>;
  /** いまの盤面。イベントで差し替えるときにも使う。 */
  snapshot(): Promise<Snapshot>;
  switchBoard(boardId: number): Promise<Snapshot>;
  createBoard(name: string): Promise<Snapshot>;
  renameBoard(name: string): Promise<Snapshot>;
  deleteBoard(boardId: number): Promise<Snapshot>;

  /** タイトルが決まってから 1 回だけ呼ぶ。空白だけのタイトルは Rust が断る（§2）。 */
  addCard(columnId: number, title: string, description: string): Promise<Snapshot>;
  /** カードの中身をまとめて書き換える。チェックリストも項目ごと一括で渡す（§3）。 */
  updateCard(
    cardId: number,
    title: string,
    description: string,
    dueDate: string,
    tagIds: number[],
    checklist: ChecklistItemDraft[],
  ): Promise<Snapshot>;
  copyCard(cardId: number): Promise<Snapshot>;
  deleteCard(cardId: number): Promise<Snapshot>;
  archiveCard(cardId: number): Promise<Snapshot>;
  /** 右クリックメニューからタグだけを付け外しする。パネルを開かずに済ませるため。 */
  setCardTags(cardId: number, tagIds: number[]): Promise<Snapshot>;

  addColumn(name: string): Promise<Snapshot>;
  renameColumn(columnId: number, name: string): Promise<Snapshot>;
  removeColumn(columnId: number): Promise<Snapshot>;
  /** 空文字で「上限なし」。読めない値は `Validation` で入力欄に返る。 */
  setColumnWipLimit(columnId: number, wipLimit: string): Promise<Snapshot>;
  sortColumnByDueDate(columnId: number): Promise<Snapshot>;
  archiveColumn(columnId: number): Promise<Snapshot>;

  addTag(name: string, color: string): Promise<Snapshot>;
  renameTag(tagId: number, name: string): Promise<Snapshot>;
  setTagColor(tagId: number, color: string): Promise<Snapshot>;
  removeTag(tagId: number): Promise<Snapshot>;
  /** 落とした瞬間に 1 回だけ呼ぶ。ドラッグ中は webview の中で完結させる（§6）。 */
  moveCard(cardId: number, toColumnId: number, toIndex: number): Promise<Snapshot>;
  moveColumn(columnId: number, toIndex: number): Promise<Snapshot>;
  /** 検索語とタグに一致するカードの ID。打鍵ごとに呼ぶ（§5）。 */
  filterCards(query: string, tagId: number | null): Promise<number[]>;
  /** 取り消し・やり直し。**キーは webview が振り分けます**（`shell/keys.ts`）。 */
  undo(): Promise<Snapshot>;
  redo(): Promise<Snapshot>;
  setFilterState(filter: FilterState): Promise<void>;
  setSidebarCollapsed(collapsed: boolean): Promise<void>;
  setThemePreference(theme: ThemePreference): Promise<void>;
  /** 文言は `Snapshot.windowTitle` が組んだものをそのまま渡す。 */
  setWindowTitle(title: string): Promise<void>;
  /** メニューが押されたことを受ける（§7）。返るのは購読をやめる関数。 */
  onAppAction(handler: (action: AppAction) => void): () => void;
  /** webview の未捕捉例外を Rust 側と同じログに落とす（§9）。 */
  logFrontendError(message: string): Promise<void>;
}

let current: Ipc | null = null;

export function setIpc(ipc: Ipc): void {
  current = ipc;
}

export function useIpc(): Ipc {
  if (current === null) {
    throw new Error("IPC の実装が設定されていません");
  }
  return current;
}
