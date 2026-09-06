// 段階 6 の受け入れ条件——メニューの行き先、テーマ、取り消しの振り分け
// （`docs/TAURI-MIGRATION.md` §7、§12）。
//
// **メニューバーそのものはここに出ません。** 描くのは OS で、押されたことは
// Rust が `app:action` で流します。ここで確かめるのは、その先——受け取った
// webview が何をするかです。ハーネスには `window.ekanbanMenu` という口だけが
// 開いていて（`src/ipc/harness.ts`）、押されたことにできます。
//
// 本物のメニューが出て、押すとこの口に届くところは、殻の煙テストの担当です
// （§10）。

import { expect, test, type Page } from "@playwright/test";

import { invoke, openBoard, startHarness, stopHarness } from "./harness";

// `window.ekanbanMenu` の宣言を読み込むためだけの取り込み（値は使わない）。
import type {} from "../src/ipc/harness";
import type { AppAction } from "../src/ipc/types/AppAction";
import type { Snapshot } from "../src/ipc/types/Snapshot";
import type { StartupState } from "../src/ipc/types/StartupState";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

/// メニューが押されたことにする。
async function chooseMenu(page: Page, action: AppAction): Promise<void> {
  await page.evaluate((name: AppAction) => {
    window.ekanbanMenu?.(name);
  }, action);
}

async function storedStartup(): Promise<StartupState> {
  const response = await invoke("startup_state");
  return (await response.json()) as StartupState;
}

async function storedTitles(): Promise<string[]> {
  const response = await invoke("snapshot");
  const snapshot = (await response.json()) as Snapshot;
  return snapshot.board.columns.flatMap((column) => column.cards.map((card) => card.title));
}

test("「カードを追加」は、選んでいるカードのカラムに下書きを開く", async ({ page }) => {
  await openBoard(page);
  // 2 つめのカラムのカードを選んでおく。
  const second = page.locator(".column").nth(1);
  await second.locator(".card").first().click();

  await chooseMenu(page, "addCard");

  await expect(page.locator(".card-panel")).toBeVisible();
  const columnName = await second.locator(".column-name").innerText();
  await expect(page.locator(".panel-context")).toHaveText(`${columnName} のカード`);
});

test("「カラムを追加」は、名前の入力欄を開く", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "addColumn");
  await expect(page.locator(".new-column-name")).toBeVisible();

  // 「編集をキャンセル」で畳む。打ちかけは残さない。
  await page.locator(".new-column-name").fill("やめるカラム");
  await chooseMenu(page, "cancelEdit");
  await expect(page.locator(".new-column-name")).toBeHidden();
  await chooseMenu(page, "addColumn");
  await expect(page.locator(".new-column-name")).toHaveValue("");
});

test("「タグを整理…」と「タグを追加」は、どちらもタグのパネルを開く", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "manageTags");
  await expect(page.locator(".tag-panel")).toBeVisible();

  // 開いているところをもう一度押しても畳まない（トグルではない）。
  await chooseMenu(page, "addTag");
  await expect(page.locator(".tag-panel")).toBeVisible();

  await chooseMenu(page, "cancelEdit");
  await expect(page.locator(".tag-panel")).toBeHidden();
});

test("「検索にフォーカス」と「検索をクリア」が検索欄に届く", async ({ page }) => {
  await openBoard(page);
  await page.locator(".search").fill("設計");
  await expect(page.locator(".search")).toHaveValue("設計");

  await chooseMenu(page, "clearSearch");
  await expect(page.locator(".search")).toHaveValue("");
  // 絞り込みは `app_state` に残る。画面だけ消して覚えたままにしない。
  await expect.poll(async () => (await storedStartup()).filter.search).toBe("");

  await chooseMenu(page, "focusSearch");
  await expect(page.locator(".search")).toBeFocused();
});

test("「ボード一覧の表示を切り替え」で畳み、次の起動でも畳んだまま", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "toggleBoardList");

  await expect(page.locator(".sidebar")).toHaveAttribute("data-collapsed", "true");
  await expect.poll(async () => (await storedStartup()).sidebarCollapsed).toBe(true);
});

test("テーマを選ぶと画面が切り替わり、覚えられる", async ({ page }) => {
  await openBoard(page);

  await chooseMenu(page, "useDarkTheme");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect.poll(async () => (await storedStartup()).theme).toBe("dark");

  await chooseMenu(page, "useLightTheme");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");

  // 「システムに合わせる」は属性を外すだけ。判定は CSS が持つ。
  await chooseMenu(page, "useSystemTheme");
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/);
  await expect.poll(async () => (await storedStartup()).theme).toBe("system");
});

test("「元に戻す」は盤面を巻き戻し、「やり直す」で戻る", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("取り消されるカード");
  await page.locator(".save-card").click();
  await expect.poll(storedTitles).toContain("取り消されるカード");

  await chooseMenu(page, "undo");
  await expect.poll(storedTitles).not.toContain("取り消されるカード");

  await chooseMenu(page, "redo");
  await expect.poll(storedTitles).toContain("取り消されるカード");
});

test("入力欄で Ctrl+Z を打っても、盤面は巻き戻らない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("残るカード");
  await page.locator(".save-card").click();
  await expect.poll(storedTitles).toContain("残るカード");

  // 説明を打っている最中の取り消しは、その欄のもの。盤面まで戻ると、書いて
  // いた行が消えたように見える（§7）。
  await page.locator(".card", { hasText: "残るカード" }).dblclick();
  await page.locator(".card-description-input").fill("打ちかけの説明");
  await page.locator(".card-description-input").press("ControlOrMeta+z");

  await expect(page.locator(".card-panel")).toBeVisible();
  expect(await storedTitles()).toContain("残るカード");
});

test("盤面の上の Ctrl+Z は盤面を巻き戻す", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("キーで取り消すカード");
  await page.locator(".save-card").click();
  await expect.poll(storedTitles).toContain("キーで取り消すカード");

  await page.locator(".board-content").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("ControlOrMeta+z");
  await expect.poll(storedTitles).not.toContain("キーで取り消すカード");
});

test("「ekanbanについて」はダイアログを出す", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "about");
  await expect(page.getByRole("dialog")).toContainText("ekanbanについて");
});
