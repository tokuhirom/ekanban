// アーカイブ、書き出し、控えの保存、説明のリンク。
//
// 「書き出したファイルが読める」を確かめるので、**受け取ったファイルを読み直し
// ます**。画面に「書き出しました」と出ているだけでは、書けていないことに
// 気づけません。
//
// ファイルの受け取り方だけが本物と違います。ブラウザに OS の保存ダイアログと
// 書き込み先が無いので、名前を決めてダウンロードになります
// （`src/ipc/browser.ts`）。**書き出す中身を組み立てるところは同じ**です。

import { readFileSync } from "node:fs";

import { expect, test, type Page } from "@playwright/test";

import { editStoredBoard, openBoard, startHarness, stopHarness, storedBoard } from "./harness";
import { archiveCard } from "../src/model/board";

import type {} from "../src/ipc/browser";
import type { AppAction } from "../src/ipc/types/AppAction";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

async function chooseMenu(page: Page, action: AppAction): Promise<void> {
  await page.evaluate((name: AppAction) => {
    window.ekanbanMenu?.(name);
  }, action);
}

/// メニューを選び、受け取ったファイルの中身を読む。
///
/// **知らせのダイアログも見ます**——書けたことを画面が言うところまでが、
/// 書き出しの受け入れ条件です（`docs/DESIGN.md`「アプリが伝えること」）。
async function exported(page: Page, action: AppAction): Promise<{ name: string; body: string }> {
  const receiving = page.waitForEvent("download");
  await chooseMenu(page, action);
  const download = await receiving;

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.locator(".dialog-title")).toHaveText("書き出しました");

  // **名前は知らせのダイアログから読みます。** ブラウザが降ろすファイルに
  // 付ける名前は環境が決めるもので、アプリが決めた行き先はこちらに出ます。
  const name = (await dialog.locator(".dialog-detail").innerText()).trim();
  return { name, body: readFileSync(await download.path(), "utf8") };
}

// ---------------------------------------------------------------- アーカイブ

test("アーカイブしたカードが日ごとに並び、復元でボードへ戻る", async ({ page }) => {
  await openBoard(page);
  const first = page.locator(".column").first().locator(".card").first();
  const title = await first.locator(".card-title").innerText();
  await first.click({ button: "right" });
  await page.locator(".card-menu").getByRole("button", { name: "アーカイブ", exact: true }).click();

  await expect.poll(async () => (await storedBoard(page)).archivedCards.length).toBe(1);

  await chooseMenu(page, "toggleArchiveView");
  const archive = page.locator(".archive");
  await expect(archive).toBeVisible();
  await expect(archive.locator(".archived-card")).toHaveCount(1);
  await expect(archive.locator(".archive-day-label")).toHaveCount(1);
  // カラムはもう出ていない。盤面の代わりに並べる（ADR 0010）。
  await expect(page.locator(".column")).toHaveCount(0);

  await archive.locator(".restore-card").click();
  await expect.poll(async () => (await storedBoard(page)).archivedCards.length).toBe(0);
  await expect.poll(async () => (await storedBoard(page)).columns[0]?.cards.at(-1)?.title).toBe(title);
});

test("アーカイブでは、絞り込みに外れたカードを隠す", async ({ page }) => {
  // 2 枚アーカイブしてから開き直す。ここで確かめたいのは絞り込みの効き方で、
  // アーカイブする道はもう上のテストが通っている。
  await openBoard(page);
  const board = await storedBoard(page);
  for (const card of board.columns[0]?.cards.slice(0, 2) ?? []) {
    await editStoredBoard(page, (document) => archiveCard(document, card.id));
  }
  await page.reload();

  await chooseMenu(page, "toggleArchiveView");
  const archived = await storedBoard(page);
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

  const written = await exported(page, "exportBoardJson");
  expect(written.name.endsWith(".json")).toBe(true);
  const parsed: unknown = JSON.parse(written.body);
  expect(parsed).toHaveProperty("columns");
  // 置いてある形の写しなので、カードの履歴まで入る（ADR 0045）。
  expect(parsed).toHaveProperty("card_events");
});

test("Markdown で書き出すと、カラムとカードが出ている", async ({ page }) => {
  await openBoard(page);
  const board = await storedBoard(page);

  const written = await exported(page, "exportBoardMarkdown");
  expect(written.name.endsWith(".md")).toBe(true);
  expect(written.body).toContain(board.columns[0]?.name ?? "");
  expect(written.body).toContain(board.columns[0]?.cards[0]?.title ?? "");
});

/// 控えは置いてあるものを丸ごと。
///
/// **この組み立てに SQLite のファイルはありません**（ADR 0036）。置いてあるのは
/// 盤面の JSON なので、それがそのまま降りてきます。SQLite のファイルが開ける
/// ことは Rust 側のテストが見ます（`a_backup_is_a_database_that_opens`）。
test("控えを保存すると、置いてあるものが丸ごと降りてくる", async ({ page }) => {
  await openBoard(page);
  const board = await storedBoard(page);

  const receiving = page.waitForEvent("download");
  await chooseMenu(page, "backupDatabase");
  const download = await receiving;
  const body = readFileSync(await download.path(), "utf8");

  const parsed = JSON.parse(body) as { boards: { name: string }[] };
  expect(parsed.boards.map((each) => each.name)).toContain(board.name);
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
      (await storedBoard(page)).columns
        .flatMap((column) => column.cards)
        .find((card) => card.title === "太字のカード")?.description,
    )
    .toBe("これは **太字** です");
});

test("箇条書きは、何も打たずに `Enter` を押すと終わる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("箇条書きのカード");

  const description = page.locator(".card-description-input");
  await description.click();
  await page.keyboard.type("- ひとつめ");
  await page.keyboard.press("Enter");
  await page.keyboard.type("ふたつめ");
  // 何も打っていない項目で押すと、そこで箇条書きが終わります。
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("ここは段落");

  await expect(description.locator("li")).toHaveCount(2);
  await expect(description.locator("p").last()).toHaveText("ここは段落");

  await page.locator(".save-card").click();
  await expect
    .poll(async () =>
      (await storedBoard(page)).columns
        .flatMap((column) => column.cards)
        .find((card) => card.title === "箇条書きのカード")?.description,
    )
    .toBe("- ひとつめ\n- ふたつめ\n\nここは段落");
});

test("`- [ ]` はチェック項目になり、印を押すと `- [x]` で保存される", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("チェックのカード");

  const description = page.locator(".card-description-input");
  await description.click();
  await page.keyboard.type("- [ ] 買い物");

  // 記法は残らず、印そのものになります。
  const item = description.locator("li[role=checkbox]");
  await expect(item).toHaveText("買い物");
  await expect(item).toHaveAttribute("aria-checked", "false");

  // 押すのは印の上だけ。文字のところを押すと、そこにカーソルが入ります。
  await item.click({ position: { x: 5, y: 8 } });
  await expect(item).toHaveAttribute("aria-checked", "true");

  await page.locator(".save-card").click();
  await expect
    .poll(async () =>
      (await storedBoard(page)).columns
        .flatMap((column) => column.cards)
        .find((card) => card.title === "チェックのカード")?.description,
    )
    .toBe("- [x] 買い物");
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
