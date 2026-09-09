// 繰り返しの判断のテスト（#198）。
//
// **見るのは境界です。** いつ出るか・いつ出ないか、いつ片付くか・いつ片付か
// ないか。日付をまたぐところを外すと、1 日ずれたまま毎日動き続けます。
//
// 「今日」は引数なので、時計を差し替えずに好きな日を作れます（ADR 0048）。

import { describe, expect, it } from "vitest";

import type { Recurrence } from "../ipc/types/Recurrence";
import type { Schedule } from "../ipc/types/Schedule";
import {
  cardsToCleanUp,
  describeSchedule,
  dueDateOf,
  firstOccurrenceAfter,
  firstOccurrenceOnOrAfter,
  lastOccurrenceOnOrBefore,
  leadDaysOf,
  matches,
  occurrenceToGenerate,
  shouldCleanUp,
  targetColumnId,
} from "./recurrence";
import type { Card } from "../ipc/types/Card";

function recurrence(over: Partial<Recurrence> = {}): Recurrence {
  return {
    id: 1,
    boardId: 1,
    title: "メールを見る",
    description: "",
    columnId: 1,
    tagIds: [],
    checklist: [],
    schedule: { kind: "daily" },
    leadDays: 0,
    previous: "delete",
    enabled: true,
    lastGeneratedOn: null,
    createdAt: 0,
    updatedAt: 0,
    ...over,
  };
}

function card(over: Partial<Card> = {}): Card {
  return {
    id: 1,
    columnId: 1,
    title: "メールを見る",
    description: "",
    position: 0,
    createdAt: 0,
    updatedAt: 0,
    dueDate: null,
    tagIds: [],
    checklistItems: [],
    archivedAt: null,
    recurrenceId: 1,
    occurrenceDate: null,
    ...over,
  };
}

describe("matches", () => {
  it("takes every day for a daily schedule", () => {
    expect(matches({ kind: "daily" }, "2026-02-14")).toBe(true);
    expect(matches({ kind: "daily" }, "2026-02-15")).toBe(true);
  });

  /// 2026-02-13 が金、14 が土、15 が日、16 が月。
  it("skips the weekend for a weekday schedule", () => {
    expect(matches({ kind: "weekday" }, "2026-02-13")).toBe(true);
    expect(matches({ kind: "weekday" }, "2026-02-14")).toBe(false);
    expect(matches({ kind: "weekday" }, "2026-02-15")).toBe(false);
    expect(matches({ kind: "weekday" }, "2026-02-16")).toBe(true);
  });

  it("takes the named weekdays, counting Monday as zero", () => {
    const monday: Schedule = { kind: "weekly", days: [0] };
    expect(matches(monday, "2026-02-16")).toBe(true);
    expect(matches(monday, "2026-02-17")).toBe(false);
  });

  /// 毎月 31 日の定義が、2 月には月末に丸まる（受け入れ条件）。
  it("rounds a day the month does not have down to its last day", () => {
    const last: Schedule = { kind: "monthly", day: 31 };
    expect(matches(last, "2026-01-31")).toBe(true);
    expect(matches(last, "2026-02-28")).toBe(true);
    expect(matches(last, "2026-02-27")).toBe(false);
    // 閏年は 29 日。
    expect(matches(last, "2028-02-29")).toBe(true);
    expect(matches(last, "2028-02-28")).toBe(false);
  });

  it("takes the last day of every month", () => {
    expect(matches({ kind: "monthlyLast" }, "2026-04-30")).toBe(true);
    expect(matches({ kind: "monthlyLast" }, "2026-04-29")).toBe(false);
  });

  it("says no to a date it cannot read", () => {
    expect(matches({ kind: "daily" }, "2026-02-30")).toBe(false);
    expect(matches({ kind: "daily" }, "きょう")).toBe(false);
  });
});

describe("walking the occurrences", () => {
  it("finds the first occurrence on or after a day", () => {
    expect(firstOccurrenceOnOrAfter({ kind: "weekday" }, "2026-02-14")).toBe("2026-02-16");
    expect(firstOccurrenceOnOrAfter({ kind: "weekday" }, "2026-02-16")).toBe("2026-02-16");
    expect(firstOccurrenceOnOrAfter({ kind: "monthly", day: 31 }, "2026-02-01")).toBe(
      "2026-02-28",
    );
  });

  it("finds the next occurrence strictly after a day", () => {
    expect(firstOccurrenceAfter({ kind: "weekday" }, "2026-02-13")).toBe("2026-02-16");
    expect(firstOccurrenceAfter({ kind: "daily" }, "2026-02-13")).toBe("2026-02-14");
  });

  it("walks back to the last occurrence on or before a day", () => {
    expect(lastOccurrenceOnOrBefore({ kind: "weekday" }, "2026-02-15")).toBe("2026-02-13");
    expect(lastOccurrenceOnOrBefore({ kind: "monthly", day: 1 }, "2026-02-28")).toBe(
      "2026-02-01",
    );
  });
});

describe("occurrenceToGenerate", () => {
  it("starts a fresh definition at the next occurrence, not a past one", () => {
    // 毎月 15 日の定義を 20 日に作る。過去の 15 日は出さない。
    const monthly = recurrence({ schedule: { kind: "monthly", day: 15 }, leadDays: 0 });
    expect(occurrenceToGenerate(monthly, "2026-02-20")).toBeNull();
    expect(occurrenceToGenerate(monthly, "2026-03-15")).toBe("2026-03-15");
  });

  it("puts out today's card as soon as a daily definition is made", () => {
    expect(occurrenceToGenerate(recurrence(), "2026-02-14")).toBe("2026-02-14");
  });

  it("puts out nothing while the definition is switched off", () => {
    expect(occurrenceToGenerate(recurrence({ enabled: false }), "2026-02-14")).toBeNull();
  });

  it("puts out one card a day and no more", () => {
    const daily = recurrence({ lastGeneratedOn: "2026-02-14" });
    expect(occurrenceToGenerate(daily, "2026-02-14")).toBeNull();
    expect(occurrenceToGenerate(daily, "2026-02-15")).toBe("2026-02-15");
  });

  /// 1 ヶ月ぶりに開いても生成されるのは 1 枚（受け入れ条件）。
  it("puts out only the latest missed occurrence", () => {
    const daily = recurrence({ lastGeneratedOn: "2026-01-15" });
    expect(occurrenceToGenerate(daily, "2026-02-14")).toBe("2026-02-14");
  });

  /// 月次・先読み 7 日の定義で、生成は発生日の 7 日前（受け入れ条件）。
  it("opens the window exactly the lead days before the occurrence", () => {
    const monthly = recurrence({
      schedule: { kind: "monthly", day: 1 },
      leadDays: 7,
      lastGeneratedOn: "2026-01-01",
    });
    expect(occurrenceToGenerate(monthly, "2026-01-24")).toBeNull();
    expect(occurrenceToGenerate(monthly, "2026-01-25")).toBe("2026-02-01");
  });

  /// 毎日と平日は先読みを持たない（1 日に 2 枚並ばないように）。
  it("refuses to look ahead for a daily or weekday definition", () => {
    const daily = recurrence({ leadDays: 7, lastGeneratedOn: "2026-02-14" });
    expect(occurrenceToGenerate(daily, "2026-02-14")).toBeNull();
    expect(leadDaysOf({ kind: "daily" }, 7)).toBe(0);
    expect(leadDaysOf({ kind: "weekday" }, 7)).toBe(0);
    expect(leadDaysOf({ kind: "weekly", days: [0] }, 7)).toBe(7);
  });
});

describe("shouldCleanUp", () => {
  /// 日次なら翌朝（受け入れ条件）。
  it("clears yesterday's card the next morning", () => {
    expect(shouldCleanUp({ kind: "daily" }, "2026-02-14", "2026-02-14")).toBe(false);
    expect(shouldCleanUp({ kind: "daily" }, "2026-02-14", "2026-02-15")).toBe(true);
  });

  /// 平日の金曜分は月曜朝（受け入れ条件）。
  it("keeps Friday's card over the weekend and clears it on Monday", () => {
    const friday = "2026-02-13";
    expect(shouldCleanUp({ kind: "weekday" }, friday, "2026-02-14")).toBe(false);
    expect(shouldCleanUp({ kind: "weekday" }, friday, "2026-02-15")).toBe(false);
    expect(shouldCleanUp({ kind: "weekday" }, friday, "2026-02-16")).toBe(true);
  });

  /// 月次・先読み 7 日で、片付けは当月 1 日（受け入れ条件）。**生成日ではない。**
  it("clears the previous month's card on the occurrence day, not when the next one is made", () => {
    const monthly: Schedule = { kind: "monthly", day: 1 };
    expect(shouldCleanUp(monthly, "2026-01-01", "2026-01-25")).toBe(false);
    expect(shouldCleanUp(monthly, "2026-01-01", "2026-01-31")).toBe(false);
    expect(shouldCleanUp(monthly, "2026-01-01", "2026-02-01")).toBe(true);
  });
});

describe("cardsToCleanUp", () => {
  it("leaves everything alone when the previous card is kept", () => {
    const kept = recurrence({ previous: "keep" });
    const cards = [card({ occurrenceDate: "2026-02-14" })];
    expect(cardsToCleanUp(kept, cards, "2026-02-15")).toEqual([]);
  });

  it("takes only the cards this definition put out", () => {
    const cards = [
      card({ id: 1, occurrenceDate: "2026-02-14" }),
      card({ id: 2, recurrenceId: 2, occurrenceDate: "2026-02-14" }),
      card({ id: 3, recurrenceId: null, occurrenceDate: null }),
    ];
    expect(cardsToCleanUp(recurrence(), cards, "2026-02-15").map((each) => each.id)).toEqual([1]);
  });
});

describe("targetColumnId", () => {
  /// 入れ先のカラムを消しても生成が止まらず、一番左に入る（受け入れ条件）。
  it("falls back to the leftmost column when the one it names is gone", () => {
    expect(targetColumnId(recurrence({ columnId: 2 }), [{ id: 2 }, { id: 3 }])).toBe(2);
    expect(targetColumnId(recurrence({ columnId: 9 }), [{ id: 2 }, { id: 3 }])).toBe(2);
    expect(targetColumnId(recurrence({ columnId: 9 }), [])).toBeNull();
  });
});

describe("dueDateOf", () => {
  /// 毎日 `⚠` を 1 件増やさないよう、日次と平日は期限を持たない。
  it("gives a due date to the schedules that name a day", () => {
    expect(dueDateOf({ kind: "daily" }, "2026-02-14")).toBeNull();
    expect(dueDateOf({ kind: "weekday" }, "2026-02-13")).toBeNull();
    expect(dueDateOf({ kind: "weekly", days: [0] }, "2026-02-16")).toBe("2026-02-16");
    expect(dueDateOf({ kind: "monthly", day: 1 }, "2026-02-01")).toBe("2026-02-01");
    expect(dueDateOf({ kind: "monthlyLast" }, "2026-02-28")).toBe("2026-02-28");
  });
});

describe("describeSchedule", () => {
  it("says the schedule in one line", () => {
    expect(describeSchedule({ kind: "daily" })).toBe("毎日");
    expect(describeSchedule({ kind: "weekday" })).toBe("平日");
    expect(describeSchedule({ kind: "weekly", days: [4, 0] })).toBe("毎週 月・金");
    expect(describeSchedule({ kind: "monthly", day: 15 })).toBe("毎月 15 日");
    expect(describeSchedule({ kind: "monthlyLast" })).toBe("毎月末");
  });
});
