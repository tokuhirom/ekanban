// アーカイブの並べ方（ADR 0010、§10 の「部品」）。

import { describe, expect, it } from "vitest";

import type { Card } from "../ipc/types/Card";
import { archivedDayLabel, archivedGroups } from "./archive";

function card(id: number, archivedAt: number | null): Card {
  return {
    id,
    columnId: 1,
    title: `カード ${id}`,
    description: "",
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    dueDate: null,
    tagIds: [],
    checklistItems: [],
    archivedAt,
  };
}

/// 手元の時間帯での「その日の正午」。日付だけを問題にしたいので、時間帯の境目を
/// またがない時刻を使う。
function noon(year: number, month: number, day: number): number {
  return new Date(year, month - 1, day, 12).getTime();
}

describe("archivedGroups", () => {
  it("新しい日から並べ、日ごとにまとめる", () => {
    const cards = [
      card(1, noon(2026, 9, 1)),
      card(2, noon(2026, 9, 3)),
      card(3, noon(2026, 9, 3)),
    ];
    const groups = archivedGroups(cards, null);
    expect(groups.map((group) => group.label)).toEqual(["2026/09/03", "2026/09/01"]);
    // 同じ日のカードは ID の順。並びが揺れないようにするためで、gpui 版の
    // `archived_groups` と同じ決め方。
    expect(groups[0]?.cards.map((each) => each.id)).toEqual([2, 3]);
  });

  /// 絞り込みから外れたカードは、減光ではなく**隠す**（ADR 0010）。
  it("絞り込みに一致しないカードは出さない", () => {
    const cards = [card(1, noon(2026, 9, 1)), card(2, noon(2026, 9, 1))];
    const groups = archivedGroups(cards, new Set([2]));
    expect(groups.flatMap((group) => group.cards.map((each) => each.id))).toEqual([2]);
  });

  it("日付を持たないカードも、見出しを付けて出す", () => {
    expect(archivedGroups([card(1, null)], null)[0]?.label).toBe("日付なし");
  });
});

describe("archivedDayLabel", () => {
  it("手元の時間帯の日付にする", () => {
    expect(archivedDayLabel(noon(2026, 1, 5))).toBe("2026/01/05");
  });
});
