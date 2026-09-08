// 検索の判定のテスト（`docs/DESIGN.md`「絞り込みと検索」）。
//
// 全角半角・大文字小文字を同じものとして扱うことと、`#12` をカード番号として
// 読むことは、どちらも**外して初めて分かる**種類の振る舞いです。カードが
// 見つからなくなるだけで、画面はふつうに動いて見えます。

import { describe, expect, it } from "vitest";

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { Column } from "../ipc/types/Column";
import { cardMatchesSearch, filterCards, normalizeSearchText, parseCardNumberQuery } from "./search";

function card(id: number, title: string, description = "", tagIds: number[] = []): Card {
  return {
    id,
    columnId: 1,
    title,
    description,
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    dueDate: null,
    tagIds,
    checklistItems: [],
    archivedAt: null,
  };
}

function board(cards: Card[], archived: Card[] = []): Board {
  const column: Column = {
    id: 1,
    boardId: 1,
    name: "やること",
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    done: false,
    cards,
  };
  return {
    id: 1,
    name: "個人 Kanban",
    createdAt: 0,
    updatedAt: 0,
    tags: [],
    archivedCards: archived,
    columns: [column],
  };
}

describe("normalizeSearchText", () => {
  it("searches case insensitively and normalizes full width ascii", () => {
    expect(normalizeSearchText(" ＫＡＮＢＡＮ　")).toBe(" kanban ");

    const subject = card(1, "Rust Ｋａｎｂａｎ", "ローカル DB");
    expect(cardMatchesSearch(subject, "kanban")).toBe(true);
    expect(cardMatchesSearch(subject, "ローカル")).toBe(true);
    expect(cardMatchesSearch(subject, "存在しない")).toBe(false);
  });

  it("matches everything when nothing was typed", () => {
    expect(cardMatchesSearch(card(1, "なんでも"), "")).toBe(true);
    expect(cardMatchesSearch(card(1, "なんでも"), "  ")).toBe(false);
  });
});

describe("parseCardNumberQuery", () => {
  it("finds a card by its number", () => {
    const first = card(1, "ひとつめ");
    const second = card(2, "ふたつめ");

    expect(cardMatchesSearch(first, "#1")).toBe(true);
    expect(cardMatchesSearch(second, "#1")).toBe(false);

    // 全角で打っても同じ。検索欄の正規化を通してから番号として読む。
    expect(parseCardNumberQuery("＃１")).toBe(1);
    expect(parseCardNumberQuery("  #12  ")).toBe(12);
  });

  it("keeps searching for text that merely starts with a hash", () => {
    const subject = card(1, "#イベント の準備");

    // 番号でない `#` 付きの語は、これまでどおりの文字列検索に落ちる。
    expect(parseCardNumberQuery("#イベント")).toBeNull();
    expect(parseCardNumberQuery("#")).toBeNull();
    expect(parseCardNumberQuery("#12a")).toBeNull();
    expect(parseCardNumberQuery("イベント")).toBeNull();
    expect(cardMatchesSearch(subject, "#イベント")).toBe(true);
  });
});

describe("filterCards", () => {
  it("looks at the archive too, and leaves the hiding to the caller", () => {
    const subject = board([card(1, "盤面のカード")], [card(2, "しまったカード")]);
    expect(filterCards(subject, "カード", null)).toEqual([1, 2]);
    expect(filterCards(subject, "しまった", null)).toEqual([2]);
  });

  it("narrows by tag as well as by text", () => {
    const subject = board([
      card(1, "急ぎ", "", [10]),
      card(2, "急ぎでない", "", [11]),
      card(3, "タグなし"),
    ]);
    expect(filterCards(subject, "", 10)).toEqual([1]);
    expect(filterCards(subject, "急ぎ", null)).toEqual([1, 2]);
    // 検索語とタグは重ねて効く。
    expect(filterCards(subject, "急ぎ", 11)).toEqual([2]);
  });
});
