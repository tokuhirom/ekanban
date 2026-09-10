// 設定画面（`docs/DESIGN.md`「画面の作り」、[ADR 0047]）。
//
// **メニューバーそのものはここに出ません。** 「設定…」を描くのは OS で、
// 押されたことは Rust が `app:action` で流します。ハーネスには
// `window.ekanbanMenu` という口だけが開いていて（`src/ipc/browser.ts`）、
// 押されたことにできます。`CmdOrCtrl+,` が本当に効くところは、殻の煙テストの
// 担当です（`docs/DESIGN.md`「テスト」）。
//
// 割り当てを捕まえるところも、ここには出ません。**この組み立てでは欄が灰色**
// だからで（ページの外まで届く割り当てを作れない）、そこは手で確かめます
// （[ADR 0041] が殻の側に残した 4 つのうちの 1 つ）。押されたキーを文字列に
// するところは `src/shell/shortcut.test.ts` が見ています。
//
// [ADR 0041]: ../../docs/adr/0041-one-layer-of-screen-tests.md
// [ADR 0047]: ../../docs/adr/0047-app-settings-live-in-a-settings-dialog.md

import { expect, test, type Page } from "@playwright/test";

import { chooseMenu, openBoard, startHarness, stopHarness, storedSetting } from "./harness";
import { DAY_BOUNDARY_HOUR, QUICK_CAPTURE_SHORTCUT, THEME_PREFERENCE } from "../src/store/keys";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

/// 「設定…」が押されたことにして、開くまで待つ。
async function openSettings(page: Page) {
  await chooseMenu(page, "openSettings");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  return dialog;
}

test("「設定…」から開き、テーマを変えるとその場で反映され、閉じても保たれる", async ({
  page,
}) => {
  await openBoard(page);
  const dialog = await openSettings(page);

  // 保存ボタンは無い。押した時点で確定する（ADR 0032 と同じ流儀）。
  await dialog.getByRole("radio", { name: "ダークモード" }).check();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect.poll(() => storedSetting(page, THEME_PREFERENCE)).toBe("dark");

  // 閉じて開き直しても、選択はそのまま。
  await dialog.getByRole("button", { name: "閉じる" }).click();
  await expect(dialog).toBeHidden();
  const again = await openSettings(page);
  await expect(again.getByRole("radio", { name: "ダークモード" })).toBeChecked();

  // 「システムに合わせる」は属性を外すだけ。判定は CSS が持つ。
  await again.getByRole("radio", { name: "システムに合わせる" }).check();
  await expect(page.locator("html")).not.toHaveAttribute("data-theme", /.*/);
  await expect.poll(() => storedSetting(page, THEME_PREFERENCE)).toBe("system");
});

/// 日付の切り替わり（#197、[ADR 0048]）。
///
/// 選んだ時点で確定し、開き直しても保たれます。**基準日が動くところ**は
/// `day.spec.ts` の担当で、ここは設定として往復することだけを見ます。
///
/// [ADR 0048]: ../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md
test("日付の切り替わりを選ぶと、その場で覚えられ、開き直しても保たれる", async ({ page }) => {
  await openBoard(page);
  const dialog = await openSettings(page);

  const boundary = dialog.getByLabel("日付の切り替わり");
  // 既定は午前 4 時。まだ何も選んでいなくても、選択肢としては出ている。
  await expect(boundary).toHaveValue("4");

  await boundary.selectOption("0");
  await expect.poll(() => storedSetting(page, DAY_BOUNDARY_HOUR)).toBe("0");

  await dialog.getByRole("button", { name: "閉じる" }).click();
  await expect(dialog).toBeHidden();
  const again = await openSettings(page);
  await expect(again.getByLabel("日付の切り替わり")).toHaveValue("0");
});

test("`Escape` でも閉じる", async ({ page }) => {
  await openBoard(page);
  const dialog = await openSettings(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

/// ページの外まで届く割り当ては、この組み立てでは作れない（ADR 0035）。
///
/// **消さずに、理由を出します。** 何ができないのかを画面で読めることが
/// 決めごとで、押せるのに何も起きない状態を作らないためです。以前はメニュー
/// 項目そのものが灰色でしたが、テーマは選べるので、灰色になるのはこの欄だけに
/// なりました（ADR 0047）。
test("ブラウザでは、割り当ての欄が灰色で、理由がその場に出る", async ({ page }) => {
  await openBoard(page);
  const dialog = await openSettings(page);

  const capture = dialog.locator(".shortcut-capture");
  await expect(capture).toHaveClass(/is-disabled/);
  await expect(capture).toHaveAttribute("aria-disabled", "true");
  await expect(capture).toContainText("ブラウザ版");
  await expect(dialog.getByRole("button", { name: "解除" })).toBeDisabled();

  // 押しても割り当てにはならない。閉じもしない——読む相手はこの理由なので。
  await page.keyboard.press("KeyK");
  await expect(dialog).toBeVisible();
  expect(await storedSetting(page, QUICK_CAPTURE_SHORTCUT)).toBeNull();
});

/// 入れ先は**出すだけ**。選ぶのはカラムの `…` のまま（ADR 0047）。
test("入れ先は、いまどのボードのどのカラムかを出す", async ({ page }) => {
  await openBoard(page);
  const dialog = await openSettings(page);
  // 既定は先頭のボードの先頭カラム（ADR 0028）。
  await expect(dialog).toContainText("個人 Kanban / やること");

  await dialog.getByRole("button", { name: "閉じる" }).click();
  // カラムの `…` から入れ先を移すと、設定画面の表示も追う。
  const second = page.locator(".column").nth(1);
  await second.locator(".column-menu-button").click();
  await second.getByRole("button", { name: "クイックキャプチャ先にする" }).click();

  const again = await openSettings(page);
  await expect(again).toContainText("個人 Kanban / 進行中");
});
