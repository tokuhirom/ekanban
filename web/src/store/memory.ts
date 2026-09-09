// 手元に置く置き場所（[ADR 0041]、[ADR 0042]）。
//
// **これが「サーバ側」の TypeScript 版です。** 配るアプリでは SQLite が
// 引き受けているもの——盤面を読む・検めて書く・ボードを採番する・覚えておく
// 設定を持つ・カードの履歴を残す——を、そのまま手元で行います。ブラウザ版
// （`localStorage`）と、画面のテスト（テストごとに空）が使います。
//
// **盤面の判断は入りません。** カードをどこに置くかを決めるのは
// `web/src/model/board.ts` で、ここが見るのは「そのまま書けるか」だけです
// （[ADR 0040]）。
//
// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
// [ADR 0041]: ../../../docs/adr/0041-one-layer-of-screen-tests.md
// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

import type { BoardDocument } from "../ipc/types/BoardDocument";
import type { CardEvent } from "../ipc/types/CardEvent";
import { renderBoardJson } from "./export";
import * as keys from "./keys";
import { boardScopedId } from "./keys";
import { firstRunBoard } from "./seed";
import type { StoredBoard, StoredCardEvent, StoredState } from "./types";

/// 置いてある形の版。SQLite の `schema_migrations` に当たるもの。
const VERSION = 1;

/// 置き場所が断る理由。
///
/// **`AppError` の形で投げます**——受ける側（画面）は、Tauri から返ってきた
/// 失敗と同じ分岐で扱えます（`web/src/ipc/error.ts`）。
export class StoreError extends Error {
  readonly kind: string;
  readonly title: string;
  readonly detail: string;
  readonly field: null = null;
  readonly value: null = null;

  constructor(kind: string, title: string, detail: string) {
    super(`${title}: ${detail}`);
    this.kind = kind;
    this.title = title;
    this.detail = detail;
  }
}

function conflict(expected: number, current: number): StoreError {
  return new StoreError(
    "save",
    "保存できませんでした",
    `ほかのウィンドウが先に保存しました（版 ${String(expected)} を書こうとして、いまは ${String(current)}）。画面を更新してください`,
  );
}

export class MemoryStore {
  private state: StoredState;

  private constructor(state: StoredState) {
    this.state = state;
  }

  /// 何も入っていない置き場所。最初のボードは `seedIfEmpty` が蒔きます。
  static empty(): MemoryStore {
    return new MemoryStore({ boards: [], state: {}, nextEventId: 1, version: VERSION });
  }

  /// 置いてあった文字列から戻す。**読めなければ空から始めます**——読めない
  /// 文字列を抱えて起動を断ると、ページを開くことすらできません。
  static decode(stored: string): MemoryStore {
    try {
      const value = JSON.parse(stored) as Partial<StoredState>;
      if (!Array.isArray(value.boards)) return MemoryStore.empty();
      return new MemoryStore({
        boards: value.boards,
        state: value.state ?? {},
        nextEventId: value.nextEventId ?? 1,
        version: value.version ?? 0,
      });
    } catch {
      return MemoryStore.empty();
    }
  }

  encode(): string {
    return JSON.stringify(this.state);
  }

  /// ほかの窓が書いたものに入れ替える。
  ///
  /// **開いたときの写しを持っている**ので、外で書き換わったら取り直します
  /// （`ipc/local.ts` の `storage` の出来事）。
  replace(other: MemoryStore): void {
    this.state = other.state;
  }

  /// ボードが 1 つも無ければ、最初の盤面を蒔く。
  seedIfEmpty(): void {
    if (this.state.boards.length > 0) return;
    this.state.boards.push(firstRunBoard());
  }

  loadDocuments(): BoardDocument[] {
    return this.state.boards.map(documentOf);
  }

  /// 盤面を書く。**版を確かめてから書きます**（ADR 0040）。書けたら次の版を返す。
  saveDocument(document: BoardDocument, events: readonly CardEvent[]): number {
    const at = this.state.boards.findIndex((board) => board.id === document.board.id);
    if (at === -1) throw noBoard();
    const stored = this.state.boards[at];
    if (stored === undefined) throw noBoard();
    if (stored.rev !== document.rev) throw conflict(document.rev, stored.rev);

    const rev = stored.rev + 1;
    const kept = [...stored.events];
    for (const event of events) {
      kept.push({
        id: this.state.nextEventId,
        cardId: event.cardId,
        kind: event.kind,
        fromColumnId: event.fromColumnId,
        toColumnId: event.toColumnId,
        at: event.at,
      });
      this.state.nextEventId += 1;
    }
    this.state.boards[at] = storedOf(document, kept, rev);
    return rev;
  }

  /// ボードを 1 つ作る。**採番するのはここ**（ADR 0039）。
  ///
  /// ボードごとに ID の区画を切ります——主キーが 1 本なので、別々のボードが
  /// 手元で採番しても衝突しないように（`store::board_scoped_id`）。
  createBoard(name: string): BoardDocument {
    if (name.trim() === "") {
      throw new StoreError("boardIo", "ボードを作れませんでした", "ボード名を入力してください");
    }
    const largest = this.state.boards.reduce((most, board) => Math.max(most, board.id), 0);
    const stored = Number(this.state.state[keys.NEXT_BOARD] ?? Number.NaN);
    const boardId = Math.max(Number.isNaN(stored) ? largest + 1 : stored, largest + 1);
    this.state.state[keys.NEXT_BOARD] = String(boardId + 1);

    const at = Date.now();
    const first = boardScopedId(boardId);
    const board: StoredBoard = {
      id: boardId,
      name,
      createdAt: at,
      updatedAt: at,
      nextCardId: first,
      nextColumnId: first + 2,
      nextTagId: first,
      nextChecklistItemId: first,
      nextRecurrenceId: first,
      tags: [],
      archivedCards: [],
      columns: [
        column(first, boardId, "やること", 0, at, false),
        column(first + 1, boardId, "完了", 1, at, true),
      ],
      recurrences: [],
      events: [],
      rev: 0,
    };
    this.state.boards.push(board);
    this.state.boards.sort((left, right) => left.id - right.id);
    this.set(keys.LAST_BOARD, String(boardId));
    return documentOf(board);
  }

  /// ボードを消す。**最後の 1 つは消せません**——開く相手がいなくなります。
  deleteBoard(boardId: number): void {
    if (this.state.boards.length <= 1) {
      throw new StoreError(
        "boardIo",
        "ボードを削除できませんでした",
        "最後のボードは削除できません",
      );
    }
    this.state.boards = this.state.boards.filter((board) => board.id !== boardId);
  }

  /// 置いてある形の写しとしての JSON（ADR 0045）。カードの履歴まで入る。
  exportBoardJson(boardId: number): string {
    const stored = this.state.boards.find((board) => board.id === boardId);
    if (stored === undefined) throw noBoard();
    return renderBoardJson(documentOf(stored), stored.events);
  }

  get(key: string): string | null {
    return this.state.state[key] ?? null;
  }

  set(key: string, value: string | null): void {
    // 消すのではなく、入れ直します。**鍵の集まりは持ち歩く文字列そのもの**
    // なので、`delete` で欠けさせるより、無いことを `undefined` で表すほうが
    // 形が読みやすくなります。JSON にしたときにも出ません。
    this.state.state = Object.fromEntries(
      Object.entries({ ...this.state.state, [key]: value }).filter(
        ([, each]) => each !== null,
      ),
    ) as Record<string, string>;
  }
}

function noBoard(): StoreError {
  return new StoreError("boardIo", "ボードを読めませんでした", "そのボードはありません");
}

function column(
  id: number,
  boardId: number,
  name: string,
  position: number,
  at: number,
  done: boolean,
): StoredBoard["columns"][number] {
  return { id, boardId, name, position, createdAt: at, updatedAt: at, done, cards: [] };
}

function documentOf(stored: StoredBoard): BoardDocument {
  return {
    board: {
      id: stored.id,
      name: stored.name,
      createdAt: stored.createdAt,
      updatedAt: stored.updatedAt,
      tags: stored.tags,
      archivedCards: stored.archivedCards,
      columns: stored.columns,
      // **繰り返しを知らない版が書いた文字列も読みます**（#198）。無ければ
      // 空で始め、採番はボードごとの区画の先頭から——ID は主キー 1 本なので、
      // 別々のボードが手元で採番しても衝突しないように（`board_scoped_id`）。
      recurrences: stored.recurrences ?? [],
    },
    nextCardId: stored.nextCardId,
    nextColumnId: stored.nextColumnId,
    nextTagId: stored.nextTagId,
    nextChecklistItemId: stored.nextChecklistItemId,
    nextRecurrenceId: stored.nextRecurrenceId ?? boardScopedId(stored.id),
    rev: stored.rev,
  };
}

function storedOf(
  document: BoardDocument,
  events: StoredCardEvent[],
  rev: number,
): StoredBoard {
  const { board } = document;
  return {
    id: board.id,
    name: board.name,
    createdAt: board.createdAt,
    updatedAt: board.updatedAt,
    nextCardId: document.nextCardId,
    nextColumnId: document.nextColumnId,
    nextTagId: document.nextTagId,
    nextChecklistItemId: document.nextChecklistItemId,
    nextRecurrenceId: document.nextRecurrenceId,
    tags: board.tags,
    archivedCards: board.archivedCards,
    columns: board.columns,
    recurrences: board.recurrences,
    events,
    rev,
  };
}
