// カレンダーの升目のテスト（`docs/DESIGN.md`「テスト」の「部品」）。

import { describe, expect, it } from "vitest";

import { monthLabel, monthOf, monthWeeks, shiftMonth } from "./calendar";

describe("monthWeeks", () => {
  it("月曜から始まり、常に 6 週ぶん出る", () => {
    // 2026-09-01 は火曜。前の週の月曜（8/31）から始まる。
    const weeks = monthWeeks("2026-09");
    expect(weeks).toHaveLength(6);
    expect(weeks.every((week) => week.length === 7)).toBe(true);
    expect(weeks[0]?.[0]?.date).toBe("2026-08-31");
    expect(weeks[0]?.[1]?.date).toBe("2026-09-01");
    expect(weeks[5]?.[6]?.date).toBe("2026-10-11");
  });

  it("前後の月からはみ出した日に印が付く", () => {
    const days = monthWeeks("2026-09").flat();
    expect(days.find((day) => day.date === "2026-08-31")?.inMonth).toBe(false);
    expect(days.find((day) => day.date === "2026-09-01")?.inMonth).toBe(true);
    expect(days.find((day) => day.date === "2026-09-30")?.inMonth).toBe(true);
    expect(days.find((day) => day.date === "2026-10-01")?.inMonth).toBe(false);
  });

  it("月の 1 日が月曜でも、その週から始まる", () => {
    // 2026-06-01 は月曜。前の月から借りる日は無い。
    expect(monthWeeks("2026-06")[0]?.[0]?.date).toBe("2026-06-01");
  });

  it("日にちの数字を添える", () => {
    expect(monthWeeks("2026-09")[0]?.[1]).toEqual({
      date: "2026-09-01",
      day: 1,
      inMonth: true,
    });
  });

  it("読めない月では升目を出さない", () => {
    expect(monthWeeks("2026-13")).toEqual([]);
    expect(monthWeeks("2026-9")).toEqual([]);
    expect(monthWeeks("")).toEqual([]);
  });
});

describe("shiftMonth", () => {
  it("年をまたいで前後に送れる", () => {
    expect(shiftMonth("2026-12", 1)).toBe("2027-01");
    expect(shiftMonth("2026-01", -1)).toBe("2025-12");
    expect(shiftMonth("2026-09", 0)).toBe("2026-09");
    expect(shiftMonth("2026-09", 12)).toBe("2027-09");
  });

  it("読めない月と、作れない月では null", () => {
    expect(shiftMonth("2026-13", 1)).toBeNull();
    expect(shiftMonth("0000-01", -1)).toBeNull();
  });
});

describe("monthOf", () => {
  it("日付が属する月を出す", () => {
    expect(monthOf("2026-09-12")).toBe("2026-09");
  });

  it("日付として成り立たないものは null", () => {
    // 2 月 30 日は無い。`parseIsoDate` と同じ判定に乗せる。
    expect(monthOf("2026-02-30")).toBeNull();
    expect(monthOf("明日")).toBeNull();
  });
});

describe("monthLabel", () => {
  it("年と月を日本語で出す", () => {
    expect(monthLabel("2026-09")).toBe("2026年9月");
    expect(monthLabel("2026-12")).toBe("2026年12月");
  });

  it("読めない月では空", () => {
    expect(monthLabel("2026-13")).toBe("");
  });
});
