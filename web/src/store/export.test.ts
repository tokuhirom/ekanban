// 書き出しの形が、置き場所をまたいでも同じであること（ADR 0041、ADR 0042）。
//
// **突き合わせる相手は Rust が出した実物**です（`export.fixture.json`）。同じ
// ファイルを `crates/core/src/export.rs` のテストも読むので、どちらかがずれた
// 日に両方が落ちます。

import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import type { BoardDocument } from "../ipc/types/BoardDocument";
import { renderBoardJson } from "./export";
import type { StoredCardEvent } from "./types";

const at = 1_700_000_000_000;

function document(): BoardDocument {
  return {
    board: {
      id: 1,
      name: "個人 Kanban",
      createdAt: at,
      updatedAt: at + 1,
      tags: [
        {
          id: 1,
          boardId: 1,
          name: "設計",
          color: "#8b5cf6",
          createdAt: at,
          updatedAt: at,
        },
      ],
      archivedCards: [
        {
          id: 3,
          columnId: 1,
          title: "しまったカード",
          description: "",
          position: 0,
          createdAt: at,
          updatedAt: at,
          dueDate: null,
          tagIds: [],
          checklistItems: [],
          archivedAt: at + 2,
          recurrenceId: null,
          occurrenceDate: null,
        },
      ],
      columns: [
        {
          id: 1,
          boardId: 1,
          name: "やること",
          position: 0,
          createdAt: at,
          updatedAt: at,
          done: false,
          cards: [
            {
              id: 1,
              columnId: 1,
              title: "期限つきのカード",
              description: "説明\nの 2 行目",
              position: 0,
              createdAt: at,
              updatedAt: at,
              dueDate: "2026-12-24",
              tagIds: [1],
              checklistItems: [
                {
                  id: 1,
                  cardId: 1,
                  text: "済んだ項目",
                  checked: true,
                  position: 0,
                  createdAt: at,
                  updatedAt: at,
                },
                {
                  id: 2,
                  cardId: 1,
                  text: "まだの項目",
                  checked: false,
                  position: 1,
                  createdAt: at,
                  updatedAt: at,
                },
              ],
              archivedAt: null,
              recurrenceId: null,
              occurrenceDate: null,
            },
          ],
        },
        {
          id: 2,
          boardId: 1,
          name: "完了",
          position: 1,
          createdAt: at,
          updatedAt: at,
          done: true,
          cards: [
            {
              id: 2,
              columnId: 2,
              title: "終わったカード",
              description: "",
              position: 0,
              createdAt: at,
              updatedAt: at,
              dueDate: null,
              tagIds: [],
              checklistItems: [],
              archivedAt: null,
              // 繰り返しが出したカード（#198）。**参照だけ**を持ちます。
              recurrenceId: 1,
              occurrenceDate: "2026-12-21",
            },
          ],
        },
      ],
      recurrences: [
        {
          id: 1,
          boardId: 1,
          title: "週次の振り返り",
          description: "今週やったことを 10 行で。",
          columnId: 1,
          tagIds: [1],
          checklist: ["やったことを並べる"],
          schedule: { kind: "weekly", days: [0, 4] },
          leadDays: 2,
          previous: "archive",
          enabled: true,
          lastGeneratedOn: "2026-12-21",
          createdAt: at,
          updatedAt: at,
        },
      ],
    },
    nextCardId: 4,
    nextColumnId: 3,
    nextTagId: 2,
    nextChecklistItemId: 3,
    nextRecurrenceId: 2,
    rev: 0,
  };
}

const events: StoredCardEvent[] = [
  { id: 1, cardId: 1, kind: "created", fromColumnId: null, toColumnId: 1, at },
  { id: 2, cardId: 3, kind: "archived", fromColumnId: 1, toColumnId: null, at: at + 2 },
];

describe("renderBoardJson", () => {
  it("は Rust が出したものと 1 バイト違わない", () => {
    // 見たいのは中身であって行末ではない。`.gitattributes` が LF に固定して
    // いるが、それが外れた checkout（Windows の既定は CRLF）で落ちると、
    // 突き合わせの差分からは理由が読み取れないので、読んだ時点でそろえる。
    const expected = readFileSync(new URL("./export.fixture.json", import.meta.url), "utf8")
      .replace(/\r\n/g, "\n")
      .replace(/\n$/, "");

    expect(renderBoardJson(document(), events)).toBe(expected);
  });
});
