// 終わったものの置き場（`Column.done`、ADR 0038）を、Playwright ＋ ハーネスで通す。
//
// 見るのは**画面の状態と SQLite に書かれた内容の両方**です。`invoke()` でハーネスを
// 直接叩いて保存された盤面を読み直します。「画面が薄くなった」だけでは、印の
// 保存が抜けていても気づけません。

import { expect, test, type Page } from "@playwright/test";

import { chooseMenu, openBoard, startHarness, stopHarness, storedBoard } from "./harness";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

/// カラムの `…` から完了扱いを切り替える。常用しない操作はここに畳んである
/// （`docs/DESIGN.md`「画面の作り」）。文言も一緒に確かめる——立っているかどうかは
/// 項目の言葉から読めることが条件。
async function toggleDone(page: Page, index: number, label: string): Promise<void> {
  const column = page.locator(".column").nth(index);
  await column.locator(".column-menu-button").click();
  const item = column.locator(".set-column-done");
  await expect(item).toHaveText(label);
  await item.click();
}

test("完了扱いにすると、印が出て、カードが沈み、期限を数えなくなる", async ({ page }) => {
  await openBoard(page);
  const first = page.locator(".column").first();
  const overdue = first.locator(".card").first();

  // 立てる前。期限切れは赤く、一覧の件数にも入っている。
  //
  // **件数は画面で見ます**（ADR 0039）。数えるのが webview になったので、
  // 保存された盤面を読んでも件数は入っていません——入っているのは、数える
  // 材料のほうです。
  await expect(overdue.locator(".card-due")).toHaveAttribute("data-tone", "danger");
  await expect(overdue.locator(".card-due")).toContainText("日超過");
  await expect(page.locator(".due-jump[data-tone='danger']")).toHaveCount(1);

  await toggleDone(page, 0, "完了扱いにする");

  // 印は ✓ だけで、意味は読み上げ名が運ぶ（`docs/DESIGN.md`「画面の作り」）。
  await expect(first.getByRole("img", { name: "完了扱い" })).toBeVisible();
  await expect(page.locator(".column").nth(1).locator(".column-done")).toHaveCount(0);

  // 期限は日付だけになる。急かす印も「N日超過」も出さない（ADR 0038）。
  await expect(overdue.locator(".card-due")).toHaveAttribute("data-tone", "muted");
  await expect(overdue.locator(".card-due")).not.toContainText("超過");
  await expect(overdue.locator(".card-due")).not.toContainText("⚠");

  // 沈めるのは色で、`opacity` ではない——濃さは絞り込みの減光が使っている。
  await expect(overdue).toHaveAttribute("data-done", "true");
  await expect(overdue).not.toHaveAttribute("data-dimmed", /.*/);

  const after = await storedBoard(page);
  expect(after.columns[0]?.done).toBe(true);
  await expect(page.locator(".due-jump[data-tone='danger']")).toHaveCount(0);
});

test("完了扱いは何本でも立てられ、やめれば元に戻る", async ({ page }) => {
  await openBoard(page);

  // 初回のボードでは「完了」が最初から立っている（`Board::first_run`、ADR 0038）。
  await expect(page.locator(".column").nth(2).locator(".column-done")).toHaveCount(1);

  // そのうえでもう 1 本立てられる。「完了」と「キャンセル済み」を並べて両方
  // 立てられるのが、ボードではなくカラムの属性にした理由（ADR 0038）。
  await toggleDone(page, 0, "完了扱いにする");

  await expect(page.locator(".column-done")).toHaveCount(2);
  const marked = await storedBoard(page);
  expect(marked.columns.filter((column) => column.done)).toHaveLength(2);

  await toggleDone(page, 0, "完了扱いをやめる");

  await expect(page.locator(".column").first().locator(".column-done")).toHaveCount(0);
  await expect(page.locator(".column").first().locator(".card").first()).not.toHaveAttribute(
    "data-done",
    /.*/,
  );
  const cleared = await storedBoard(page);
  expect(cleared.columns[0]?.done).toBe(false);
  await expect(page.locator(".due-jump[data-tone='danger']")).toHaveCount(1);
});

test("完了扱いにした 1 手は、Undo で戻る", async ({ page }) => {
  await openBoard(page);

  await toggleDone(page, 0, "完了扱いにする");
  expect((await storedBoard(page)).columns[0]?.done).toBe(true);

  await chooseMenu(page, "undo");

  await expect(page.locator(".column").first().locator(".column-done")).toHaveCount(0);
  expect((await storedBoard(page)).columns[0]?.done).toBe(false);
});
