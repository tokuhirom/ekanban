// クイックキャプチャ（`docs/DESIGN.md`「クイックキャプチャ」）。
//
// **ホットキーそのものはここに出ません。** グローバルな割り当ては OS が押しかたを
// 捕まえるもので、ブラウザからは押せません（そもそも Wayland では使えない、
// [ADR 0012]）。ここで確かめるのは、押されたあとの窓と、入れ先の決まり方、
// 割り当ての読み書きです。
//
// キャプチャの窓は**別のエントリポイント**（`capture.html`）なので、そのまま
// 開けます。閉じるところだけが本物ではありません——ブラウザに閉じる窓が
// ありません。
//
// [ADR 0012]: ../../../docs/adr/0012-focus-after-quick-capture-on-linux.md

import { expect, test, type Page } from "@playwright/test";

import { HARNESS_PORT, invoke, openBoard, startHarness, stopHarness } from "./harness";

import type {} from "../src/ipc/harness";
import type { AppAction } from "../src/ipc/types/AppAction";
import type { Snapshot } from "../src/ipc/types/Snapshot";
import type { StartupState } from "../src/ipc/types/StartupState";

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

async function storedStartup(): Promise<StartupState> {
  const response = await invoke("startup_state");
  return (await response.json()) as StartupState;
}

async function openCapture(page: Page): Promise<void> {
  await page.goto(`/capture.html?harness=http://127.0.0.1:${HARNESS_PORT}`);
  await expect(page.locator(".capture-input")).toBeVisible();
}

test("1 行を打って Enter で、入れ先のカラムの末尾に足される", async ({ page }) => {
  const before = await storedBoard();
  const target = before.columns[0];

  await openCapture(page);
  // どこに入るのかを常に見せる。決まっていなければ既定（先頭カラム）。
  await expect(page.locator(".capture-destination")).toHaveText(
    `${before.name} / ${target?.name ?? ""}`,
  );

  await page.locator(".capture-input").fill("思いついたこと");
  await page.locator(".capture-input").press("Enter");

  await expect
    .poll(async () => (await storedBoard()).columns[0]?.cards.at(-1)?.title)
    .toBe("思いついたこと");
});

test("空のまま Enter を押しても、何も足さない", async ({ page }) => {
  const before = (await storedBoard()).columns[0]?.cards.length ?? 0;
  await openCapture(page);
  await page.locator(".capture-input").press("Enter");
  // 何も言わずに何も起きない（拒否は黙る、`docs/DESIGN.md`）。
  await expect(page.locator(".capture-hint")).toHaveText("Enter で追加、Escape で閉じる");
  expect((await storedBoard()).columns[0]?.cards.length ?? 0).toBe(before);
});

test("入れ先を選ぶと、そのカラムに印が出て、キャプチャもそこへ入る", async ({ page }) => {
  await openBoard(page);
  const second = page.locator(".column").nth(1);
  await second.locator(".column-menu-button").click();
  await second.locator(".set-capture-column").click();

  await expect(second.locator(".column-capture")).toBeVisible();
  await expect(page.locator(".column").first().locator(".column-capture")).toHaveCount(0);

  await openCapture(page);
  await page.locator(".capture-input").fill("2 つめのカラムへ");
  await page.locator(".capture-input").press("Enter");

  await expect
    .poll(async () => (await storedBoard()).columns[1]?.cards.at(-1)?.title)
    .toBe("2 つめのカラムへ");
});

test("ほかの窓が盤面を変えたら、開いているボードにも出る", async ({ page }) => {
  await openBoard(page);
  // キャプチャの窓が書いたことにする。届く経路（`board:changed`）は本物では
  // Rust が投げるので、ここでは届いたあとの差し替えだけを見る。
  const response = await invoke("capture_card", { title: "別の窓から足したカード" });
  const snapshot = (await response.json()) as Snapshot;
  await page.evaluate((payload: Snapshot) => {
    window.ekanbanBoardChanged?.(payload);
  }, snapshot);

  await expect(page.locator(".card", { hasText: "別の窓から足したカード" })).toBeVisible();
});

test("割り当てを記録して、解除できる", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "setQuickCaptureShortcut");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();

  // 押された組み合わせが、そのままの形で `app_state` に入る。
  await page.keyboard.press("Control+Alt+KeyK");
  await expect.poll(async () => (await storedStartup()).quickCaptureShortcut).toBe("ctrl-alt-k");

  await chooseMenu(page, "setQuickCaptureShortcut");
  await page.locator(".clear-shortcut").click();
  await expect.poll(async () => (await storedStartup()).quickCaptureShortcut).toBeNull();
});

test("修飾キーの無い割り当ては断られ、ダイアログはそのまま", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "setQuickCaptureShortcut");
  await expect(page.getByRole("dialog")).toBeVisible();

  await page.keyboard.press("KeyK");

  // 打ち直せるように、閉じずにその場で理由を出す。
  await expect(page.getByRole("dialog")).toBeVisible();
  await expect(page.locator(".field-error")).toContainText("修飾キー");
  expect((await storedStartup()).quickCaptureShortcut).toBeNull();
});
