// 開きっぱなしで日付をまたいだときの読み直し（#135）。
//
// **期限の判定は Rust のままです。** ここで確かめるのは、手元の日付が
// `Snapshot.today` とずれたときに、画面が盤面を取り直しに行くことだけです。
// 時計は Playwright の `page.clock` で差し替えます——実際に日をまたぐまで
// 待つわけにはいかないので。

import { expect, test } from "@playwright/test";

import { invoke, openBoard, startHarness, stopHarness } from "./harness";
import type { Snapshot } from "../src/ipc/types/Snapshot";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

test("日付をまたぐと、コマンドを呼ばなくても盤面を取り直す", async ({ page }) => {
  const today = ((await (await invoke("snapshot")).json()) as Snapshot).today;
  const tomorrow = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000)
    .toISOString()
    .slice(0, 10);

  // 画面を開く前に時計を握る。開いたあとでは、起動時に読んだ `today` が
  // すでに差し替えた日付で出てしまう。
  await page.clock.install({ time: new Date(`${today}T23:59:00`) });
  await openBoard(page);

  const reread = page.waitForRequest((request) => request.url().endsWith("/invoke/snapshot"));
  await page.clock.setSystemTime(new Date(`${tomorrow}T00:00:30`));
  // 分ごとの見張りが 1 回動くぶんだけ進める。
  await page.clock.runFor(60_000);
  await reread;

  await expect(page.locator(".column").first()).toBeVisible();
});
