// 開きっぱなしで日付をまたいだときの読み直し（#135）。
//
// **期限の判定は画面にあります**（`web/src/model/due.ts`、ADR 0039）。基準日は
// 画面が手元の時計から持つ 1 つで、日付をまたいだらそれが進み、「⚠」も件数も
// 一緒に付いてきます。取り直しに行く相手はもういません。
//
// ここで確かめるのは、**コマンドを 1 回も呼ばずに表示が変わること**です。
// 時計は Playwright の `page.clock` で差し替えます——実際に日をまたぐまで
// 待つわけにはいかないので。

import { expect, test } from "@playwright/test";

import { DEFAULT_DAY_BOUNDARY_HOUR, localDay } from "../src/state/day";
import {
  editStoredBoard,
  editStoredSetting,
  openBoard,
  startHarness,
  stopHarness,
  storedBoard,
} from "./harness";
import { DAY_BOUNDARY_HOUR } from "../src/store/keys";
import { setCardDueDate } from "../src/model/board";
import type { Board } from "../src/ipc/types/Board";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

test("日付をまたぐと、コマンドを呼ばなくても期限の表示が進む", async ({ page }) => {
  const today = localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR);
  const tomorrow = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000)
    .toISOString()
    .slice(0, 10);

  // 明日が期限のカードを 1 枚用意する。
  const cardId = firstCard(await storedBoard(page));
  await editStoredBoard(page, (document) => setCardDueDate(document, cardId, tomorrow));

  // 画面を開く前に時計を握る。開いたあとでは、起動のときに読んだ日付が
  // すでに差し替えたものになってしまう。
  await page.clock.install({ time: new Date(`${today}T23:59:00`) });
  await openBoard(page);

  const due = page.locator(`.card[data-card="${String(cardId)}"] .card-due`);
  await expect(due).toHaveAttribute("data-tone", "info");
  await expect(due).toContainText("あと1日");

  // ここから先、コマンドは 1 回も飛ばない。
  let commands = 0;
  page.on("request", (request) => {
    if (request.url().includes("/invoke/")) commands += 1;
  });

  // 0 時をまたいだだけでは、まだ変わらない（既定の境界は午前 4 時、ADR 0048）。
  await page.clock.setSystemTime(new Date(`${tomorrow}T00:00:30`));
  // 分ごとの見張りが 1 回動くぶんだけ進める。
  await page.clock.runFor(60_000);

  await expect(due).toHaveAttribute("data-tone", "info");
  await expect(due).toContainText("あと1日");

  // 午前 4 時を過ぎたところで、基準日が進む。
  await page.clock.setSystemTime(new Date(`${tomorrow}T04:00:30`));
  await page.clock.runFor(60_000);

  await expect(due).toHaveAttribute("data-tone", "warning");
  await expect(due).toContainText("今日");
  expect(commands, "日付が変わっただけでは、置き場所に聞きに行かない").toBe(0);
});

/// 受け入れ条件（#197）。**午前 3 時は、まだ前の日。**
///
/// カードの `⚠` もボード一覧の件数も同じ基準日から出ているので、両方が同じ
/// 前日を指していることまで見ます。
test("既定では、午前 3 時の時点で前日が「今日」として数えられる", async ({ page }) => {
  const today = localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR);
  const tomorrow = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000)
    .toISOString()
    .slice(0, 10);

  // 「今日」が期限のカードを 1 枚。
  const cardId = firstCard(await storedBoard(page));
  await editStoredBoard(page, (document) => setCardDueDate(document, cardId, today));

  // 暦のうえでは翌日の午前 3 時。境界は午前 4 時なので、基準日はまだ前の日。
  await page.clock.install({ time: new Date(`${tomorrow}T03:00:00`) });
  await openBoard(page);

  const due = page.locator(`.card[data-card="${String(cardId)}"] .card-due`);
  await expect(due).toHaveAttribute("data-tone", "warning");
  await expect(due).toContainText("今日");
  // ボード一覧の件数も同じ基準日から出ている。**まだ 1 枚も過ぎていない。**
  // 件数そのものは土台の盤面しだいなので、何の件数が出ているかで見ます
  // （`done.spec.ts` と同じ読み方）。
  await expect(page.locator(".due-jump[data-tone='warning']")).toHaveCount(1);
  await expect(page.locator(".due-jump[data-tone='danger']")).toHaveCount(0);
});

/// 0 を選べば、0 時境界に戻る（#197）。
test("切り替わりを 0 時にすると、午前 3 時は翌日として数えられる", async ({ page }) => {
  const today = localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR);
  const tomorrow = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000)
    .toISOString()
    .slice(0, 10);

  const cardId = firstCard(await storedBoard(page));
  await editStoredBoard(page, (document) => setCardDueDate(document, cardId, today));
  await editStoredSetting(page, DAY_BOUNDARY_HOUR, "0");

  await page.clock.install({ time: new Date(`${tomorrow}T03:00:00`) });
  await openBoard(page);

  // 0 時境界では、もう翌日。前の日が期限のカードは過ぎている。
  const due = page.locator(`.card[data-card="${String(cardId)}"] .card-due`);
  await expect(due).toHaveAttribute("data-tone", "danger");
  // 同じ時刻・同じ盤面で、一覧の件数も「期限切れ」に変わる。
  await expect(page.locator(".due-jump[data-tone='danger']")).toHaveCount(1);
});

function firstCard(board: Board): number {
  const card = board.columns[0]?.cards[0];
  if (card === undefined) throw new Error("土台のボードにカードがない");
  return card.id;
}
