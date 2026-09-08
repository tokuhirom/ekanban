// Rust のコマンドを呼ぶ口。
//
// ここを通すのは、実装を差し替えられるようにするためです。Tauri の
// `invoke` を画面のあちこちから直に呼ぶと、`ekanban-harness` （開発用に
// core を HTTP へ出すもの、`docs/DESIGN.md`「テスト」）を挟めなくなり、
// 画面の振る舞いを Playwright から確かめる道が閉じます。

import type { AppAction } from "./types/AppAction";
import type { BoardDocument } from "./types/BoardDocument";
import type { CardEvent } from "./types/CardEvent";
import type { SavedBoard } from "./types/SavedBoard";
import type { CaptureTarget } from "./types/CaptureTarget";
import type { FilterState } from "./types/FilterState";
import type { QuickCaptureStatus } from "./types/QuickCaptureStatus";
import type { StartupState } from "./types/StartupState";
import type { ThemePreference } from "./types/ThemePreference";

/// 画面が呼べるコマンド。Rust の `crates/app/src/commands.rs` に 1 対 1。
///
/// 段階ごとに必要なぶんだけ増やします。使うあてのない口を先に並べても、
/// 合っているかどうかを確かめる方法がありません。
export interface Ipc {
  /** 起動のときに読む、最初に開くボードと付随する表示の状態。 */
  startupState(): Promise<StartupState>;
  /** 全部のボードを、盤面ごと読む（ADR 0039）。**盤面を持つのは画面**。 */
  loadDocuments(): Promise<BoardDocument[]>;
  /** 盤面を書く。版が合わなければ断られる（ADR 0040）。次の版が返る。
   *
   * **盤面を変える道はこれ 1 本**です。カードを足すのも動かすのも取り消すのも
   * `web/src/model/board.ts` で当ててから、ここへ渡します。 */
  saveDocument(document: BoardDocument, events: CardEvent[]): Promise<SavedBoard>;
  /** 開いているボードを覚える。次の起動でここから始まる。 */
  setOpenBoard(boardId: number): Promise<void>;
  /** ボードを作る。**採番するのは置き場所**なので、ここは頼むだけ（ADR 0039）。 */
  createBoard(name: string): Promise<BoardDocument>;
  /** ボードを消す。最後の 1 つは置き場所が断る。次に開くのは呼んだ側が決める。 */
  deleteBoard(boardId: number): Promise<void>;

  setFilterState(filter: FilterState): Promise<void>;
  setSidebarCollapsed(collapsed: boolean): Promise<void>;
  setThemePreference(theme: ThemePreference): Promise<void>;
  /** 文言は画面が組む（`state/board.ts`）。ここは窓に渡すだけ。 */
  setWindowTitle(title: string): Promise<void>;
  /** メニューが押されたことを受ける（`docs/DESIGN.md`「メニューとキー割り当て」）。返るのは購読をやめる関数。 */
  onAppAction(handler: (action: AppAction) => void): () => void;
  /** OS の保存ダイアログ。閉じられたら `null`——**そのときは何も言わない**
   * （`docs/DESIGN.md`「アプリが伝えること」）。 */
  chooseSavePath(fileName: string): Promise<string | null>;
  /** 組み立てた中身をファイルに書く。拡張子が無ければ補う。書けたパスが返る。
   *
   * ブラウザには書き込める場所が無いので、そちらではダウンロードになる
   * （[ADR 0035]）。
   *
   * [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md */
  writeTextFile(destination: string, extension: string, contents: string): Promise<string>;
  /** 盤面を JSON で書き出す。**組み立てるのも置き場所**——カードの履歴のように、
   * 画面が持っていない値まで入るため（[ADR 0045]）。書けたパスが返る。
   *
   * [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md */
  exportBoardJson(boardId: number, destination: string): Promise<string>;
  /** データベースの控えを取る。書けたパスが返る。 */
  backupDatabase(destination: string): Promise<string>;
  databaseLocation(): Promise<string>;
  /**
   * 「場所を開く」を出せるか。
   *
   * ブラウザだけで動く組み立てには、開く相手（OS のファイル管理）がいません
   * （[ADR 0035]）。**押しても何も起きないボタンを出さない**ために、出す前に
   * ここを見ます。ダウンロードとして受け取ったファイルの居場所は、そもそも
   * ページが知りません。
   *
   * [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
   */
  readonly canRevealPaths: boolean;
  /** OS のファイル管理で場所を開く。 */
  revealPath(path: string): Promise<void>;
  revealDatabase(): Promise<void>;
  /** 控えがまだ 1 つも無ければ、何も起きない。 */
  revealBackups(): Promise<void>;
  /** 説明の中のリンクをブラウザで開く。 */
  openUrl(url: string): Promise<void>;
  /** 覚えてあるクイックキャプチャの入れ先。選ばれていなければ `null`。
   *
   * **既定に落とすのも名前を引くのも画面**です（ADR 0028、ADR 0039）——盤面は
   * こちらが持っているので、往復せずに決められます。 */
  captureTarget(): Promise<CaptureTarget | null>;
  /** 入れ先を覚える。`null` で既定（先頭のボードの先頭カラム）に戻す。 */
  setCaptureTarget(target: CaptureTarget | null): Promise<void>;
  /** 割り当てのダイアログが開くときに読むもの（`docs/DESIGN.md`「クイックキャプチャ」）。 */
  quickCaptureStatus(): Promise<QuickCaptureStatus>;
  /**
   * メニューに付いているキーの割り当てを、付け外しする。
   *
   * 割り当てを捕まえている間だけ外します。付いたままだと、メニューの
   * アクセラレータが webview より先に押されたキーを取ってしまいます。
   */
  setMenuAcceleratorsActive(active: boolean): Promise<void>;
  /** 割り当てを差し替える。`null` で解除。保存された形が返る。
   *
   * 受けるのは `ctrl-shift-n` の形の文字列で、**組み立てるのは画面**です
   * （`shell/shortcut.ts`、ADR 0039）。ここから先は OS への登録で、
   * 登録できなければ保存しません（`docs/DESIGN.md`「クイックキャプチャ」）。 */
  setQuickCaptureShortcut(shortcut: string | null): Promise<string | null>;
  /** キャプチャの窓を閉じる。`focusBoard` でボードを前に出す（ADR 0012）。 */
  closeCaptureWindow(focusBoard: boolean): Promise<void>;
  /** ほかの窓が盤面を書いたときに届く（`docs/DESIGN.md`「コマンドとイベント」）。
   *
   * **積荷はありません**——受け取った側が読み直します。返るのは購読をやめる関数。 */
  onBoardChanged(handler: () => void): () => void;
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
