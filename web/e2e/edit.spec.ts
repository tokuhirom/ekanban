// カードの編集を、Playwright ＋ ハーネスで通す。
//
// 見るのは**画面の状態と SQLite に書かれた内容の両方**です。`invoke()` でハーネスを
// 直接叩いて保存された盤面を読み直します。「画面に出ている」だけでは、保存の
// 配線が抜けていても気づけません。
//
// 動かしているのは本物の webview ではありません（ADR 0021）。エンジンの系統
// （Chromium と WebKit）の差はここで出ますが、platform 層の差は出ません。

import { expect, test, type Locator, type Page } from "@playwright/test";

import { localDay } from "../src/state/day";
import type { StartupState } from "../src/ipc/types/StartupState";
import {
  editStoredBoard,
  openBoard,
  startHarness,
  stopHarness,
  storedBoard,
  storedStartup,
} from "./harness";
import { deleteCard, removeColumn } from "../src/model/board";


test.beforeEach(startHarness);
test.afterEach(stopHarness);

/// 保存された盤面を読み直す。画面ではなく SQLite の側を見るための口。
/// 保存された絞り込みを読み直す。覚えているかどうかは `app_state` の側で見る。
async function storedFilter(): Promise<StartupState["filter"]> {
  return (await storedStartup()).filter;
}

async function storedTitles(): Promise<string[]> {
  const board = await storedBoard();
  return board.columns.flatMap((column) => column.cards.map((card) => card.title));
}

/// 1 枚目のカードの編集パネルを開く。クリックは選ぶだけなので、開くのは
/// ダブルクリック（`Card.tsx`）。
async function openFirstCard(page: Page) {
  await page.locator(".column").first().locator(".card").first().dblclick();
  await expect(page.locator(".card-panel")).toBeVisible();
}

/// 欄の枠と、置かれている場所を読む。**触る前と後で見比べる**ためのもの（#168）。
async function fieldFrame(locator: Locator) {
  return await locator.evaluate((element) => {
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return {
      borderWidth: style.borderTopWidth,
      borderColor: style.borderTopColor,
      // 小数の丸めでちらつかせない。動いたかどうかだけが見たい。
      frame: {
        left: Math.round(rect.left),
        top: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      },
    };
  });
}

/// IME の変換に伴う `keydown` を流す。
///
/// Playwright は本物の IME を打てないので、**WebKit が変換を確定するときに
/// 寄こす形**をそのまま作る——`compositionend` が先に出ているので
/// `isComposing` は `false`、変換の名残りは `keyCode` の 229 だけ（#124、
/// `web/src/shell/ime.ts`）。`KeyboardEventInit` の `keyCode` は非推奨なので、
/// 作ってから生やす。
async function pressWhileComposing(field: Locator, key: string) {
  await field.evaluate((element, pressed) => {
    const event = new KeyboardEvent("keydown", { key: pressed, bubbles: true, cancelable: true });
    Object.defineProperty(event, "keyCode", { get: () => 229 });
    element.dispatchEvent(event);
  }, key);
}

/// 掴んで運ぶ。**HTML5 の drag events は使わない**ので、ポインタを自分で動かす
/// （ADR 0020）。1 回で飛ばすと掴んだと判定されないため、刻んで動かす。
async function dragTo(page: Page, from: Locator, to: Locator) {
  const start = await from.boundingBox();
  const end = await to.boundingBox();
  if (start === null || end === null) throw new Error("画面に出ていません");
  const at = { x: start.x + start.width / 2, y: start.y + start.height / 2 };
  const target = { x: end.x + end.width / 2, y: end.y + end.height / 2 };
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  const steps = 12;
  for (let i = 1; i <= steps; i += 1) {
    await page.mouse.move(
      at.x + ((target.x - at.x) * i) / steps,
      at.y + ((target.y - at.y) * i) / steps,
    );
    await page.waitForTimeout(16);
  }
  await page.mouse.up();
}

// ---------------------------------------------------------------- カード

test("カードを足して保存すると、タイトルがデータベースに入る", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  await page.locator(".card-title-input").fill("新しく足したカード");
  await page.locator(".card-description-input").fill("説明も入れる");
  await page.locator(".save-card").click();

  await expect(page.locator(".card-panel")).toBeHidden();
  await expect.poll(storedTitles).toContain("新しく足したカード");
  const board = await storedBoard();
  const added = board.columns
    .flatMap((column) => column.cards)
    .find((card) => card.title === "新しく足したカード");
  expect(added?.description).toBe("説明も入れる");
  // 末尾に足す（`Board::add_card`）。1 枚目のカラムに入っていること。
  expect(board.columns[0]?.cards.at(-1)?.title).toBe("新しく足したカード");
});

/// 新しいカードにも期限・チェックリスト・タグの欄を出す（#127）。
///
/// `add_card` が下書きを丸ごと受けるので、足したあとに開き直して付け直す往復が
/// 要らない。**画面と SQLite の両方**で、1 回の保存で入っていることを見る。
test("新しいカードにも期限・チェックリスト・タグを付けて保存できる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  await expect(page.locator(".card-due-input")).toBeVisible();
  await expect(page.locator(".tags-input-field")).toBeVisible();
  await expect(page.locator(".add-checklist-item")).toBeVisible();

  await page.locator(".card-title-input").fill("備えて足すカード");
  await page.locator(".card-due-input").fill("2026-12-31");
  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").fill("先にやる");
  await page.locator(".tags-input-field").fill("足すときのタグ");
  await page.locator(".tags-input-field").press("Enter");
  await expect(
    page.locator(".tags-input-chip").filter({ hasText: "足すときのタグ" }),
  ).toHaveCount(1);

  await page.locator(".save-card").click();
  await expect(page.locator(".card-panel")).toBeHidden();

  const added = async () => {
    const board = await storedBoard();
    return board.columns
      .flatMap((column) => column.cards)
      .find((card) => card.title === "備えて足すカード");
  };
  await expect.poll(async () => (await added())?.dueDate).toBe("2026-12-31");
  const board = await storedBoard();
  const tagId = board.tags.find((tag) => tag.name === "足すときのタグ")?.id;
  expect(tagId).toBeDefined();
  expect((await added())?.tagIds).toEqual([tagId]);
  expect((await added())?.checklistItems.map((item) => item.text)).toEqual(["先にやる"]);
});

/// 枠は最初から出す（#168）。
///
/// 触るまで枠が無いと、そこが打てる場所だと読めない。しかも触った瞬間に枠と
/// 余白が現れると、打ち始めたところで文字が動く。**見るのは 2 つ**——触って
/// いない欄にも枠があること、触っても欄と案内の位置が変わらないこと。
test("タイトルと説明の枠は、触る前から出ていて、触っても位置が動かない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  const title = page.locator(".card-title-input");
  const description = page.locator(".description-field");
  const placeholder = page.locator(".description-placeholder");
  const due = page.locator(".card-due-input");
  // 透明な枠は「枠が無い」のと同じ。色まで見る。
  const invisible = ["0px", "rgba(0, 0, 0, 0)", "transparent"];

  // 開いた直後、焦点はタイトルにある（`autoFocus`）。説明と期限はまだ触って
  // いないので、そこが触る前の姿。
  await expect(title).toBeFocused();
  const descriptionBefore = await fieldFrame(description);
  const placeholderBefore = await fieldFrame(placeholder);
  const dueBefore = await fieldFrame(due);
  expect(invisible).not.toContain(descriptionBefore.borderWidth);
  expect(invisible).not.toContain(descriptionBefore.borderColor);
  expect(invisible).not.toContain(dueBefore.borderWidth);
  expect(invisible).not.toContain(dueBefore.borderColor);

  // 説明に触る。タイトルは焦点を失う側になる。
  const titleFocused = await fieldFrame(title);
  await description.click();
  await expect(page.locator(".card-description-input")).toBeFocused();

  const titleBlurred = await fieldFrame(title);
  expect(invisible).not.toContain(titleBlurred.borderWidth);
  expect(invisible).not.toContain(titleBlurred.borderColor);
  expect(titleBlurred.frame).toEqual(titleFocused.frame);

  expect((await fieldFrame(description)).frame).toEqual(descriptionBefore.frame);
  expect((await fieldFrame(placeholder)).frame).toEqual(placeholderBefore.frame);

  // 期限も同じ欄の並びにいる。触ったときに幅や余白が変わると、その 1 行ごと動く。
  await due.click();
  await expect(due).toBeFocused();
  expect((await fieldFrame(due)).frame).toEqual(dueBefore.frame);
});

/// 足したばかりのカードは、Undo 1 回で消える（#127）。
///
/// 期限もタグもチェックリストも備えたまま足すので、積まれる操作は 1 件。
/// 2 件に割れていると、ここで 1 回押しただけではカードが残る。
test("期限やタグごと足したカードも、Undo 1 回で消える", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("戻す対象");
  await page.locator(".card-due-input").fill("2026-12-31");
  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").fill("項目");
  await page.locator(".save-card").click();
  await expect.poll(storedTitles).toContain("戻す対象");

  // 入力欄にフォーカスがある間は盤面の Undo に回らない（`docs/DESIGN.md`）。
  await page.locator(".board-content").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("ControlOrMeta+z");
  await expect.poll(storedTitles).toEqual(before);
  await expect(page.locator(".card-title", { hasText: "戻す対象" })).toHaveCount(0);
});

test("説明の欄は、打った分だけ縦に伸びる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  const description = page.locator(".card-description-input");
  const height = async () => (await description.boundingBox())?.height ?? 0;
  const empty = await height();

  await description.fill("一行だけ");
  // 4 行ぶんの下限があるので、少し書いたくらいでは変わらない。
  expect(await height()).toBe(empty);

  await description.fill(Array.from({ length: 20 }, (_, i) => `${String(i)} 行目`).join("\n"));
  await expect.poll(height).toBeGreaterThan(empty);

  // 消せば戻る。伸ばしっぱなしだと、下にある操作が押せなくなる。
  await description.fill("一行だけ");
  await expect.poll(height).toBe(empty);
});

test("タイトル欄で Enter を押すと、そのまま保存される", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。打ち終わりに保存
  // ボタンまで手を伸ばさせない。
  await page.locator(".card-title-input").fill("Enter で保存");
  await page.locator(".card-title-input").press("Enter");

  await expect(page.locator(".card-panel")).toBeHidden();
  await expect.poll(storedTitles).toContain("Enter で保存");
});

test("タイトル欄で変換を確定しても、保存されない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  const title = page.locator(".card-title-input");
  await title.fill("変換の途中");

  // 変換を確定しただけ。保存でもなければ、パネルを閉じる操作でもない。
  await pressWhileComposing(title, "Enter");
  await expect(page.locator(".card-panel")).toBeVisible();
  // 変換の取り消しでパネルごと消えるのも、打ちかけを捨てることになる。
  await pressWhileComposing(title, "Escape");
  await expect(page.locator(".card-panel")).toBeVisible();
  expect(await storedTitles()).not.toContain("変換の途中");

  // 確定したあとに押した Enter は、いつもどおり保存する。
  await title.press("Enter");
  await expect(page.locator(".card-panel")).toBeHidden();
  await expect.poll(storedTitles).toContain("変換の途中");
});

test("足しかけたカードを取り下げると、跡が残らない", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("やっぱりやめる");
  await page.getByRole("button", { name: "キャンセル" }).click();

  await expect(page.locator(".card-panel")).toBeHidden();
  // **一度も存在していない。** 下書きは webview のものなので、そもそも
  // SQLite に触っていない（`docs/DESIGN.md`「状態の持ち主」）。
  expect(await storedTitles()).toEqual(before);
  await expect(page.locator(".card-title", { hasText: "やっぱりやめる" })).toHaveCount(0);
});

test("空のタイトルは、保存されずに断られる", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("   ");

  // 押せるのに断る操作は、理由を言わずにコントロールを無効にする
  // （`docs/DESIGN.md`）。理由は欄の脇に出す。
  await expect(page.locator(".save-card")).toBeDisabled();
  await expect(page.locator(".card-panel .field-error")).toContainText("タイトルを入力してください");
  await page.locator(".card-title-input").press("Enter");
  expect(await storedTitles()).toEqual(before);
});

test("カードを開いて名前を変えると、盤面と保存の両方が変わる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  await page.locator(".card-title-input").fill("書き換えたタイトル");
  await page.locator(".close-card").click();

  await expect(page.locator(".card-panel")).toBeHidden();
  await expect(page.locator(".card-title").first()).toHaveText("書き換えたタイトル");
  await expect.poll(storedTitles).toContain("書き換えたタイトル");
});

test("選んだカードは Enter で開き、Escape で閉じる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".card").first().click();
  await expect(page.locator(".card[data-selected]")).toHaveCount(1);

  await page.keyboard.press("Enter");
  await expect(page.locator(".card-panel")).toBeVisible();

  await page.locator(".card-title-input").press("Escape");
  await expect(page.locator(".card-panel")).toBeHidden();
});

// ---------------------------------------------------------------- パネルの並び

/// パネルは「カードを大きくしたもの」（#144）。上から順に、タイトル、期限、
/// タグ、説明、チェックリスト。**3 つとも別の行**（#169）——期限とタグを 1 行に
/// 押し込んでいたころは、期限の placeholder が切れ、タグのチップが 1 つで
/// 折り返していた。見出しは出さず、名前は読み上げに残す。
test("パネルの上に、タイトル・期限・タグがそれぞれの行で出る", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  const title = page.getByRole("textbox", { name: "タイトル" });
  const due = page.getByRole("textbox", { name: "期限" });
  const tags = page.getByRole("textbox", { name: "タグ" });
  await expect(title).toBeVisible();
  await expect(due).toBeVisible();
  await expect(tags).toBeVisible();

  // 上から順に、重ならずに並ぶ。横に並んでいれば行が重なる。
  const titleBox = await title.boundingBox();
  const dueBox = await due.boundingBox();
  const tagsBox = await tags.boundingBox();
  expect(titleBox).not.toBeNull();
  expect(dueBox).not.toBeNull();
  expect(tagsBox).not.toBeNull();
  if (titleBox === null || dueBox === null || tagsBox === null) return;
  expect(dueBox.y).toBeGreaterThanOrEqual(titleBox.y + titleBox.height);
  expect(tagsBox.y).toBeGreaterThanOrEqual(dueBox.y + dueBox.height);

  // 見出しの文字は出さない（名前は placeholder と aria-label が言う）。
  await expect(page.locator(".panel-body .field-label")).toHaveCount(0);
});

// ------------------------------------------------ 欄ごとに確定する（#141）

/// 保存済みのカードは、欄を離れた時点で確定します（ADR 0032）。押すものは
/// ありません。
test("説明を打って欄を離れると、保存を押さずに書き戻される", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  await page.locator(".card-description-input").fill("欄を離れたら残る");
  // 欄を離れる。押していないのに届く。
  await page.locator(".card-title-input").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.description,
    )
    .toBe("欄を離れたら残る");
});

/// 別のカードを開いても、直前のカードの変更が消えない（#142）。マニュアルが
/// 前から言っていた振る舞いに、実装が追いついた形。
test("編集中に別のカードを開いても、直前の変更が残る", async ({ page }) => {
  await openBoard(page);
  const cards = page.locator(".column").first().locator(".card");
  const firstId = Number(await cards.nth(0).getAttribute("data-card"));

  await cards.nth(0).dblclick();
  await page.locator(".card-description-input").fill("消えては困る説明");
  // 保存を押さずに、2 枚目を開く。
  await cards.nth(1).dblclick();
  await expect(page.locator(".card-panel")).toBeVisible();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === firstId)?.description,
    )
    .toBe("消えては困る説明");
});

/// チェックは押した瞬間に確定する（#139）。`Escape` で閉じても残り、Undo は
/// 1 チェックずつ戻る。
test("チェックは押した瞬間に残り、Undo で 1 つずつ戻る", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  const checks = async () =>
    (await storedBoard()).columns
      .flatMap((column) => column.cards)
      .find((card) => card.id === cardId)
      ?.checklistItems.filter((item) => item.checked).length;
  const before = (await checks()) ?? 0;

  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("ひとつ目");
  await page.keyboard.press("Enter");
  await page.keyboard.type("ふたつ目");
  await page.locator(".card-title-input").click();

  // 1 つずつ、押した時点で届いていることを確かめる。
  const rows = page.locator(".checklist-row");
  const count = await rows.count();
  await rows.nth(count - 2).locator(".checklist-toggle").click();
  await expect.poll(checks).toBe(before + 1);
  await rows.nth(count - 1).locator(".checklist-toggle").click();
  await expect.poll(checks).toBe(before + 2);

  // 打ちかけのタイトルは消えない。
  await expect(page.locator(".card-title-input")).not.toHaveValue("");

  await page.locator(".card-title-input").press("Escape");
  await expect(page.locator(".card-panel")).toBeHidden();
  expect(await checks()).toBe(before + 2);

  await page.keyboard.press("ControlOrMeta+z");
  await expect.poll(checks).toBe(before + 1);
});

/// 無題のカードは作れないまま（`docs/DESIGN.md`）。空にして離したら元に戻る。
test("タイトルを空にして欄を離れると、元のタイトルに戻る", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const before = await page.locator(".card-title-input").inputValue();

  await page.locator(".card-title-input").fill("");
  await page.locator(".card-description-input").click();

  await expect(page.locator(".card-title-input")).toHaveValue(before);
  await expect.poll(storedTitles).toContain(before);
});

// ---------------------------------------------------------------- 期限

/// 右クリックからその場で期限を当てられる（#132）。呼ぶのは
/// `set_card_due_date` の 1 コマンドなので、Undo も 1 回で戻る。
test("右クリックから期限を当てて、Undo 1 回で戻せる", async ({ page }) => {
  await openBoard(page);
  const card = page.locator(".column").first().locator(".card").first();
  const cardId = Number(await card.getAttribute("data-card"));
  const before = (await storedBoard()).columns
    .flatMap((column) => column.cards)
    .find((each) => each.id === cardId)?.dueDate;

  // 「明日」は Rust が返した `Snapshot.today` から数える（ブラウザの時計ではなく）。
  const today = localDay(new Date());
  const tomorrow = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000)
    .toISOString()
    .slice(0, 10);

  await card.click({ button: "right" });
  await page.locator(".card-menu").getByRole("button", { name: "明日" }).click();
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((each) => each.id === cardId)?.dueDate,
    )
    .toBe(tomorrow);

  await page.locator(".board-content").click({ position: { x: 5, y: 5 } });
  await page.keyboard.press("ControlOrMeta+z");
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((each) => each.id === cardId)?.dueDate,
    )
    .toBe(before);
});

test("右クリックの「なし」で期限が外れる", async ({ page }) => {
  await openBoard(page);
  const card = page.locator(".column").first().locator(".card").first();
  const cardId = Number(await card.getAttribute("data-card"));

  await card.click({ button: "right" });
  await page.locator(".card-menu").getByRole("button", { name: "なし" }).click();
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((each) => each.id === cardId)?.dueDate,
    )
    .toBeNull();
});

/// 期限はカレンダーから選ぶ欄（#120）。`type="date"` なので、打ち込める形は
/// `"YYYY-MM-DD"` だけ。ポップアップそのものは OS が出すので、ここでは見られない。
test("欄に打った日付が、そのまま保存される", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  await expect(page.locator(".card-due-input")).toHaveAttribute("type", "text");
  await page.locator(".card-due-input").fill("2026-12-31");
  await page.locator(".close-card").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.dueDate,
    )
    .toBe("2026-12-31");
});

/// 期限は文字で打てる（#134、ADR 0031）。読み方は Rust に 1 つだけなので、
/// ここで確かめるのは「打った文字が、その日付として SQLite に届くこと」。
test("「明日」と打つと、翌日の期限が保存される", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  const today = localDay(new Date());
  const tomorrow = new Date(Date.parse(`${today}T00:00:00Z`) + 86_400_000)
    .toISOString()
    .slice(0, 10);

  await page.locator(".card-due-input").fill("明日");
  // 打った文字がどう読まれたかは、確定する前に欄の下に出る。
  await expect(page.locator(".due-preview")).toContainText(tomorrow);

  await page.locator(".close-card").click();
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.dueDate,
    )
    .toBe(tomorrow);
});

test("読めない期限は、欄の脇で断られて保存されない", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  const before = (await storedBoard()).columns
    .flatMap((column) => column.cards)
    .find((each) => each.id === cardId)?.dueDate;

  await page.locator(".card-due-input").fill("きのう");
  // 読めない間は、読み下しを出さない。
  await expect(page.locator(".due-preview")).toHaveCount(0);

  // 欄を離れた時点で確定しにいき、そこで断られる（#141）。
  await page.locator(".card-title-input").click();
  await expect(page.locator(".field-error")).toContainText("日付として読めません");
  // 断られた値は打ち直せるように残り、パネルも開いたまま。
  await expect(page.locator(".card-due-input")).toHaveValue("きのう");
  expect(
    (await storedBoard()).columns
      .flatMap((column) => column.cards)
      .find((each) => each.id === cardId)?.dueDate,
  ).toBe(before);
});

/// 外す × は、期限が入っているときだけ出す（#128）。新しいカードは期限が
/// 空なので、開いた直後は出ていない。
test("期限が空のうちは、外す × を出さない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  await expect(page.getByRole("button", { name: "期限を外す" })).toHaveCount(0);
  await page.locator(".card-due-input").fill("2026-12-31");
  await expect(page.getByRole("button", { name: "期限を外す" })).toBeVisible();
});

test("× を押すと期限が外れる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  await page.locator(".card-due-input").fill("2026-12-31");
  await page.getByRole("button", { name: "期限を外す" }).click();
  await expect(page.locator(".card-due-input")).toHaveValue("");
  await page.locator(".close-card").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.dueDate,
    )
    .toBeNull();
});

// ---------------------------------------------------------------- キーで消す

/// 選んだカードは `Delete` / `Backspace` で消せる（#143）。確認は出さない
/// ——`docs/DESIGN.md`「確認ダイアログは Undo の代わりではない」。
test("選んだカードを Delete で消して、Undo 1 回で戻せる", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();
  const card = page.locator(".column").first().locator(".card").first();
  const title = await card.locator(".card-title").innerText();

  await card.click();
  await page.keyboard.press("Delete");
  await expect.poll(storedTitles).not.toContain(title);
  // 消したあとは隣のカードが選ばれ、そのまま次を消せる。
  await expect(page.locator(".card[data-selected]")).toHaveCount(1);

  await page.keyboard.press("ControlOrMeta+z");
  await expect.poll(storedTitles).toEqual(before);
});

test("検索欄にいる間は、Backspace でカードが消えない", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  await page.locator(".column").first().locator(".card").first().click();
  await page.locator("input.search").fill("あ");
  await page.keyboard.press("Backspace");
  await expect(page.locator("input.search")).toHaveValue("");
  expect(await storedTitles()).toEqual(before);
});

// ---------------------------------------------------------------- 期限の件数から辿る

/// ボード一覧の件数は押せる（#136）。押すと、その状態のいちばん先頭のカードが
/// 選ばれる。絞り込みはしないので、ほかのカードは暗くならない。
test("ボード一覧の「期限切れ」を押すと、そのカードが選ばれる", async ({ page }) => {
  await openBoard(page);

  const jump = page.getByRole("button", { name: /期限切れ/ });
  await expect(jump).toBeVisible();
  await jump.click();

  const selected = page.locator(".card[data-selected]");
  await expect(selected).toHaveCount(1);
  await expect(selected.locator(".card-due")).toContainText("⚠");
  // 減光は絞り込みのときだけ。件数から辿っても盤面の見え方は変わらない。
  await expect(page.locator(".card[data-dimmed]")).toHaveCount(0);
});

// ---------------------------------------------------------------- チェックリスト

test("チェックリストの項目を足し、並べ替え、チェックできる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  const before = await page.locator(".checklist-row").count();
  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").nth(before).fill("いちばん目");
  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").nth(before + 1).fill("につ目");

  // 2 つ目を上げると入れ替わる（`Alt+↑`、#137）。
  await page.locator(".checklist-text").nth(before + 1).press("Alt+ArrowUp");
  await expect(page.locator(".checklist-text").nth(before)).toHaveValue("につ目");

  await page.locator(".checklist-row").nth(before).locator(".checklist-toggle").click();
  await page.locator(".close-card").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)
        ?.checklistItems.map((item) => [item.text, item.checked]),
    )
    .toEqual([
      ["につ目", true],
      ["いちばん目", false],
    ]);
});

/// 名前を入れないままの項目があっても保存でき、その項目は残らない（#114）。
test("項目名を入れないままの行は、保存のときに消える", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  const before = await page.locator(".checklist-row").count();

  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").nth(before).fill("書いた");
  // 2 行目は空のまま。ここで止められないことが、この issue の受け入れ条件。
  await page.locator(".add-checklist-item").click();
  await page.locator(".close-card").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)
        ?.checklistItems.map((item) => item.text),
    )
    .toEqual(["書いた"]);
});

/// ハンドルを掴んで並べ替える（#113）。落とす位置を決めるのは `draft.ts` で、
/// 動くのは下書きの配列だけ——落とした瞬間には Rust を呼ばず、保存の 1 回で渡す。
test("チェックリストの項目を掴んで並べ替えられる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  const before = await page.locator(".checklist-row").count();
  // 「＋」は 1 回だけ押して、あとは `Enter` で続ける（#138）。押すたびに確定が
  // 走って行が増えるので、ボタンの位置が動き続けます。
  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("いち");
  await page.keyboard.press("Enter");
  await page.keyboard.type("に");
  await page.keyboard.press("Enter");
  await page.keyboard.type("さん");
  await expect(page.locator(".checklist-row")).toHaveCount(before + 3);

  const rows = page.locator(".checklist-row");
  // 3 つ目を 1 つ目の位置へ運ぶ。
  await dragTo(page, rows.nth(before + 2).locator(".checklist-handle"), rows.nth(before));
  await expect(page.locator(".checklist-text").nth(before)).toHaveValue("さん");

  // **落とした直後はクリックで保存しません。** `@dnd-kit` は落としてから 50ms
  // のあいだ document の `click` を握りつぶします（`AbstractPointerSensor` の
  // `detach()`）。人の指では届かない窓ですが、Playwright は届いてしまうので、
  // ここは打鍵で保存します（タイトル欄の `Enter`、`docs/DESIGN.md`「画面の作り」）。
  // タイトル欄の `Enter` は確定です。**閉じません**（#141）——欄ごとに確定する
  // ので、閉じるのは別の操作になりました。
  await page.locator(".card-title-input").press("Enter");
  await expect(page.locator(".card-panel")).toBeVisible();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)
        ?.checklistItems.map((item) => item.text),
    )
    .toEqual(["さん", "いち", "に"]);
});

/// 進捗のバーは幅が決まっているので、項目が増えてもカードの高さが変わらない
/// （#140、`docs/DESIGN.md`「ドラッグ＆ドロップ」）。
test("チェックリストの項目数が増えても、カードの高さが変わらない", async ({ page }) => {
  await openBoard(page);
  const card = page.locator(".column").first().locator(".card").first();
  const before = await card.boundingBox();

  // 1 項目のカードの高さを測る。ここまでは「進捗の 1 行が増えた」ぶんの差。
  await card.dblclick();
  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("項目 1");
  await page.locator(".close-card").click();
  await expect(page.locator(".card-checklist").first()).toContainText("0/1");
  const withOne = await card.boundingBox();
  expect(withOne?.height).toBeGreaterThan(before?.height ?? 0);

  // ここから項目を 11 個足しても、高さは動かない。「＋」は 1 回だけ押して、
  // あとは `Enter` で続ける（#138）。
  await card.dblclick();
  await page.locator(".add-checklist-item").click();
  for (let index = 2; index <= 12; index += 1) {
    await page.keyboard.type(`項目 ${String(index)}`);
    if (index < 12) await page.keyboard.press("Enter");
  }
  await page.locator(".close-card").click();

  await expect(page.locator(".card-checklist").first()).toContainText("0/12");
  const withTwelve = await card.boundingBox();
  expect(withTwelve?.height).toBe(withOne?.height);
});

/// 箇条書きと同じ流れで打てる（#138）。押すのは「＋ 項目を追加」1 回だけで、
/// あとは `Enter` で次の行が生まれ、そこに打てる。
test("＋ を 1 回押したら、Enter だけで項目を続けて打てる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  const before = await page.locator(".checklist-text").count();

  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("あ");
  await page.keyboard.press("Enter");
  await page.keyboard.type("い");
  await page.keyboard.press("Enter");
  await page.keyboard.type("う");
  await expect(page.locator(".checklist-text")).toHaveCount(before + 3);

  await page.locator(".close-card").click();
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)
        ?.checklistItems.map((item) => item.text)
        .slice(-3),
    )
    .toEqual(["あ", "い", "う"]);
});

test("末尾の空行で Enter を押すと、その行が消える", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const before = await page.locator(".checklist-text").count();

  await page.locator(".add-checklist-item").click();
  await expect(page.locator(".checklist-text")).toHaveCount(before + 1);
  await page.keyboard.press("Enter");
  await expect(page.locator(".checklist-text")).toHaveCount(before);
});

test("空の行で Backspace を押すと、その行が消える", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const before = await page.locator(".checklist-text").count();

  await page.locator(".add-checklist-item").click();
  await page.keyboard.press("Backspace");
  await expect(page.locator(".checklist-text")).toHaveCount(before);
});

/// 変換を確定する `Enter` で行が増えてはいけない（#124、ADR 0029）。
test("変換中の Enter では、チェックリストの行が増えない", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("あ");
  const rows = await page.locator(".checklist-text").count();
  await pressWhileComposing(page.locator(".checklist-text").last(), "Enter");
  await expect(page.locator(".checklist-text")).toHaveCount(rows);
});

/// 1 項目は必ず 1 行（#137）。折り返すと、行ごとに折り返しの位置が揃わない。
test("20 文字の項目を 5 つ入れても、各行が 1 行に収まる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  await page.locator(".add-checklist-item").click();
  for (let index = 0; index < 5; index += 1) {
    await page.keyboard.type("あ".repeat(20));
    if (index < 4) await page.keyboard.press("Enter");
  }

  const rows = page.locator(".checklist-row");
  const heights = await rows.evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect().height),
  );
  expect(heights.length).toBeGreaterThanOrEqual(5);
  // どの行も同じ高さ = どれも折り返していない。
  expect(new Set(heights).size).toBe(1);
});

/// `↑` `↓` のボタンは畳んだが、キーボードだけで並べ替える道は残る（#113、#137）。
test("項目の欄で Alt+↑ を押すと、その項目が 1 つ上がる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("あと");
  await page.keyboard.press("Enter");
  await page.keyboard.type("さき");
  await page.keyboard.press("Alt+ArrowUp");
  await page.locator(".close-card").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)
        ?.checklistItems.map((item) => item.text)
        .slice(-2),
    )
    .toEqual(["さき", "あと"]);
});

test("行末の ✕ で項目が消える", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const before = await page.locator(".checklist-row").count();

  await page.locator(".add-checklist-item").click();
  await page.keyboard.type("消す項目");
  await expect(page.locator(".checklist-row")).toHaveCount(before + 1);

  await page.locator(".checklist-remove").last().click();
  await expect(page.locator(".checklist-row")).toHaveCount(before);
});

// ---------------------------------------------------------------- タグ

test("タグを作り、カードに付け、名前を変えて消せる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".open-tag-panel").click();
  await expect(page.locator(".tag-panel")).toBeVisible();

  await page.getByLabel("新しいタグの名前").fill("あたらしいタグ");
  await page.locator(".add-tag").click();
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .toContain("あたらしいタグ");

  // カードのパネルから付ける。
  await page.locator(".open-tag-panel").click();
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  // 候補は打っているあいだだけ出ます（#169）。打ってから候補を押す。
  await page.locator(".tags-input-field").fill("あたらしい");
  await page.locator(".tag-suggestions").getByRole("button", { name: "あたらしいタグ" }).click();
  await page.locator(".close-card").click();

  const tagId = (await storedBoard()).tags.find((tag) => tag.name === "あたらしいタグ")?.id;
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.tagIds,
    )
    .toContain(tagId);

  // 名前を変える。1 行の欄なので Enter で確定する。
  await page.locator(".open-tag-panel").click();
  const row = page.locator(`.tag-row[data-tag="${tagId}"]`);
  await row.locator(".tag-name-input").fill("名前を変えたタグ");
  await row.locator(".tag-name-input").press("Enter");
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .toContain("名前を変えたタグ");

  // 消す。カードは残り、付いていたタグが外れるだけ。
  await row.locator(".remove-tag").click();
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .not.toContain("名前を変えたタグ");
  await expect.poll(storedTitles).not.toHaveLength(0);
});

/// 「追加」を押したあと、保存の往復の最中に打った名前が消えないこと（#185）。
///
/// **欄を空にするのは押した時点**で、返事が返ってきたときではありません。返事を
/// 待ってから空にしていたころは、待っているあいだに打った文字がその差し替えに
/// 巻き込まれて消えていました。
test("保存の往復の最中に打った名前が、返事で消えない", async ({ page }) => {
  await openBoard(page);
  await page.locator(".open-tag-panel").click();
  await expect(page.locator(".tag-panel")).toBeVisible();

  // **往復をわざと遅くします。** 「返事が返る前に打つ」を待ち時間の運任せに
  // しないため。遅らせるだけで、答えているのは本物のハーネスのままです。
  await page.route("**/invoke/add_tag", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 1000));
    await route.continue();
  });

  const field = page.getByLabel("新しいタグの名前");
  await field.fill("続けて 1");
  await field.press("Enter");
  // 返事を待たずに次を打つ。ここが、いままで消えていた文字。
  await field.fill("続けて 2");

  // **1 つめの返事が画面に届くのを待ちます。** 一覧に行が増えたことが、盤面の
  // 差し替えが済んだ印です（保存された盤面を読むだけでは、返事が画面に届く前を
  // すり抜けます）。返事を受け取っても、打った文字はそのまま残っている。
  await expect(page.getByLabel("続けて 1 の名前")).toBeVisible();
  await expect(field).toHaveValue("続けて 2");

  // 返事のあとに押しても、2 つめは打ったとおりに足せる。**欄を空にするのが
  // 返事のあとだったころは、ここで欄が空になっていて何も起きませんでした。**
  await field.press("Enter");
  await expect
    .poll(async () =>
      (await storedBoard()).tags
        .map((tag) => tag.name)
        .filter((name) => name.startsWith("続けて")),
    )
    .toEqual(["続けて 1", "続けて 2"]);
  await expect(field).toHaveValue("");
});

/// 断られた名前は、打ち直せるように欄へ戻る（`docs/DESIGN.md`）。
///
/// 欄は押した時点で空になるので、**戻すのは断られたときの仕事**になった。
test("同じ名前で断られたタグは、名前が欄に戻る", async ({ page }) => {
  await openBoard(page);
  await page.locator(".open-tag-panel").click();
  const field = page.getByLabel("新しいタグの名前");
  const add = page.locator(".add-tag");

  await field.fill("重複するタグ");
  // 押せるようになったことで、打った名前が React 側に渡ったと分かる。
  await expect(add).toBeEnabled();
  await field.press("Enter");
  // **1 つめの返事が画面に届くまで待ちます。** 保存された盤面を読むだけだと、
  // 返事が画面に届く前に 2 つめを送れてしまい、遅れて届いた 1 つめの成功が
  // 2 つめの失敗の表示（`setFailed`）を消してしまうことがあります。
  await expect(page.getByLabel("重複するタグ の名前")).toBeVisible();

  // 同じ名前をもう一度。同じ名前のタグは 2 つ作らない（ADR 0027）。
  await field.fill("重複するタグ");
  await expect(add).toBeEnabled();
  await field.press("Enter");

  await expect(page.locator(".tag-panel").getByRole("alert")).toBeVisible();
  await expect(field).toHaveValue("重複するタグ");
  expect(
    (await storedBoard()).tags.filter((tag) => tag.name === "重複するタグ"),
  ).toHaveLength(1);
});

/// タグの色は自動で付く（ADR 0044）。**保存に入るのは「色を決めていない」まま**で、
/// 見分けの付く色を当てるのは画面の側。作るときに色を選ばせる欄はもう無い。
test("作ったタグには、それぞれ違う色が自動で付く", async ({ page }) => {
  await openBoard(page);
  await page.locator(".open-tag-panel").click();
  await expect(page.locator(".tag-panel")).toBeVisible();
  await expect(page.getByLabel("新しいタグの色")).toHaveCount(0);

  for (const name of ["いろの試し 1", "いろの試し 2"]) {
    await page.getByLabel("新しいタグの名前").fill(name);
    await page.locator(".add-tag").click();
    // 欄は押した時点で空になる（#185）。次の名前を打てる状態に戻ったことを
    // 見てから進む。
    await expect(page.getByLabel("新しいタグの名前")).toHaveValue("");
    await expect
      .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
      .toContain(name);
  }

  // 置き場所に入るのは空の色。色を決めた覚えが無いことが、そのまま残る。
  const stored = (await storedBoard()).tags.filter((tag) =>
    tag.name.startsWith("いろの試し"),
  );
  expect(stored.map((tag) => tag.color)).toEqual(["", ""]);

  // カードに付けて、チップの色が実際に違うことを見る。
  await page.locator(".open-tag-panel").click();
  await openFirstCard(page);
  for (const name of ["いろの試し 1", "いろの試し 2"]) {
    await page.locator(".tags-input-field").fill(name);
    await page.locator(".tag-suggestions").getByRole("button", { name }).click();
  }
  const painted = [];
  for (const name of ["いろの試し 1", "いろの試し 2"]) {
    const chip = page.locator(".tags-input-chip").filter({ hasText: name });
    await expect(chip).toHaveCount(1);
    painted.push(
      await chip.evaluate((node) => {
        const style = getComputedStyle(node);
        return { background: style.backgroundColor, text: style.color };
      }),
    );
  }
  expect(new Set(painted.map((chip) => chip.background)).size).toBe(2);
  // 文字色は背景から決まるので、どのチップでも背景と同じ色にはならない。
  for (const chip of painted) {
    expect(chip.text).not.toBe(chip.background);
  }
});

/// 打った名前のタグが無ければ、その場で作って付ける（#115、ADR 0027）。
///
/// 撮影用のシードは 1 枚目のカードに「設計」を付けているので、チップは
/// 名前で絞って見る。
test("カードの編集中に、打った名前のタグをその場で作って付けられる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  await page.locator(".tags-input-field").fill("その場で作った");
  await page.locator(".tags-input-field").press("Enter");
  // 作られたタグは、保存する前からチップとして出る。
  await expect(
    page.locator(".tags-input-chip").filter({ hasText: "その場で作った" }),
  ).toHaveCount(1);
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .toContain("その場で作った");

  await page.locator(".close-card").click();

  const tagId = (await storedBoard()).tags.find((tag) => tag.name === "その場で作った")?.id;
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.tagIds,
    )
    .toContain(tagId);
});

/// 同じ名前のタグを 2 つ作らない。大文字小文字と前後の空白は同じものとして扱う。
test("既にある名前を打つと、タグは増えずに選ばれるだけ", async ({ page }) => {
  await openBoard(page);
  const before = (await storedBoard()).tags.length;

  await openFirstCard(page);
  // シードの「調査」は 1 枚目のカードには付いていない。表記を変えて打つ。
  await page.locator(".tags-input-field").fill("  調査 ");
  await page.locator(".tags-input-field").press("Enter");

  await expect(page.locator(".tags-input-chip").filter({ hasText: "調査" })).toHaveCount(1);
  await page.locator(".close-card").click();

  // 打ってからひととおり通しても、タグは増えていない。
  await expect.poll(async () => (await storedBoard()).tags.length).toBe(before);
  await expect
    .poll(async () => (await storedBoard()).tags.filter((tag) => tag.name === "調査").length)
    .toBe(1);
});

/// チップの `✕` は「このカードから外す」。タグそのものは残る。
test("チップの ✕ でカードからタグが外れ、ボードのタグは残る", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  // シードで「設計」が付いている 1 枚目。
  await expect(page.locator(".tags-input-chip").filter({ hasText: "設計" })).toHaveCount(1);

  await page.getByLabel("設計 を外す").click();
  await expect(page.locator(".tags-input-chip").filter({ hasText: "設計" })).toHaveCount(0);
  await page.locator(".close-card").click();

  const tagId = (await storedBoard()).tags.find((tag) => tag.name === "設計")?.id;
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.tagIds,
    )
    .not.toContain(tagId);
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .toContain("設計");
});

/// 候補は打っているあいだだけ出る（#169）。開いただけで、まだ付けていないタグが
/// 全部並ぶことはない。
test("タグの候補は、打っているあいだだけ出る", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  // 開いただけでは 1 つも出ない。シードには「調査」があり、1 枚目には付いていない。
  await expect(page.locator(".tag-suggestions")).toHaveCount(0);

  await page.locator(".tags-input-field").fill("調");
  await expect(
    page.locator(".tag-suggestions").getByRole("button", { name: "調査" }),
  ).toBeVisible();

  // 消せばまた引っこむ。
  await page.locator(".tags-input-field").fill("");
  await expect(page.locator(".tag-suggestions")).toHaveCount(0);
});

test("カードのタグを押すと、そのタグで絞り込む", async ({ page }) => {
  await openBoard(page);

  // タグを 1 つ作り、1 枚目のカードにだけ付ける。
  await page.locator(".open-tag-panel").click();
  await page.getByLabel("新しいタグの名前").fill("絞り込み用");
  await page.locator(".add-tag").click();
  await page.locator(".open-tag-panel").click();

  await openFirstCard(page);
  await page.locator(".tags-input-field").fill("絞り込み");
  await page.locator(".tag-suggestions").getByRole("button", { name: "絞り込み用" }).click();
  await page.locator(".close-card").click();
  await expect(page.locator(".card-panel")).toBeHidden();

  const cards = page.locator(".column .card");
  const tagged = cards.first();
  // **印はカードそのものに付きます**（子孫ではない）。`.card[data-dimmed]` で見る。
  const dimmed = page.locator(".column .card[data-dimmed]");
  await expect(dimmed).toHaveCount(0);

  // カードの上のチップを押す。**ここが絞り込みの唯一の入口**（ヘッダには
  // タグを一覧しない）。カードには元から別のタグも付いているので、名前で選ぶ。
  const chip = tagged.locator("button.tag-chip", { hasText: "絞り込み用" });
  await chip.click();

  // 付いていないカードが暗くなり、理由がヘッダに出る。
  await expect(page.locator(".filter-chip")).toContainText("絞り込み用");
  await expect.poll(async () => await dimmed.count()).toBeGreaterThan(0);
  await expect(tagged).not.toHaveAttribute("data-dimmed", /.*/);

  // 次の起動でも覚えている。
  await expect
    .poll(async () => (await storedFilter()).tagId)
    .toBe((await storedBoard()).tags.find((tag) => tag.name === "絞り込み用")?.id);

  // 同じチップをもう一度で解除。
  await chip.click();
  await expect(page.locator(".filter-chip")).toBeHidden();
  await expect.poll(async () => await dimmed.count()).toBe(0);
  await expect.poll(async () => (await storedFilter()).tagId).toBeNull();
});

test("「クリア」は検索語とタグを両方とも解いて、覚え直す", async ({ page }) => {
  await openBoard(page);
  await page.locator(".search").fill("SQLite");
  await expect.poll(async () => await page.locator(".column .card[data-dimmed]").count())
    .toBeGreaterThan(0);

  await page.locator(".clear-filter").click();

  await expect(page.locator(".search")).toHaveValue("");
  await expect.poll(async () => await page.locator(".column .card[data-dimmed]").count()).toBe(0);
  await expect.poll(async () => (await storedFilter()).search).toBe("");
});

test("検索欄の Escape でも解ける", async ({ page }) => {
  await openBoard(page);
  await page.locator(".search").fill("SQLite");
  await expect.poll(async () => await page.locator(".column .card[data-dimmed]").count())
    .toBeGreaterThan(0);

  await page.locator(".search").press("Escape");

  await expect(page.locator(".search")).toHaveValue("");
  await expect.poll(async () => (await storedFilter()).search).toBe("");
});

test("空のカラムには、落とし先だと分かる目印が出る", async ({ page }) => {
  await openBoard(page);
  // 出来合いの盤面はどのカラムにもカードが入っているので、空のものを 1 本作る。
  await page.locator(".add-column").click();
  await page.locator(".new-column-name").fill("からっぽ");
  await page.locator(".new-column-name").press("Enter");

  const empty = page.locator(".column", { hasText: "からっぽ" });
  await expect(empty.locator(".column-empty")).toHaveText("ここにドロップ");

  // カードのあるカラムには出さない。
  await expect(page.locator(".column").first().locator(".column-empty")).toHaveCount(0);
});

// ---------------------------------------------------------------- カラム

test("打った名前でカラムが足される", async ({ page }) => {
  await openBoard(page);
  await page.locator(".add-column").click();
  await page.locator(".new-column-name").fill("あたらしいカラム");
  await page.locator(".new-column-name").press("Enter");

  await expect
    .poll(async () => (await storedBoard()).columns.map((column) => column.name))
    .toContain("あたらしいカラム");
  await expect(page.locator(".column-name", { hasText: "あたらしいカラム" })).toBeVisible();
});

test("カラムの名前を直せる", async ({ page }) => {
  await openBoard(page);
  const column = page.locator(".column").first();
  const columnId = Number(await column.getAttribute("data-column"));

  await column.locator(".column-menu-button").click();
  await column.getByRole("button", { name: "編集" }).click();
  await column.locator(".column-name-input").fill("直した名前");
  await column.locator(".save-column").click();

  await expect
    .poll(async () => {
      const stored = (await storedBoard()).columns.find((each) => each.id === columnId);
      return stored?.name;
    })
    .toBe("直した名前");
});

test("カードの入ったカラムを消すには、確認に答える", async ({ page }) => {
  await openBoard(page);
  const column = page.locator(".column").first();
  const columnId = Number(await column.getAttribute("data-column"));

  await column.locator(".column-menu-button").click();
  await column.getByRole("button", { name: "削除" }).click();

  // 1 操作で複数件が消えるので確認する（`docs/DESIGN.md`）。
  await expect(page.locator(".dialog")).toContainText("カラムを削除しますか？");
  await page.locator(".dialog").getByRole("button", { name: "削除" }).click();

  await expect
    .poll(async () => (await storedBoard()).columns.map((each) => each.id))
    .not.toContain(columnId);
});

test("最後の 1 本になったカラムは消せない", async ({ page }) => {
  // カラムを 1 本だけにするのは画面の外で済ませる。**確かめたいのは、その
  // 状態で削除が押せないこと**で、そこへ辿り着くまでの操作ではない。
  const board = await storedBoard();
  for (const column of board.columns.slice(1)) {
    await editStoredBoard((document) => removeColumn(document, column.id));
  }

  await openBoard(page);
  await expect(page.locator(".column")).toHaveCount(1);
  const only = page.locator(".column").first();
  await only.locator(".column-menu-button").click();
  // 理由を言わずにコントロールを無効にする（`docs/DESIGN.md`）。
  await expect(only.getByRole("button", { name: "削除" })).toBeDisabled();
});

/// 名前を変える入口をボード一覧・カードと揃える（#145）。掴んだ判定は 4px
/// 動かしてからなので、ダブルクリックでは並びが動かない。
test("カラム名をダブルクリックすると、名前の欄が開く", async ({ page }) => {
  await openBoard(page);
  const first = page.locator(".column").first();
  const before = (await storedBoard()).columns.map((column) => column.name);

  await first.locator(".column-name").dblclick();
  const name = first.locator(".column-name-input");
  await expect(name).toBeVisible();
  // 掴んだことにはなっていない。カラムの並びはそのまま。
  expect((await storedBoard()).columns.map((column) => column.name)).toEqual(before);

  await name.fill("名前を変えた");
  await first.locator(".save-column").click();
  await expect.poll(async () => (await storedBoard()).columns[0]?.name).toBe("名前を変えた");
});

// ---------------------------------------------------------------- ボード

test("ボードを足し、名前を変え、消せる", async ({ page }) => {
  await openBoard(page);

  // #91 のとおり、ボード名はインラインではなくダイアログで打つ。
  await page.getByLabel("ボードを追加").click();
  await page.locator(".dialog-input").fill("あたらしいボード");
  await page.locator(".dialog-input").press("Enter");
  await expect(page.locator(".dialog")).toHaveCount(0);
  // 作ったボードがそのまま開く。
  await expect(page.locator(".board-header .board-name")).toHaveText("あたらしいボード");
  await expect.poll(async () => (await storedBoard()).name).toBe("あたらしいボード");

  await page.locator(".rename-board").click();
  await page.locator(".dialog-input").fill("名前を変えたボード");
  await page.locator(".dialog-input").press("Enter");
  await expect(page.locator(".board-header .board-name")).toHaveText("名前を変えたボード");
  await expect.poll(async () => (await storedBoard()).name).toBe("名前を変えたボード");

  const row = page.locator(".board-row-line", { hasText: "名前を変えたボード" });
  await row.getByLabel("名前を変えたボード の操作").click();
  await row.getByRole("button", { name: "削除" }).click();
  await expect(page.locator(".dialog")).toContainText("ボードを削除しますか？");
  await page.locator(".dialog").getByRole("button", { name: "削除" }).click();

  await expect(page.locator(".board-list")).not.toContainText("名前を変えたボード");
});

/// 一覧のボード名をダブルクリックしても名前を変えられる（#119）。開くのは
/// `…` の「名前を変更」と同じダイアログ。
test("ボード一覧の名前をダブルクリックすると、名前を変える画面が開く", async ({ page }) => {
  await openBoard(page);
  const name = await page.locator(".board-header .board-name").textContent();

  await page.locator(".board-row").first().dblclick();
  await expect(page.locator(".dialog")).toContainText("ボードの名前を変更");
  await expect(page.locator(".dialog-input")).toHaveValue(name ?? "");

  await page.locator(".dialog-input").fill("ダブルクリックで変えた");
  await page.locator(".dialog-input").press("Enter");
  await expect(page.locator(".dialog")).toHaveCount(0);
  await expect(page.locator(".board-list")).toContainText("ダブルクリックで変えた");
  await expect.poll(async () => (await storedBoard()).name).toBe("ダブルクリックで変えた");
});

test("空のボード名は受け付けない", async ({ page }) => {
  await openBoard(page);
  await page.getByLabel("ボードを追加").click();
  await page.locator(".dialog-input").fill("   ");
  await expect(page.locator(".dialog").getByRole("button", { name: "作成" })).toBeDisabled();
});

// ---------------------------------------------------------------- 割り込み

test("入力欄にいる間は、盤面の割り当てを取らない", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const before = await storedTitles();

  // パネルのタイトル欄で矢印を叩いても、裏の選択は動かない。
  await page.locator(".card-title-input").click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(200);

  expect(await storedTitles()).toEqual(before);
  await expect(page.locator(".card-panel")).toBeVisible();
});

test("失敗はダイアログに出て、盤面はそのまま", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  // 画面の裏で 1 枚消しておき、同じカードを画面から消しにいく。**入力欄に
  // 返すものではない**失敗なので、ダイアログに出る（ADR 0016）。
  const card = page.locator(".column").first().locator(".card").first();
  const cardId = Number(await card.getAttribute("data-card"));
  await editStoredBoard((document) => deleteCard(document, cardId));

  await card.click({ button: "right" });
  await page.locator(".card-menu").getByRole("button", { name: "削除" }).click();

  await expect(page.locator(".dialog")).toBeVisible();
  await page.locator(".dialog").getByRole("button", { name: "OK" }).click();
  await expect(page.locator(".dialog")).toHaveCount(0);
  // 消えたのは裏で消した 1 枚だけ。断られた操作は何も変えていない。
  expect((await storedTitles()).length).toBe(before.length - 1);
});
