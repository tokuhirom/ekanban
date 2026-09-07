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

// ---------------------------------------------------------------- 説明の Markdown

/// 説明は Markdown のエディタ（#129、ADR 0033）。**打ちながら整い、保存される
/// のは Markdown の文字列**です。
///
/// 説明が空の新しいカードで打ちます。出来合いのカードには説明が入っていて、
/// 選び直してから打つと、打ち始めの 1 文字と選択の入れ替わりが重なります。
test("`**` で打った太字が、そのまま Markdown で保存される", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("太字のカード");

  const description = page.locator(".card-description-input");
  await description.click();
  await page.keyboard.type("これは **太字** です");

  // 画面では太字。記法の `**` は残らない。
  await expect(description.locator(".description-bold")).toHaveText("太字");
  await expect(description).not.toContainText("**");

  await page.locator(".save-card").click();
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.title === "太字のカード")?.description,
    )
    .toBe("これは **太字** です");
});

test("打った URL がリンクになり、修飾キー無しでは開かない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("リンクのカード");

  const description = page.locator(".card-description-input");
  await description.click();
  await page.keyboard.type("詳しくは https://example.com/a を見てください");

  const link = description.locator(".description-link");
  await expect(link).toHaveText("https://example.com/a");
  await expect(link).toHaveAttribute("href", "https://example.com/a");

  // 修飾キー無しのクリックでは開かない（文章のどこかを指すためのもの）。
  // 開く先は Rust の `open_url` で、開いてよい形かはあちらが決める。
  await link.click();
  await expect(description).toContainText("詳しくは");
});

test("URL でない文字列はリンクにしない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("リンクでないカード");

  const description = page.locator(".card-description-input");
  await description.click();
  await page.keyboard.type("example.com と ftp://example.com");
  await expect(description).toContainText("example.com");
  await expect(description.locator(".description-link")).toHaveCount(0);
});
