// ブラウザ版が、ブラウザの中だけで動いていること（[ADR 0035]、[ADR 0042]）。
//
// **動いているのはアプリと同じ TypeScript**で、置き場所だけが `localStorage`
// に差し替わっています。だから盤面の振る舞いをここで数え直しません——`e2e/`
// が見ているものがそのまま効きます。
//
// ここで見るのは、**組み立ての違いから来るところだけ**です。組み立てたものを
// `vite preview` で出して叩くので、`e2e/` が見ていない「配る形になったときに
// 壊れていないか」も一緒に通ります。
//
// [ADR 0035]: ../../docs/adr/0035-a-browser-build-of-the-real-core.md
// [ADR 0042]: ../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

import { expect, test, type Page } from "@playwright/test";

/// `localStorage` に置いてある盤面（JSON）の大きさ。
async function storedSize(page: Page): Promise<number> {
  return page.evaluate(() => localStorage.getItem("ekanban:board")?.length ?? 0);
}

async function openDemo(page: Page): Promise<void> {
  await page.goto("/");
  // 種を蒔くまで。既定の盤面が出たら起動できている。
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

test("メニューバーは、配るアプリと同じ構成のまま出る", async ({ page }) => {
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
  // ブラウザ版に SQLite のファイルはありません（ADR 0036）。
  await expect(help.getByRole("menuitem", { name: /データベースをコピー/ })).toBeDisabled();
});

test("書き出しは、ダウンロードとして受け取れる", async ({ page }) => {
  await openDemo(page);
  await page.getByRole("menuitem", { name: "ファイル" }).click();

  // 置き場所が JSON になっても、持ち出しは残ります（ADR 0036）。
  await expect(
    page.getByRole("menuitem", { name: "ボードを書き出す（Markdown）" }),
  ).toBeEnabled();

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
