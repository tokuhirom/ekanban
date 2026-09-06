// Rust のコマンドを呼ぶ口。
//
// ここを通すのは、実装を差し替えられるようにするためです。Tauri の
// `invoke` を画面のあちこちから直に呼ぶと、`ekanban-harness` （開発用に
// core を HTTP へ出すもの、`docs/TAURI-MIGRATION.md` §10）を挟めなくなり、
// 画面の振る舞いを Playwright から確かめる道が閉じます。

import type { FilterState } from "./types/FilterState";
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";

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
  /** 検索語とタグに一致するカードの ID。打鍵ごとに呼ぶ（§5）。 */
  filterCards(query: string, tagId: number | null): Promise<number[]>;
  setFilterState(filter: FilterState): Promise<void>;
  setSidebarCollapsed(collapsed: boolean): Promise<void>;
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
