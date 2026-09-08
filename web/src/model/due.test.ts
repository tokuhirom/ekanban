import { describe, expect, it } from "vitest";

import { dueDatePreview, parseDueDate } from "./due";

/// 2026-09-09 は水曜。曜日をまたぐ数え方はここを基準に読む。
const BASE_DAY = "2026-09-09";

function parsed(value: string, today = BASE_DAY): string | null {
  const read = parseDueDate(value, today);
  if (!read.ok) throw new Error(`「${value}」が読めなかった`);
  return read.date;
}

function rejected(value: string, today = BASE_DAY): boolean {
  return !parseDueDate(value, today).ok;
}

describe("parseDueDate", () => {
  it("parses and rejects due date strings", () => {
    expect(parsed("2028-02-29")).toBe("2028-02-29");
    expect(parsed(" ")).toBeNull();
    expect(rejected("2028-02-30")).toBe(true);
  });

  it("reads a month and a day as this year until it has passed", () => {
    expect(parsed("9/12")).toBe("2026-09-12");
    expect(parsed("9-12")).toBe("2026-09-12");
    // 今日そのものは過ぎていない。
    expect(parsed("9/9")).toBe("2026-09-09");
    // 過ぎているものは来年として読む。
    expect(parsed("9/8")).toBe("2027-09-08");
    expect(parsed("1/5")).toBe("2027-01-05");
  });

  it("reads the words for nearby days", () => {
    expect(parsed("今日")).toBe("2026-09-09");
    expect(parsed("today")).toBe("2026-09-09");
    expect(parsed("明日")).toBe("2026-09-10");
    expect(parsed("tomorrow")).toBe("2026-09-10");
    expect(parsed("明後日")).toBe("2026-09-11");
    expect(parsed("+3")).toBe("2026-09-12");
    expect(parsed("+0")).toBe("2026-09-09");
  });

  /// 2026-09-09 は水曜。「来週」は次の月曜、「今週末」は今週の土曜。
  it("reads next week and the weekend from monday and saturday", () => {
    expect(parsed("来週")).toBe("2026-09-14");
    expect(parsed("今週末")).toBe("2026-09-12");
    // 月曜に打った「来週」は今日ではなく次の月曜。
    expect(parsed("来週", "2026-09-14")).toBe("2026-09-21");
    // 土曜に打った「今週末」は今日。
    expect(parsed("今週末", "2026-09-12")).toBe("2026-09-12");
  });

  it("reads a weekday as the next one to come", () => {
    expect(parsed("金")).toBe("2026-09-11");
    expect(parsed("金曜")).toBe("2026-09-11");
    expect(parsed("金曜日")).toBe("2026-09-11");
    expect(parsed("fri")).toBe("2026-09-11");
    expect(parsed("friday")).toBe("2026-09-11");
    expect(parsed("月")).toBe("2026-09-14");
    // 今日と同じ曜日は 7 日後。今日のことを言っていないため。
    expect(parsed("水")).toBe("2026-09-16");
  });

  /// 全角で打たれたものも、検索と同じ均し方で読む。
  it("reads full width digits and upper case", () => {
    expect(parsed("９/１２")).toBe("2026-09-12");
    expect(parsed("＋３")).toBe("2026-09-12");
    expect(parsed("Tomorrow")).toBe("2026-09-10");
    expect(parsed("　明日　")).toBe("2026-09-10");
  });

  it("rejects what it cannot read", () => {
    for (const value of ["きのう", "9/", "/12", "+", "+ 3", "13/40", "来年"]) {
      expect(rejected(value), `${value} は読めない`).toBe(true);
    }
  });

  /// 読めなかった文字は、打たれたまま返す。入力欄の脇に出すため。
  it("gives back what was typed when it cannot read it", () => {
    const read = parseDueDate("  きのう  ", BASE_DAY);
    expect(read).toEqual({ ok: false, typed: "きのう" });
  });
});

describe("dueDatePreview", () => {
  it("names the weekday of the date it read", () => {
    expect(dueDatePreview("明日", BASE_DAY)).toEqual({
      date: "2026-09-10",
      label: "2026-09-10（木）",
    });
    expect(dueDatePreview("9/12", BASE_DAY)).toEqual({
      date: "2026-09-12",
      label: "2026-09-12（土）",
    });
  });

  it("shows nothing for an empty field or unreadable text", () => {
    expect(dueDatePreview("", BASE_DAY)).toBeNull();
    expect(dueDatePreview("き", BASE_DAY)).toBeNull();
  });
});
