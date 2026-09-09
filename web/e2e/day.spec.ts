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

import { localDay } from "../src/state/day";
import { editStoredBoard, openBoard, startHarness, stopHarness, storedBoard } from "./harness";
import { setCardDueDate } from "../src/model/board";
import type { Board } from "../src/ipc/types/Board";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

test("日付をまたぐと、コマンドを呼ばなくても期限の表示が進む", async ({ page }) => {
  const today = localDay(new Date());
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

  await page.clock.setSystemTime(new Date(`${tomorrow}T00:00:30`));
  // 分ごとの見張りが 1 回動くぶんだけ進める。
  await page.clock.runFor(60_000);

  await expect(due).toHaveAttribute("data-tone", "warning");
  await expect(due).toContainText("今日");
  expect(commands, "日付が変わっただけでは、置き場所に聞きに行かない").toBe(0);
});

function firstCard(board: Board): number {
  const card = board.columns[0]?.cards[0];
  if (card === undefined) throw new Error("土台のボードにカードがない");
  return card.id;
}
