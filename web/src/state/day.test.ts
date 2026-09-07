import { describe, expect, it } from "vitest";

import { dayHasTurned, localDay } from "./day";

describe("localDay", () => {
  it("地方時の日付を YYYY-MM-DD で返す", () => {
    expect(localDay(new Date(2026, 8, 7, 1, 30))).toBe("2026-09-07");
    expect(localDay(new Date(2026, 8, 7, 23, 59))).toBe("2026-09-07");
  });

  it("1 桁の月日を 0 で埋める", () => {
    expect(localDay(new Date(2026, 0, 2, 12, 0))).toBe("2026-01-02");
  });
});

describe("dayHasTurned", () => {
  it("同じ日のうちは変わっていない", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 7, 23, 59, 59))).toBe(false);
  });

  it("日付が変わったら真になる", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 8, 0, 0, 1))).toBe(true);
  });

  /// 開きっぱなしのまま何日も経つこともある。
  it("何日も経っていても真になる", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 30, 9, 0))).toBe(true);
  });
});
