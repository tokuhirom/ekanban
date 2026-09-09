// 置き場所が持っている形（[ADR 0036]、[ADR 0039]）。
//
// **盤面そのものは `ipc/types/` の生成物を使います。** ここに足すのは、置き場所
// にしか無いもの——カードの履歴と、その版——だけです。
//
// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import type { Board } from "../ipc/types/Board";

/// 保存されたカードの履歴の 1 件。
///
/// `CardEvent`（webview が積むもの）と違って `id` を持ちます。**並び順が
/// 書き出しに出る**ので、置き場所が振った番号をそのまま運びます。
export interface StoredCardEvent {
  id: number;
  cardId: number;
  kind: string;
  fromColumnId: number | null;
  toColumnId: number | null;
  at: number;
}

/// 置いてある盤面 1 つぶん。
///
/// 採番の続きは `Board` の外に出ています——境界を越える `Board` には
/// `#[serde(skip)]` が付いており、保存から外してよいという意味ではないので、
/// 置き場所の側では持ちます。
export interface StoredBoard {
  id: number;
  name: string;
  createdAt: number;
  updatedAt: number;
  nextCardId: number;
  nextColumnId: number;
  nextTagId: number;
  nextChecklistItemId: number;
  /// 繰り返しの定義の採番の続き（#198）。
  ///
  /// **置いてあった文字列に無いことがあります。** 繰り返しを知らない版が
  /// 書いたものをそのまま読み続けられるように、任意にしてあります
  /// （`memory.ts` の `documentOf` が既定に落とします）。
  nextRecurrenceId?: number;
  tags: Board["tags"];
  archivedCards: Board["archivedCards"];
  columns: Board["columns"];
  /// 繰り返しの定義（#198）。古い文字列には入っていません。
  recurrences?: Board["recurrences"];
  events: StoredCardEvent[];
  /// 保存の競合を見るための版（ADR 0040）。
  rev: number;
}

/// 置いてある全部。**文字列 1 つに収まります**（`localStorage` に置くため）。
///
/// 形は `crates/core/src/store.rs` の `JsonStore` と同じにしてあります——
/// 前の版が置いた文字列を、そのまま読み続けられるように。
export interface StoredState {
  boards: StoredBoard[];
  /// 覚えておく設定。鍵は SQLite の `app_state` と同じ綴り（`keys.ts`）。
  state: Record<string, string>;
  nextEventId: number;
  version: number;
}
