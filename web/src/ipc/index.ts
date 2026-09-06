// Rust のコマンドを呼ぶ口。
//
// ここを通すのは、実装を差し替えられるようにするためです。Tauri の
// `invoke` を画面のあちこちから直に呼ぶと、`ekanban-harness` （開発用に
// core を HTTP へ出すもの、`docs/DESIGN.md`「テスト」）を挟めなくなり、
// 画面の振る舞いを Playwright から確かめる道が閉じます。

import type { AppAction } from "./types/AppAction";
import type { CaptureTarget } from "./types/CaptureTarget";
import type { ChecklistItemDraft } from "./types/ChecklistItemDraft";
import type { ExportFormat } from "./types/ExportFormat";
import type { KeyPress } from "./types/KeyPress";
import type { FilterState } from "./types/FilterState";
import type { Snapshot } from "./types/Snapshot";
import type { StartupState } from "./types/StartupState";
import type { ThemePreference } from "./types/ThemePreference";
import type { UrlSpan } from "./types/UrlSpan";

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

  /** タイトルが決まってから 1 回だけ呼ぶ。空白だけのタイトルは Rust が断る（`docs/DESIGN.md`「状態の持ち主」）。 */
  addCard(columnId: number, title: string, description: string): Promise<Snapshot>;
  /** カードの中身をまとめて書き換える。チェックリストも項目ごと一括で渡す
   * （`docs/DESIGN.md`「コマンドとイベント」）。 */
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
  /** アーカイブから戻す。戻り先は元のカラムの末尾（`Board::restore_card`）。 */
  restoreCard(cardId: number): Promise<Snapshot>;
  /** 右クリックメニューからタグだけを付け外しする。パネルを開かずに済ませるため。 */
  setCardTags(cardId: number, tagIds: number[]): Promise<Snapshot>;

  addColumn(name: string): Promise<Snapshot>;
  renameColumn(columnId: number, name: string): Promise<Snapshot>;
  removeColumn(columnId: number): Promise<Snapshot>;
  /** 空文字で「上限なし」。読めない値は `Validation` で入力欄に返る。 */
  setColumnWipLimit(columnId: number, wipLimit: string): Promise<Snapshot>;
  archiveColumn(columnId: number): Promise<Snapshot>;

  addTag(name: string, color: string): Promise<Snapshot>;
  renameTag(tagId: number, name: string): Promise<Snapshot>;
  setTagColor(tagId: number, color: string): Promise<Snapshot>;
  removeTag(tagId: number): Promise<Snapshot>;
  /** 落とした瞬間に 1 回だけ呼ぶ。ドラッグ中は webview の中で完結させる
   * （`docs/DESIGN.md`「ドラッグ＆ドロップ」）。 */
  moveCard(cardId: number, toColumnId: number, toIndex: number): Promise<Snapshot>;
  moveColumn(columnId: number, toIndex: number): Promise<Snapshot>;
  /** 検索語とタグに一致するカードの ID。打鍵ごとに呼ぶ（`docs/DESIGN.md`「絞り込みと検索」）。 */
  filterCards(query: string, tagId: number | null): Promise<number[]>;
  /** 取り消し・やり直し。**キーは webview が振り分けます**（`shell/keys.ts`）。 */
  undo(): Promise<Snapshot>;
  redo(): Promise<Snapshot>;
  setFilterState(filter: FilterState): Promise<void>;
  setSidebarCollapsed(collapsed: boolean): Promise<void>;
  setThemePreference(theme: ThemePreference): Promise<void>;
  /** 文言は `Snapshot.windowTitle` が組んだものをそのまま渡す。 */
  setWindowTitle(title: string): Promise<void>;
  /** メニューが押されたことを受ける（`docs/DESIGN.md`「メニューとキー割り当て」）。返るのは購読をやめる関数。 */
  onAppAction(handler: (action: AppAction) => void): () => void;
  /** 保存ダイアログに出す既定のファイル名。 */
  suggestedExportName(format: ExportFormat): Promise<string>;
  /** OS の保存ダイアログ。閉じられたら `null`——**そのときは何も言わない**
   * （`docs/DESIGN.md`「アプリが伝えること」）。 */
  chooseSavePath(fileName: string): Promise<string | null>;
  /** 書き出す。書けたパスが返る。 */
  exportBoard(format: ExportFormat, destination: string): Promise<string>;
  /** データベースの控えを取る。書けたパスが返る。 */
  backupDatabase(destination: string): Promise<string>;
  databaseLocation(): Promise<string>;
  /** OS のファイル管理で場所を開く。 */
  revealPath(path: string): Promise<void>;
  revealDatabase(): Promise<void>;
  /** 控えがまだ 1 つも無ければ、何も起きない。 */
  revealBackups(): Promise<void>;
  /** 説明の中の URL の位置。**見つけ方は Rust に 1 つだけ**（ADR 0002）。 */
  descriptionLinks(text: string): Promise<UrlSpan[]>;
  /** 説明の中のリンクをブラウザで開く。 */
  openUrl(url: string): Promise<void>;
  /** クイックキャプチャの入れ先。設定が無ければ既定（先頭カラム）が返る。 */
  captureTarget(): Promise<CaptureTarget | null>;
  /** 開いているボードのカラムを入れ先にする。`null` で既定に戻す。 */
  setCaptureColumn(columnId: number | null): Promise<Snapshot>;
  /** 1 行のキャプチャ。ボードと同じ保存経路に乗る（`docs/DESIGN.md`「クイックキャプチャ」）。 */
  captureCard(title: string): Promise<Snapshot>;
  /** 割り当てを使えない環境なら、その理由。使えるなら `null`。 */
  quickCaptureSupport(): Promise<string | null>;
  /** 押されたキーを割り当てにする。`null` で解除。保存された形が返る。 */
  setQuickCaptureShortcut(press: KeyPress | null): Promise<string | null>;
  /** キャプチャの窓を閉じる。`focusBoard` でボードを前に出す（ADR 0012）。 */
  closeCaptureWindow(focusBoard: boolean): Promise<void>;
  /** ほかの窓が盤面を変えたときに届く（`docs/DESIGN.md`「コマンドとイベント」）。返るのは購読をやめる関数。 */
  onBoardChanged(handler: (snapshot: Snapshot) => void): () => void;
  /** webview の未捕捉例外を Rust 側と同じログに落とす（`docs/DESIGN.md`「アプリが伝えること」）。 */
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
