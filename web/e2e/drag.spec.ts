// §6 の受け入れ条件 1〜6 を、Chromium と WebKit の両方で確かめる。
//
// 条件 7（落としてから確定するまでに間が空かない）は SQLite と IPC の速さの
// 話で、エンジンでは変わらないので、ここでは測りません（Tauri で実測済み）。
// 条件 8 が問うているのは**エンジンの系統ごとの差**で、それがここで出ます。
//
// 動かしているのは本物の webview ではありません（ADR 0021）。platform 層の差
// ——macOS の慣性スクロールや跳ね返り——は、ここでは出ません。

import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test, type Locator, type Page } from "@playwright/test";

const ROOT = join(import.meta.dirname, "..", "..");
const HARNESS_PORT = 1421;

let harness: ChildProcess | undefined;
let workspace: string;

/// テストごとに新しい盤面から始める。
///
/// 1 つのデータベースを使い回すと、前のテストが動かしたカードの位置に次の
/// テストが引きずられます。**盤面の形が前提になっているテストは、順番に
/// 依存する**ので、そこは分けます（Rust 側の `Harness::open()` と同じ考え）。
test.beforeEach(async () => {
  workspace = mkdtempSync(join(tmpdir(), "ekanban-e2e-"));
  const database = join(workspace, "board.sqlite3");
  // 盤面は SQL ではなくアプリ自身の API で組み立てる。テストが見ている状態が、
  // アプリが本当に復元できる状態であることを、作り方の側で保証するため。
  execFileSync(
    "cargo",
    ["run", "-q", "-p", "ekanban", "--example", "manual_screenshot_seed", "board"],
    { cwd: ROOT, env: { ...process.env, EKANBAN_DATABASE: database }, stdio: "ignore" },
  );

  harness = spawn(
    "cargo",
    ["run", "-q", "-p", "ekanban-harness", "--", database, String(HARNESS_PORT)],
    { cwd: ROOT, stdio: "ignore" },
  );
  for (let i = 0; i < 120; i += 1) {
    try {
      const probe = await fetch(`http://127.0.0.1:${HARNESS_PORT}/invoke/snapshot`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: "{}",
      });
      if (probe.ok) return;
    } catch {
      // まだ立ち上がっていない。
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("ekanban-harness が立ち上がりませんでした");
});

test.afterEach(async () => {
  harness?.kill();
  harness = undefined;
  // 次のテストが同じポートを取れるまで待つ。
  await new Promise((resolve) => setTimeout(resolve, 250));
  rmSync(workspace, { recursive: true, force: true });
});

async function openBoard(page: Page) {
  await page.goto(`/?harness=http://127.0.0.1:${HARNESS_PORT}`);
  await expect(page.locator(".column").first()).toBeVisible();
}

/// 掴んで運ぶ。**HTML5 の drag events は使わない**ので、ポインタを自分で動かす
/// （ADR 0020）。1 回で飛ばすと掴んだと判定されないため、刻んで動かす。
async function dragTo(page: Page, from: { x: number; y: number }, to: { x: number; y: number }) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  const steps = 12;
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(
      from.x + ((to.x - from.x) * i) / steps,
      from.y + ((to.y - from.y) * i) / steps,
    );
    await page.waitForTimeout(16);
  }
}

async function boxOf(locator: Locator) {
  const box = await locator.boundingBox();
  if (box === null) throw new Error("画面に出ていません");
  return box;
}

async function centerOf(page: Page, selector: string) {
  const box = await boxOf(page.locator(selector));
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

/// 見えている並び。`[[カラム名, [カードのタイトル...]], ...]`
async function shape(page: Page): Promise<[string, string[]][]> {
  return page.$$eval(".column", (columns) =>
    columns.map(
      (column): [string, string[]] => [
        column.querySelector(".column-name")?.textContent ?? "",
        [...column.querySelectorAll(".card:not(.card-ghost) .card-title")].map(
          (title) => title.textContent,
        ),
      ],
    ),
  );
}

test("条件 1: 掴んだカードのゴーストがポインタについてくる", async ({ page }) => {
  await openBoard(page);
  const from = await centerOf(page, ".column >> nth=0 >> .card >> nth=0");
  await dragTo(page, from, { x: from.x + 260, y: from.y + 40 });

  // ゴーストは自分の要素。OS の描くものではない。
  const ghost = page.locator(".card-ghost");
  await expect(ghost).toBeVisible();
  const first = await boxOf(ghost);

  await page.mouse.move(from.x + 320, from.y + 120);
  await page.waitForTimeout(50);
  const second = await boxOf(ghost);
  await page.mouse.up();

  // ポインタが動いた分だけゴーストも動く。
  expect(second.x - first.x).toBeGreaterThan(40);
  expect(second.y - first.y).toBeGreaterThan(40);
});

test("条件 2: 落とす位置が、落とす前に見て分かる", async ({ page }) => {
  await openBoard(page);
  const before = await shape(page);
  const from = await centerOf(page, ".column >> nth=0 >> .card >> nth=0");
  const to = await centerOf(page, ".column >> nth=1 >> .card >> nth=0");
  await dragTo(page, from, to);

  // 離す前に、もう並びが組み替わっている。
  const during = await shape(page);
  expect(during).not.toEqual(before);
  expect(during[1]?.[1]?.[0]).toBe(before[0]?.[1]?.[0]);
  await page.mouse.up();
});

test("条件 4: 減光しているカードの位置にも落とせる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".search").fill("README");
  await expect(page.locator(".card[data-dimmed]").first()).toBeVisible();

  const before = await shape(page);
  const from = await centerOf(page, ".column >> nth=0 >> .card >> nth=0");
  const to = await centerOf(page, ".column >> nth=0 >> .card >> nth=2");
  await dragTo(page, from, to);
  await page.mouse.up();
  await expect.poll(async () => (await shape(page))[0]?.[1]?.[0]).not.toBe(before[0]?.[1]?.[0]);
});

test("条件 5: カラムそのものも、カードと同じ操作感で並べ替えられる", async ({ page }) => {
  await openBoard(page);
  const before = await shape(page);
  // 掴むのはヘッダ。カードの上でカラムのドラッグが始まると、1 枚動かす
  // つもりが列ごと動く。
  const from = await centerOf(page, ".column >> nth=1 >> .column-header");
  const to = await centerOf(page, ".column >> nth=0 >> .column-header");
  await dragTo(page, from, to);
  await expect(page.locator(".column-ghost")).toBeVisible();
  await page.mouse.up();

  await expect
    .poll(async () => (await shape(page)).map(([name]) => name))
    .toEqual([before[1]?.[0], before[0]?.[0], ...before.slice(2).map(([name]) => name)]);
});

test("条件 6: キーボードでもカードを動かせる", async ({ page }) => {
  await openBoard(page);
  const before = await shape(page);

  await page.locator(".column").first().locator(".card").first().click();
  await expect(page.locator(".card[data-selected]")).toHaveCount(1);

  // `secondary`＋`alt`＋矢印。どの修飾キーかを決めるのは **Rust が返す
  // platform** で、ブラウザの UA ではない。ハーネスはこの機械で動いている
  // ので、Playwright の Safari 模擬が `Macintosh` を名乗っても Ctrl のまま。
  const secondary = process.platform === "darwin" ? "Meta" : "Control";
  await page.keyboard.press(`${secondary}+Alt+ArrowDown`);
  await expect
    .poll(async () => (await shape(page))[0]?.[1]?.[0])
    .toBe(before[0]?.[1]?.[1]);

  await page.keyboard.press(`${secondary}+Alt+ArrowRight`);
  await expect.poll(async () => (await shape(page))[1]?.[1]?.length).toBeGreaterThan(
    before[1]?.[1]?.length ?? 0,
  );
});

test("矢印だけなら選択が動き、盤面は動かない", async ({ page }) => {
  await openBoard(page);
  const before = await shape(page);
  await page.locator(".column").first().locator(".card").first().click();
  await page.keyboard.press("ArrowDown");
  await page.waitForTimeout(200);
  expect(await shape(page)).toEqual(before);
  await expect(page.locator(".card[data-selected]")).toHaveCount(1);
});

test("入力欄にいる間は、盤面の割り当てを取らない", async ({ page }) => {
  await openBoard(page);
  const before = await shape(page);
  await page.locator(".column").first().locator(".card").first().click();
  await page.locator(".search").click();
  const secondary = process.platform === "darwin" ? "Meta" : "Control";
  await page.keyboard.press(`${secondary}+Alt+ArrowDown`);
  await page.waitForTimeout(200);
  expect(await shape(page)).toEqual(before);
});
