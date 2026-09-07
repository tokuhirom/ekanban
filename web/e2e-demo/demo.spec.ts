// ブラウザ版が、ブラウザの中だけで動いていること（[ADR 0035]）。
//
// ここで動いているのは **`wasm32-unknown-unknown` に組み直した本物の
// `ekanban-core`** で、SQLite のファイルは `localStorage` にあります。ハーネス
// 越しの e2e（`e2e/`）と違って、確かめる相手は HTTP の向こうではなく、この
// ページの中です——だから**読み込み直しても残っていること**が、ここでしか
// 見られない受け入れ条件になります。
//
// 盤面の振る舞いそのものは、ここで数え直しません。同じ `Board` と同じ
// `ekanban-core` が動いているので、`e2e/` が見ているものがそのまま効きます。
// ここで見るのは、**組み立ての違いから来るところだけ**です。
//
// [ADR 0035]: ../../docs/adr/0035-a-browser-build-of-the-real-core.md

import { expect, test, type Page } from "@playwright/test";

/// `localStorage` に置いてあるデータベース（base64）の大きさ。
async function storedSize(page: Page): Promise<number> {
  return page.evaluate(() => localStorage.getItem("ekanban:database")?.length ?? 0);
}

async function openDemo(page: Page): Promise<void> {
  await page.goto("/");
  // wasm を読み込んで種を蒔くまで。既定の盤面が出たら起動できている。
  await expect(page.getByRole("heading", { name: "個人 Kanban" })).toBeVisible({ timeout: 30_000 });
}

test("起動すると、種を蒔いた盤面が localStorage に残る", async ({ page }) => {
  await openDemo(page);
  // ここが 0 のままなら、動いてはいても何も残っていない——次に開くと消えます。
  expect(await storedSize(page)).toBeGreaterThan(0);
});

test("足したカードは、読み込み直しても残っている", async ({ page }) => {
  await openDemo(page);
  await page.getByRole("button", { name: "カードを追加" }).first().click();
  await page.keyboard.type("ブラウザ版で足したカード");
  await page.keyboard.press("Enter");
  await expect(page.getByText("ブラウザ版で足したカード")).toBeVisible();

  await page.reload();
  await expect(page.getByText("ブラウザ版で足したカード")).toBeVisible({ timeout: 30_000 });
});

test("メニューバーは Rust が決めた構成のまま出る", async ({ page }) => {
  await openDemo(page);
  await page.getByRole("menuitem", { name: "表示" }).click();

  const menu = page.getByRole("menu", { name: "表示" });
  await expect(menu.getByRole("menuitem", { name: "アーカイブ表示を切り替え" })).toBeVisible();
  // OS が持っている項目（カット・コピー・終了）は、ブラウザに相手がいないので
  // 出しません。「編集」を開いても並ばないこと。
  await page.getByRole("menuitem", { name: "編集" }).click();
  await expect(page.getByRole("menu", { name: "編集" }).getByText("ペースト")).toHaveCount(0);
});

test("ブラウザにできないことは、消さずに灰色にする", async ({ page }) => {
  await openDemo(page);
  await page.getByRole("menuitem", { name: "ヘルプ" }).click();

  const help = page.getByRole("menu", { name: "ヘルプ" });
  // 消すと「この機能はこのアプリに無い」に見えます（`crates/app/src/menu.rs`）。
  await expect(help.getByRole("menuitem", { name: /データベースの場所/ })).toBeDisabled();
  // ダウンロードはブラウザにもある。ここまで灰色にしない。
  await expect(help.getByRole("menuitem", { name: "データベースをコピー…" })).toBeEnabled();
});

test("書き出しは、ダウンロードとして受け取れる", async ({ page }) => {
  await openDemo(page);
  await page.getByRole("menuitem", { name: "ファイル" }).click();

  const download = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "ボードを書き出す（Markdown）" }).click();

  // **中身まで見ます。** ファイル名だけを見ても、組み立てたのが Rust の
  // `export::render_board_markdown` かどうかは分かりません。
  const stream = await (await download).createReadStream();
  let markdown = "";
  stream.setEncoding("utf8");
  for await (const chunk of stream) markdown += String(chunk);
  expect(markdown).toContain("# 個人 Kanban");

  // 書けた場所を「開く」導線は出しません——開く相手がいないので（ADR 0035）。
  await expect(page.getByRole("button", { name: "場所を開く" })).toHaveCount(0);
});

test("メニューの割り当ては、ページが受ける", async ({ page }) => {
  await openDemo(page);
  // 配るアプリではこれを OS のメニューバーが取ります。ブラウザには
  // メニューバーが無いので、`shell/accelerator.ts` が受けます。
  await page.keyboard.press("Control+Shift+A");
  await expect(page.getByRole("button", { name: "ボードへ戻る" })).toBeVisible();
});
