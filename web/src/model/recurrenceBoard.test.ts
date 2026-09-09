// 繰り返しを盤面に当てるところのテスト（#198、[ADR 0049]）。
//
// **周期そのものは `recurrence.test.ts` が見ています。** ここで見るのは盤面に
// 何が起きるか——どこに出るか、前のものがどう片付くか、Undo に積まれないこと、
// 履歴に何が残るか。
//
// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md

import { describe, expect, it } from "vitest";

import type { BoardDocument, RecurrenceDraft } from "./board";
import {
  addCard,
  addRecurrence,
  addTag,
  applyRecurrences,
  canUndo,
  removeColumn,
  removeRecurrence,
  removeTag,
  undo,
  updateRecurrence,
} from "./board";
import { cardAt, columnAt, fixture, must, only } from "./fixture";

function draft(over: Partial<RecurrenceDraft> = {}): RecurrenceDraft {
  return {
    title: "メールを見る",
    description: "",
    columnId: 1,
    tagIds: [],
    checklist: [],
    schedule: { kind: "daily" },
    leadDays: 0,
    previous: "delete",
    enabled: true,
    ...over,
  };
}

/// 定義を 1 つ持つ盤面。**積んだ操作と履歴は空にして返します**——土台を
/// 組み立てた手順が、テストの Undo に混ざらないように。
function withRecurrence(over: Partial<RecurrenceDraft> = {}): {
  document: BoardDocument;
  recurrenceId: number;
} {
  const document = fixture();
  const recurrenceId = must(addRecurrence(document, draft(over)));
  document.undoStack.length = 0;
  document.redoStack.length = 0;
  document.pendingEvents.length = 0;
  return { document, recurrenceId };
}

describe("addRecurrence", () => {
  it("refuses a recurrence without a title", () => {
    const document = fixture();
    const outcome = addRecurrence(document, draft({ title: "  " }));
    expect(outcome.ok).toBe(false);
    expect(document.board.recurrences).toHaveLength(0);
  });

  it("refuses a tag the board does not have", () => {
    const document = fixture();
    expect(addRecurrence(document, draft({ tagIds: [99] })).ok).toBe(false);
  });

  it("starts with nothing generated yet", () => {
    const { document } = withRecurrence();
    expect(only(document.board.recurrences).lastGeneratedOn).toBeNull();
  });

  /// 毎日と平日は先読みを持たない。**モデルが 0 に落とします**——画面に同じ
  /// 規則を書かせないため。
  it("drops the lead days a daily definition cannot use", () => {
    const { document } = withRecurrence({ leadDays: 7 });
    expect(only(document.board.recurrences).leadDays).toBe(0);
  });
});

describe("applyRecurrences", () => {
  it("puts the card at the head of the column it names", () => {
    const { document, recurrenceId } = withRecurrence({ columnId: 2 });

    expect(must(applyRecurrences(document, "2026-02-14"))).toBe(true);
    const put = cardAt(document, 1, 0);
    expect(put.title).toBe("メールを見る");
    expect(put.recurrenceId).toBe(recurrenceId);
    expect(put.occurrenceDate).toBe("2026-02-14");
    // 日次は期限を持たない（毎日 `⚠` を 1 件増やさないため）。
    expect(put.dueDate).toBeNull();
    expect(only(document.board.recurrences).lastGeneratedOn).toBe("2026-02-14");
  });

  it("carries the template's description, tags and checklist", () => {
    const document = fixture();
    const tagId = must(addTag(document, "毎日", ""));
    must(
      addRecurrence(
        document,
        draft({ description: "受信箱を空にする", tagIds: [tagId], checklist: ["未読を見る", " "] }),
      ),
    );

    must(applyRecurrences(document, "2026-02-14"));
    const put = cardAt(document, 0, 0);
    expect(put.description).toBe("受信箱を空にする");
    expect(put.tagIds).toEqual([tagId]);
    // 空の項目は落ちる。
    expect(put.checklistItems.map((item) => item.text)).toEqual(["未読を見る"]);
  });

  it("says nothing changed when the day has not moved on", () => {
    const { document } = withRecurrence();
    must(applyRecurrences(document, "2026-02-14"));
    expect(must(applyRecurrences(document, "2026-02-14"))).toBe(false);
  });

  /// 日次の定義を作ると、翌日の基準日で新しいカードが 1 枚出て、前日のカードが
  /// 片付く（受け入れ条件）。
  it("puts out tomorrow's card and clears yesterday's", () => {
    const { document } = withRecurrence();
    must(applyRecurrences(document, "2026-02-14"));
    const yesterday = cardAt(document, 0, 0).id;

    must(applyRecurrences(document, "2026-02-15"));
    const put = columnAt(document, 0).cards.filter((card) => card.recurrenceId !== null);
    expect(put).toHaveLength(1);
    expect(put[0]?.occurrenceDate).toBe("2026-02-15");
    expect(put[0]?.id).not.toBe(yesterday);
  });

  /// 1 ヶ月ぶりに開いても生成されるのは 1 枚（受け入れ条件）。
  it("puts out one card after a month away", () => {
    const { document } = withRecurrence();
    must(applyRecurrences(document, "2026-01-15"));
    must(applyRecurrences(document, "2026-02-14"));

    const put = columnAt(document, 0).cards.filter((card) => card.recurrenceId !== null);
    expect(put).toHaveLength(1);
    expect(put[0]?.occurrenceDate).toBe("2026-02-14");
  });

  /// 平日の定義で、金曜のカードは土日のあいだ盤面に残り、月曜朝に片付く
  /// （受け入れ条件）。
  it("keeps Friday's card over the weekend", () => {
    const { document } = withRecurrence({ schedule: { kind: "weekday" }, previous: "archive" });
    must(applyRecurrences(document, "2026-02-13"));
    const friday = cardAt(document, 0, 0).id;

    // 土曜と日曜は何も起きない。
    expect(must(applyRecurrences(document, "2026-02-14"))).toBe(false);
    expect(must(applyRecurrences(document, "2026-02-15"))).toBe(false);
    expect(columnAt(document, 0).cards.some((card) => card.id === friday)).toBe(true);

    must(applyRecurrences(document, "2026-02-16"));
    expect(columnAt(document, 0).cards.some((card) => card.id === friday)).toBe(false);
    expect(document.board.archivedCards.map((card) => card.id)).toEqual([friday]);
  });

  /// 月次・先読み 7 日の定義で、生成は発生日の 7 日前、片付けは発生日
  /// （受け入れ条件）。
  it("looks ahead to make the card and waits until the occurrence to clear the last one", () => {
    const { document } = withRecurrence({
      schedule: { kind: "monthly", day: 1 },
      leadDays: 7,
      previous: "archive",
    });
    must(applyRecurrences(document, "2026-01-01"));
    const january = cardAt(document, 0, 0).id;

    // 1 月 24 日はまだ窓が開いていない。
    expect(must(applyRecurrences(document, "2026-01-24"))).toBe(false);

    // 25 日に 2 月ぶんが出る。**1 月ぶんはまだ盤面にいる**（手を動かしている
    // かもしれないので、生成と同時に片付けない）。
    must(applyRecurrences(document, "2026-01-25"));
    expect(cardAt(document, 0, 0).occurrenceDate).toBe("2026-02-01");
    expect(cardAt(document, 0, 0).dueDate).toBe("2026-02-01");
    expect(columnAt(document, 0).cards.some((card) => card.id === january)).toBe(true);

    // 片付くのは 2 月 1 日。
    must(applyRecurrences(document, "2026-02-01"));
    expect(columnAt(document, 0).cards.some((card) => card.id === january)).toBe(false);
  });

  /// 毎月 31 日の定義が、2 月には月末（28 日 / 29 日）に丸められる（受け入れ条件）。
  it("rounds the 31st down to the end of February", () => {
    const { document } = withRecurrence({ schedule: { kind: "monthly", day: 31 } });
    must(applyRecurrences(document, "2026-01-31"));
    must(applyRecurrences(document, "2026-02-28"));
    expect(cardAt(document, 0, 0).occurrenceDate).toBe("2026-02-28");
  });

  /// 入れ先のカラムを消しても生成が止まらず、一番左に入る（受け入れ条件）。
  it("puts the card in the leftmost column when the one it names is gone", () => {
    const { document } = withRecurrence({ columnId: 3 });
    must(removeColumn(document, 3));

    must(applyRecurrences(document, "2026-02-14"));
    expect(cardAt(document, 0, 0).title).toBe("メールを見る");
  });

  /// **未完了でも進行中のカラムにあっても同じように片付ける**（フラグを立てた
  /// 人の選択）。
  it("clears the card wherever it has been moved to", () => {
    const { document } = withRecurrence();
    must(applyRecurrences(document, "2026-02-14"));
    const card = cardAt(document, 0, 0);
    // 進行中へ動かしても、片付けの相手であることは変わらない。
    card.columnId = 2;
    columnAt(document, 0).cards.shift();
    columnAt(document, 1).cards.unshift(card);

    must(applyRecurrences(document, "2026-02-15"));
    expect(columnAt(document, 1).cards.some((each) => each.id === card.id)).toBe(false);
  });

  it("leaves the previous card alone when it is told to keep it", () => {
    const { document } = withRecurrence({ previous: "keep" });
    must(applyRecurrences(document, "2026-02-14"));
    const first = cardAt(document, 0, 0).id;

    must(applyRecurrences(document, "2026-02-15"));
    expect(columnAt(document, 0).cards.some((card) => card.id === first)).toBe(true);
  });

  /// 生成の直後に `Cmd+Z` を押しても、生成されたカードは消えない
  /// （直前のユーザー操作が戻る。受け入れ条件）。
  it("stacks nothing to undo", () => {
    const { document } = withRecurrence();
    must(addCard(document, 1, "手で足したカード", ""));
    must(applyRecurrences(document, "2026-02-14"));

    expect(canUndo(document)).toBe(true);
    must(undo(document));
    // 戻ったのは手で足したカードのほう。繰り返しが出したカードは残る。
    expect(columnAt(document, 0).cards.some((card) => card.title === "手で足したカード")).toBe(
      false,
    );
    expect(columnAt(document, 0).cards.some((card) => card.recurrenceId !== null)).toBe(true);
  });

  /// `card_events` には `created` と `archived` / `deleted` を積む。
  it("writes the card's lifecycle into the history", () => {
    const { document } = withRecurrence({ previous: "delete" });
    must(applyRecurrences(document, "2026-02-14"));
    expect(document.pendingEvents.map((event) => event.kind)).toEqual(["created"]);

    document.pendingEvents.length = 0;
    must(applyRecurrences(document, "2026-02-15"));
    expect(document.pendingEvents.map((event) => event.kind).sort()).toEqual([
      "created",
      "deleted",
    ]);
  });

  it("puts out nothing while the definition is switched off", () => {
    const { document, recurrenceId } = withRecurrence();
    must(updateRecurrence(document, recurrenceId, draft({ enabled: false })));
    expect(must(applyRecurrences(document, "2026-02-14"))).toBe(false);
  });
});

describe("removeRecurrence", () => {
  /// 定義を消しても、盤面のカードは残す。
  it("leaves the cards it already put out on the board", () => {
    const { document, recurrenceId } = withRecurrence();
    must(applyRecurrences(document, "2026-02-14"));
    const put = cardAt(document, 0, 0).id;

    must(removeRecurrence(document, recurrenceId));
    expect(document.board.recurrences).toHaveLength(0);
    expect(columnAt(document, 0).cards.some((card) => card.id === put)).toBe(true);
    // 参照はそのまま残る。**しるしを出すかどうかを決めるのは画面**で、
    // 指す先が消えていることは盤面から読めます。
    expect(cardAt(document, 0, 0).recurrenceId).toBe(recurrenceId);
  });

  it("comes back with undo", () => {
    const { document, recurrenceId } = withRecurrence();
    must(removeRecurrence(document, recurrenceId));
    must(undo(document));
    expect(only(document.board.recurrences).id).toBe(recurrenceId);
  });
});

describe("removeTag", () => {
  /// テンプレートに残ったままだと、置き場所が「知らないタグ」として保存を断る。
  it("takes the tag off the recurrence templates too", () => {
    const document = fixture();
    const tagId = must(addTag(document, "毎日", ""));
    must(addRecurrence(document, draft({ tagIds: [tagId] })));

    must(removeTag(document, tagId));
    expect(only(document.board.recurrences).tagIds).toEqual([]);

    must(undo(document));
    expect(only(document.board.recurrences).tagIds).toEqual([tagId]);
  });
});
