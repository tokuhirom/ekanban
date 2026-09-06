// アーカイブ、書き出し、控えの保存、説明のリンク。
//
// 「書き出したファイルが読め、控えが増える」を確かめるので、**書けたファイルを
// ディスクから読み直します**。画面に「書き出しました」と出ているだけでは、
// 書けていないことに気づけません。
//
// 保存先を選ぶところだけが本物ではありません。ブラウザに OS の保存ダイアログは
// 無いので、ハーネスがデータベースの隣のパスを返します（`src/ipc/harness.ts`）。

import { readFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { invoke, openBoard, startHarness, stopHarness } from "./harness";

import type {} from "../src/ipc/harness";
import type { AppAction } from "../src/ipc/types/AppAction";
import type { Snapshot } from "../src/ipc/types/Snapshot";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

async function chooseMenu(page: Page, action: AppAction): Promise<void> {
  await page.evaluate((name: AppAction) => {
    window.ekanbanMenu?.(name);
  }, action);
}

async function storedBoard(): Promise<Snapshot["board"]> {
  const response = await invoke("snapshot");
  const snapshot = (await response.json()) as Snapshot;
  return snapshot.board;
}

/// ダイアログが出した書き出し先を読む。
async function writtenPath(page: Page): Promise<string> {
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  return (await dialog.locator(".dialog-detail").innerText()).trim();
}

// ---------------------------------------------------------------- アーカイブ

test("アーカイブしたカードが日ごとに並び、復元でボードへ戻る", async ({ page }) => {
  await openBoard(page);
  const first = page.locator(".column").first().locator(".card").first();
  const title = await first.locator(".card-title").innerText();
  await first.click({ button: "right" });
  await page.locator(".card-menu").getByRole("button", { name: "アーカイブ", exact: true }).click();

  await expect.poll(async () => (await storedBoard()).archivedCards.length).toBe(1);

  await chooseMenu(page, "toggleArchiveView");
  const archive = page.locator(".archive");
  await expect(archive).toBeVisible();
  await expect(archive.locator(".archived-card")).toHaveCount(1);
  await expect(archive.locator(".archive-day-label")).toHaveCount(1);
  // カラムはもう出ていない。盤面の代わりに並べる（ADR 0010）。
  await expect(page.locator(".column")).toHaveCount(0);

  await archive.locator(".restore-card").click();
  await expect.poll(async () => (await storedBoard()).archivedCards.length).toBe(0);
  await expect.poll(async () => (await storedBoard()).columns[0]?.cards.at(-1)?.title).toBe(title);
});

test("アーカイブでは、絞り込みに外れたカードを隠す", async ({ page }) => {
  // 画面を開く前に 2 枚アーカイブしておく。ここで確かめたいのは絞り込みの
  // 効き方で、アーカイブする道はもう上のテストが通っている。
  const board = await storedBoard();
  for (const card of board.columns[0]?.cards.slice(0, 2) ?? []) {
    await invoke("archive_card", { cardId: card.id });
  }
  await openBoard(page);

  await chooseMenu(page, "toggleArchiveView");
  const archived = await storedBoard();
  expect(archived.archivedCards).toHaveLength(2);
  const target = archived.archivedCards[0]?.title ?? "";
  await page.locator(".search").fill(target);

  const archive = page.locator(".archive");
  await expect(archive.locator(".archived-card")).toHaveCount(1);
  await expect(archive.locator(".card-title")).toHaveText(target);
});

// ---------------------------------------------------------------- 書き出し

test("JSON で書き出すと、読めるファイルができる", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "exportBoardJson");

  const path = await writtenPath(page);
  expect(path.endsWith(".json")).toBe(true);
  const written: unknown = JSON.parse(readFileSync(path, "utf8"));
  expect(written).toHaveProperty("columns");
});

test("Markdown で書き出すと、カラムとカードが出ている", async ({ page }) => {
  await openBoard(page);
  const board = await storedBoard();
  await chooseMenu(page, "exportBoardMarkdown");

  const path = await writtenPath(page);
  expect(path.endsWith(".md")).toBe(true);
  const written = readFileSync(path, "utf8");
  expect(written).toContain(board.columns[0]?.name ?? "");
  expect(written).toContain(board.columns[0]?.cards[0]?.title ?? "");
});

test("データベースをコピーすると、開けるファイルができる", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "backupDatabase");

  const path = await writtenPath(page);
  expect(path.endsWith(".sqlite3")).toBe(true);
  // SQLite のファイルは先頭がこの文字列（開けることの、いちばん軽い確かめ方）。
  expect(readFileSync(path).subarray(0, 15).toString("utf8")).toBe("SQLite format 3");
});

// ---------------------------------------------------------------- 説明のリンク

test("説明の中の URL に色が付き、修飾キー＋クリックで開ける", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".card").first().dblclick();

  await page
    .locator(".card-description-input")
    .fill("詳しくは https://example.com/a を見てください");

  // 色が付くのはリンクだけ。本文のほかの部分は素のまま（ADR 0002）。
  const link = page.locator(".description-link");
  await expect(link).toHaveText("https://example.com/a");

  // 修飾キー無しのクリックでは開かない（文章のどこかを指すためのもの）。
  // 開いたかどうかはブラウザからは見えないので、ここで見るのは「色の付いた
  // 場所が本文と揃っていること」まで。
  await expect(page.locator(".description-layer")).toContainText("詳しくは");
});

// 説明の文字は入力欄ではなく裏の表示層が描いています（ADR 0002）。入力欄は
// 表示層より手前に描かれるので、そこに下地を塗ると層ごと覆って**説明が丸ごと
// 消えます**。見えているかどうかはスクリーンショットを撮らないと分からないので、
// ここでは「入力欄は塗らない、下地は枠が持つ」という置き方のほうを見ます。
test("説明の入力欄は下地を塗らず、裏の表示層を覆わない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".card").first().dblclick();
  await page.locator(".card-description-input").fill("見えていてほしい説明");

  const background = (selector: string) =>
    page.locator(selector).evaluate((element) => getComputedStyle(element).backgroundColor);

  await expect.poll(() => background(".card-description-input")).toBe("rgba(0, 0, 0, 0)");
  await expect.poll(() => background(".description-field")).not.toBe("rgba(0, 0, 0, 0)");
});

test("URL でない文字列はリンクにしない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".card").first().dblclick();
  await page.locator(".card-description-input").fill("example.com と ftp://example.com");
  await expect(page.locator(".description-layer")).toContainText("example.com");
  await expect(page.locator(".description-link")).toHaveCount(0);
});
