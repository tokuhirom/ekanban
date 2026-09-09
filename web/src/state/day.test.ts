import { describe, expect, it } from "vitest";

import {
  dayHasTurned,
  DEFAULT_DAY_BOUNDARY_HOUR,
  localDay,
  readDayBoundaryHour,
} from "./day";

describe("localDay", () => {
  it("地方時の日付を YYYY-MM-DD で返す", () => {
    expect(localDay(new Date(2026, 8, 7, 12, 30), 0)).toBe("2026-09-07");
    expect(localDay(new Date(2026, 8, 7, 23, 59), 0)).toBe("2026-09-07");
  });

  it("1 桁の月日を 0 で埋める", () => {
    expect(localDay(new Date(2026, 0, 2, 12, 0), 0)).toBe("2026-01-02");
  });

  /// 既定の境界（午前 4 時）。日をまたいで作業している間は、まだ前日。
  it("境界より前の時刻は前日になる", () => {
    expect(localDay(new Date(2026, 8, 8, 3, 59), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2026-09-07");
    expect(localDay(new Date(2026, 8, 8, 0, 0), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2026-09-07");
  });

  it("境界のちょうどその時刻から当日になる", () => {
    expect(localDay(new Date(2026, 8, 8, 4, 0), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2026-09-08");
    expect(localDay(new Date(2026, 8, 8, 4, 1), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2026-09-08");
  });

  it("境界が 0 なら 0 時境界に戻る", () => {
    expect(localDay(new Date(2026, 8, 8, 0, 0), 0)).toBe("2026-09-08");
    expect(localDay(new Date(2026, 8, 8, 3, 59), 0)).toBe("2026-09-08");
  });

  /// 前日へ戻るときの桁下がり。月初と年初で、月と年まで動く。
  it("月初の未明は前の月の末日になる", () => {
    expect(localDay(new Date(2026, 8, 1, 2, 0), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2026-08-31");
  });

  it("元日の未明は前の年の大晦日になる", () => {
    expect(localDay(new Date(2026, 0, 1, 2, 0), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2025-12-31");
  });

  it("うるう日の翌日の未明は 2 月 29 日になる", () => {
    expect(localDay(new Date(2028, 2, 1, 1, 0), DEFAULT_DAY_BOUNDARY_HOUR)).toBe("2028-02-29");
  });
});

describe("dayHasTurned", () => {
  it("同じ日のうちは変わっていない", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 7, 23, 59, 59), 0)).toBe(false);
  });

  it("日付が変わったら真になる", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 8, 0, 0, 1), 0)).toBe(true);
  });

  /// 開きっぱなしのまま何日も経つこともある。
  it("何日も経っていても真になる", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 30, 9, 0), 0)).toBe(true);
  });

  /// 境界の前後。0 時をまたいだだけでは、まだ変わらない。
  it("境界が 4 なら 0 時をまたいでも変わらない", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 8, 3, 59), DEFAULT_DAY_BOUNDARY_HOUR)).toBe(
      false,
    );
  });

  it("境界が 4 なら午前 4 時に変わる", () => {
    expect(dayHasTurned("2026-09-07", new Date(2026, 8, 8, 4, 0), DEFAULT_DAY_BOUNDARY_HOUR)).toBe(
      true,
    );
  });
});

describe("readDayBoundaryHour", () => {
  it("0〜23 の整数はそのまま", () => {
    expect(readDayBoundaryHour("0")).toBe(0);
    expect(readDayBoundaryHour("23")).toBe(23);
    expect(readDayBoundaryHour(4)).toBe(4);
  });

  it("範囲の外・整数でないもの・読めないものは既定に落ちる", () => {
    expect(readDayBoundaryHour("24")).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
    expect(readDayBoundaryHour("-1")).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
    expect(readDayBoundaryHour("4.5")).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
    expect(readDayBoundaryHour("よる")).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
    expect(readDayBoundaryHour(null)).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
    expect(readDayBoundaryHour(undefined)).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
    expect(readDayBoundaryHour("")).toBe(DEFAULT_DAY_BOUNDARY_HOUR);
  });
});
