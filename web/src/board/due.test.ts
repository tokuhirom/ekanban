import { describe, expect, it } from "vitest";

import { dueChoices } from "./due";

describe("dueChoices", () => {
  /// 2026-09-06 は日曜。週の起点は月曜なので、「来週」は翌日の月曜。
  it("日曜からは、来週が翌日の月曜", () => {
    expect(dueChoices("2026-09-06")).toEqual([
      { label: "今日", date: "2026-09-06" },
      { label: "明日", date: "2026-09-07" },
      { label: "来週", date: "2026-09-07" },
    ]);
  });

  it("月曜からは、来週が 7 日後", () => {
    expect(dueChoices("2026-09-07")[2]).toEqual({ label: "来週", date: "2026-09-14" });
  });

  it("月をまたいでも日付が繰り上がる", () => {
    expect(dueChoices("2026-09-30")[1]).toEqual({ label: "明日", date: "2026-10-01" });
  });

  it("読めない日付には候補を出さない", () => {
    expect(dueChoices("きょう")).toEqual([]);
  });
});
