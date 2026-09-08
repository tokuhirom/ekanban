// Markdown の書き出しのテスト（`docs/DESIGN.md`「テスト」の「部品」）。
//
// **出す文字列そのものを見ます。** 「カラム名が入っている」だけを確かめると、
// 行頭の記号や空行のような、読む道具が実際に見るところが抜けても通ります。

import { describe, expect, it } from "vitest";

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";
import type { ChecklistItem } from "../ipc/types/ChecklistItem";
import type { Column } from "../ipc/types/Column";
import type { Tag } from "../ipc/types/Tag";
import { renderBoardMarkdown, suggestedExportName } from "./export";

function card(id: number, title: string, description: string, extra: Partial<Card> = {}): Card {
  return {
    id,
    columnId: 1,
    title,
    description,
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    dueDate: null,
    tagIds: [],
    checklistItems: [],
    archivedAt: null,
    ...extra,
  };
}

function column(id: number, name: string, cards: Card[]): Column {
  return { id, boardId: 1, name, position: id - 1, createdAt: 0, updatedAt: 0, done: false, cards };
}

function item(id: number, text: string, checked: boolean): ChecklistItem {
  return { id, cardId: 1, text, checked, position: id - 1, createdAt: 0, updatedAt: 0 };
}

function tag(id: number, name: string): Tag {
  return { id, boardId: 1, name, color: "#60a5fa", createdAt: 0, updatedAt: 0 };
}

function board(columns: Column[], archived: Card[] = [], tags: Tag[] = []): Board {
  return {
    id: 1,
    name: "個人 Kanban",
    createdAt: 0,
    updatedAt: 0,
    tags,
    archivedCards: archived,
    columns,
  };
}

describe("renderBoardMarkdown", () => {
  /// 期待する文字列をそのまま置く。**この形は Rust が出していたものと同じ**で、
  /// 書き出したファイルを読む道具から見て、移す前と後で違いが無いこと。
  it("writes every column and the archive", () => {
    const subject = board(
      [
        column(1, "やること", [
          card(1, "画面の描画をひととおり通す", "カラムとカードを表示する", {
            dueDate: "2026-09-12",
          }),
        ]),
        column(2, "進行中", [card(3, "SQLite の設計", "マイグレーションを用意する", { columnId: 2 })]),
        column(3, "完了", [card(4, "README を書く", "プロジェクトの方針をまとめる", { columnId: 3 })]),
      ],
      [card(2, "D&D の操作を試す", "カードを掴んで移動する", { position: 1, archivedAt: 1 })],
    );

    expect(renderBoardMarkdown(subject)).toBe(
      [
        "# 個人 Kanban",
        "",
        "## やること",
        "",
        "- **画面の描画をひととおり通す**",
        "  - 期限: 2026-09-12",
        "  > カラムとカードを表示する",
        "",
        "## 進行中",
        "",
        "- **SQLite の設計**",
        "  > マイグレーションを用意する",
        "",
        "## 完了",
        "",
        "- **README を書く**",
        "  > プロジェクトの方針をまとめる",
        "",
        "## アーカイブ",
        "",
        "- **D&D の操作を試す**",
        "  - カラム: やること",
        "  - アーカイブ済み",
        "  > カードを掴んで移動する",
        "",
        "",
      ].join("\n"),
    );
  });

  it("says so when a column has no cards", () => {
    expect(renderBoardMarkdown(board([column(1, "空のカラム", [])]))).toContain("カードはありません。");
  });

  it("names the tags on a card and marks off the checklist", () => {
    const subject = board(
      [
        column(1, "やること", [
          card(1, "支度", "", {
            tagIds: [10, 11],
            checklistItems: [item(1, "済んだこと", true), item(2, "まだのこと", false)],
          }),
        ]),
      ],
      [],
      [tag(10, "重要"), tag(11, "今週")],
    );

    const markdown = renderBoardMarkdown(subject);
    expect(markdown).toContain("  - タグ: 重要, 今週\n");
    expect(markdown).toContain("  - [x] 済んだこと\n");
    expect(markdown).toContain("  - [ ] まだのこと\n");
  });

  /// 説明の末尾の改行で、`> ` だけの行を作らない。
  it("does not add an empty quote line for a trailing newline", () => {
    const subject = board([column(1, "やること", [card(1, "題", "1 行目\n2 行目\n")])]);
    expect(renderBoardMarkdown(subject)).toContain("  > 1 行目\n  > 2 行目\n\n");
  });

  it("escapes markdown syntax inside the text the user typed", () => {
    // 書き出した Markdown を読む道具が、カードの中身を見出しや強調として
    // 解釈しないこと。改行も潰す（1 行の中に収めるための形なので）。
    const subject = board([column(1, "やること", [card(1, "*強調* _と_ `コード`", "[link]")])]);
    const markdown = renderBoardMarkdown(subject);
    expect(markdown).toContain("- **\\*強調\\* \\_と\\_ \\`コード\\`**\n");
    expect(markdown).toContain("  > \\[link\\]\n");
  });
});

describe("suggestedExportName", () => {
  it("builds a file name that the file system accepts", () => {
    expect(suggestedExportName("個人 Kanban", "md")).toBe("個人 Kanban.md");
    expect(suggestedExportName("a/b\\c", "json")).toBe("a_b_c.json");
    expect(suggestedExportName("  ...  ", "md")).toBe("board.md");
    expect(suggestedExportName("", "json")).toBe("board.json");
  });
});
