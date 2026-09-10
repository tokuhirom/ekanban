import { describe, expect, it } from "vitest";

import { readQuickCapture } from "./quickCapture";

/// 2026-09-10 は木曜。次の金曜は 09-11、次の水曜は 09-16。
const BASE_DAY = "2026-09-10";

function read(line: string) {
  return readQuickCapture(line, BASE_DAY);
}

describe("readQuickCapture", () => {
  it("reads a due date written after an at mark", () => {
    expect(read("牛乳を買う @today")).toEqual({
      title: "牛乳を買う",
      dueDate: "2026-09-10",
      tagNames: [],
    });
    expect(read("棚卸し @+3")).toEqual({ title: "棚卸し", dueDate: "2026-09-13", tagNames: [] });
    expect(read("会議の資料 @9/12")).toEqual({
      title: "会議の資料",
      dueDate: "2026-09-12",
      tagNames: [],
    });
  });

  it("needs no space before the mark", () => {
    // 日本語は空白で語を切らない。空白を打たせない。
    expect(read("買い物@明日")).toEqual({ title: "買い物", dueDate: "2026-09-11", tagNames: [] });
  });

  it("takes a due date and tags in any order", () => {
    const expected = { title: "買い物", dueDate: "2026-09-11", tagNames: ["家事"] };
    expect(read("買い物@明日 #家事")).toEqual(expected);
    expect(read("買い物 #家事 @明日")).toEqual(expected);
  });

  it("keeps the tags in the order they were typed", () => {
    expect(read("掃除 #家事 #週末")).toEqual({
      title: "掃除",
      dueDate: null,
      tagNames: ["家事", "週末"],
    });
  });

  it("reads marks typed in full width", () => {
    expect(read("買い物＠明日　＃家事")).toEqual({
      title: "買い物",
      dueDate: "2026-09-11",
      tagNames: ["家事"],
    });
  });

  it("leaves a line it cannot read alone", () => {
    // `@example.com` は日付として読めない。行はそのままタイトル。
    expect(read("メールは foo@example.com")).toEqual({
      title: "メールは foo@example.com",
      dueDate: null,
      tagNames: [],
    });
    // 空白を含むトークンは剥がさない。
    expect(read("打ち合わせ @明日 と資料")).toEqual({
      title: "打ち合わせ @明日 と資料",
      dueDate: null,
      tagNames: [],
    });
    // 記号のうしろに何も無ければ、そこで止まる。
    expect(read("メモ #")).toEqual({ title: "メモ #", dueDate: null, tagNames: [] });
  });

  it("strips only from the end of the line", () => {
    // 先頭の `#12` はカード番号の見た目のまま残る（行末の `@金` だけが外れる）。
    expect(read("#12 の件を確認 @金")).toEqual({
      title: "#12 の件を確認",
      dueDate: "2026-09-11",
      tagNames: [],
    });
    expect(read("@明日 の買い物")).toEqual({
      title: "@明日 の買い物",
      dueDate: null,
      tagNames: [],
    });
  });

  it("takes only the last due date", () => {
    // 2 つ目の `@…` はタイトルに残す。どちらを採るかを規則にしない。
    expect(read("打ち合わせ@水 @明日")).toEqual({
      title: "打ち合わせ@水",
      dueDate: "2026-09-11",
      tagNames: [],
    });
  });

  it("can leave the title empty", () => {
    // 何を足すのか分からない 1 行。断るのは呼ぶ側（`capture/Capture.tsx`）。
    expect(read("@today")).toEqual({ title: "", dueDate: "2026-09-10", tagNames: [] });
    expect(read("")).toEqual({ title: "", dueDate: null, tagNames: [] });
  });
});
