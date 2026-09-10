// 繰り返しタスクを、画面から作って日をまたぐところ（#198、[ADR 0049]）。
//
// **単位のテストは `src/model/recurrence.test.ts` が見ています。** ここで見る
// のはパネルからの経路——メニューから開いて定義を作り、時計を進めるとカードが
// 出て、前のものが片付くこと。読むのは画面と**置き場所の両方**です（ADR 0041）。
//
// 時計は Playwright の `page.clock` で差し替えます。実際に日をまたぐまで
// 待つわけにはいかないので。
//
// [ADR 0041]: ../../docs/adr/0041-one-layer-of-screen-tests.md
// [ADR 0049]: ../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md

import { expect, test, type Page } from "@playwright/test";

import { openBoard, startHarness, stopHarness, storedBoard } from "./harness";
import { DEFAULT_DAY_BOUNDARY_HOUR, localDay } from "../src/state/day";

// `window.ekanbanMenu` の宣言を読み込むためだけの取り込み（値は使わない）。
import type {} from "../src/ipc/browser";
import type { AppAction } from "../src/ipc/types/AppAction";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

async function chooseMenu(page: Page, action: AppAction): Promise<void> {
  await page.evaluate((name: AppAction) => {
    window.ekanbanMenu?.(name);
  }, action);
}

/// 置き場所に入っている、繰り返しが出したカード。
async function recurringTitles(page: Page): Promise<string[]> {
  const board = await storedBoard(page);
  return board.columns
    .flatMap((column) => column.cards)
    .filter((card) => card.recurrenceId !== null)
    .map((card) => card.title);
}

/// 日付を `days` 日ずらした `"YYYY-MM-DD"`。
function shift(day: string, days: number): string {
  return new Date(Date.parse(`${day}T00:00:00Z`) + days * 86_400_000)
    .toISOString()
    .slice(0, 10);
}

test("繰り返しを作ると当日のカードが出て、翌日には入れ替わる", async ({ page }) => {
  const today = localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR);
  const tomorrow = shift(today, 1);

  // 画面を開く前に時計を握る。開いたあとでは、起動のときに読んだ日付が
  // すでに差し替えたものになってしまう。
  await page.clock.install({ time: new Date(`${today}T12:00:00`) });
  await openBoard(page);

  // メニューの「繰り返しを設定…」がパネルの入り口（ADR 0047 の設定画面とは別で、
  // これは開いているボードのもの）。
  await chooseMenu(page, "manageRecurrences");
  const panel = page.locator(".recurrence-panel");
  await expect(panel).toBeVisible();

  await panel.locator(".recurrence-add .recurrence-title-input").fill("メールを見る");
  await panel.locator(".add-recurrence").click();

  // 追加した時点で、その日のぶんが 1 枚出る。画面にも置き場所にも。
  const card = page.locator(".card", { hasText: "メールを見る" });
  await expect(card).toHaveCount(1);
  await expect(card.locator(".card-recurring")).toBeVisible();
  await expect.poll(() => recurringTitles(page)).toEqual(["メールを見る"]);

  const first = await storedBoard(page);
  const firstId = first.columns
    .flatMap((column) => column.cards)
    .find((each) => each.recurrenceId !== null)?.id;
  expect(firstId, "繰り返しが出したカードが置き場所にある").toBeDefined();

  // 翌日の午前 4 時を過ぎたところで基準日が進み、入れ替わる。
  await page.clock.setSystemTime(new Date(`${tomorrow}T04:00:30`));
  await page.clock.runFor(60_000);

  await expect.poll(() => recurringTitles(page)).toEqual(["メールを見る"]);
  const next = await storedBoard(page);
  const nextCard = next.columns
    .flatMap((column) => column.cards)
    .find((each) => each.recurrenceId !== null);
  expect(nextCard?.occurrenceDate, "今日のぶんに入れ替わっている").toBe(tomorrow);
  expect(nextCard?.id, "前日のカードは片付いている").not.toBe(firstId);
});

/// 生成の直後に `Cmd+Z` を押しても、生成されたカードは消えない（受け入れ条件）。
test("生成は取り消しに積まれず、Cmd+Z は直前のユーザー操作を戻す", async ({ page }) => {
  const today = localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR);

  await page.clock.install({ time: new Date(`${today}T12:00:00`) });
  await openBoard(page);

  await chooseMenu(page, "manageRecurrences");
  const panel = page.locator(".recurrence-panel");
  await panel.locator(".recurrence-add .recurrence-title-input").fill("メールを見る");
  await panel.locator(".add-recurrence").click();
  await expect(page.locator(".card", { hasText: "メールを見る" })).toHaveCount(1);

  // パネルを畳んでから、盤面で 1 手ぶん動かす。
  await chooseMenu(page, "cancelEdit");
  const before = await storedBoard(page);
  const moved = before.columns[0]?.cards[1];
  expect(moved, "動かせるカードがある").toBeDefined();
  await page.locator(`.card[data-card="${String(moved?.id)}"]`).click();
  await chooseMenu(page, "undo");

  // 戻ったのはカードの選択ではなく、直前に積まれた操作（繰り返しの追加）。
  // **繰り返しが出したカードは盤面に残ります。**
  await expect.poll(() => recurringTitles(page)).toEqual(["メールを見る"]);
  // 定義そのものは戻るので、しるしは消える（指す先がいなくなるため）。
  await expect
    .poll(async () => (await storedBoard(page)).recurrences.length)
    .toBe(0);
  await expect(page.locator(".card-recurring")).toHaveCount(0);
});

/// タグの欄はカードの編集パネルと同じもの（`panel/TagsInput.tsx`）。ここで見る
/// のは、**打った名前でタグが作られて定義に付く**こと——ボードのタグを全部
/// 並べた押しボタンではなくなったので、経路そのものが変わっています。
test("繰り返しのタグは打って作れ、周期に使えない先読みは灰色になる", async ({ page }) => {
  const today = localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR);

  await page.clock.install({ time: new Date(`${today}T12:00:00`) });
  await openBoard(page);

  await chooseMenu(page, "manageRecurrences");
  const panel = page.locator(".recurrence-panel");
  await panel.locator(".recurrence-add .recurrence-title-input").fill("メールを見る");
  await panel.locator(".add-recurrence").click();

  const row = panel.locator(".recurrence-row");
  await expect(row).toHaveCount(1);

  // 既定は「毎日」なので、先読みは触れない。灰色にしているだけでなく、
  // 理由も文言に出す（`docs/DESIGN.md`「色だけに意味を持たせない」）。
  const lead = row.getByLabel("メールを見る の先読み日数");
  await expect(lead).toBeDisabled();
  await expect(row.locator(".recurrence-field.is-disabled")).toHaveCount(1);
  await expect(row).toContainText("毎日・平日は先読みを持ちません");

  // 「毎週」にすると触れるようになる。
  await row.getByLabel("メールを見る の周期").selectOption("weekly");
  await expect(lead).toBeEnabled();
  await expect(row.locator(".recurrence-field.is-disabled")).toHaveCount(0);

  // カードの編集パネルと同じ欄——打って Enter で、無ければ作って付ける。
  await row.getByLabel("メールを見る のタグ").fill("朝");
  await row.getByLabel("メールを見る のタグ").press("Enter");

  await expect(row.locator(".tags-input-chip")).toHaveText(/朝/);
  await expect
    .poll(async () => {
      const board = await storedBoard(page);
      const tag = board.tags.find((each) => each.name === "朝");
      return board.recurrences[0]?.tagIds.includes(tag?.id ?? -1) ?? false;
    }, { message: "作ったタグが定義に付いている" })
    .toBe(true);
});
