// メニューバーの構成（[ADR 0043]）。
//
// **並びと行き先を同じところで見ます。** 押されたときに何が起きるかは
// `shell/actions.ts` にあり、並びはここにあるので、「メニューにあるのに行き先が
// 無い」を 1 つのテストで捕まえられます。
//
// [ADR 0043]: ../../../docs/adr/0043-the-menu-is-described-by-the-webview.md

import { describe, expect, it } from "vitest";

import type { AppAction } from "../ipc/types/AppAction";
import type { Platform } from "../ipc/types/Platform";
import type { Item, Predefined, Section, WindowAction } from "./menu";
import { sectionsFor, webSections } from "./menu";

const PLATFORMS: Platform[] = ["macos", "windows", "linux"];

/// 画面が引き受ける操作。**dispatcher が網羅を見ている一覧と同じもの**で、
/// `AppAction` から作ります（`shell/actions.ts`）。
const APP_ACTIONS: AppAction[] = [
  "addBoard",
  "addCard",
  "addColumn",
  "addTag",
  "exportBoardJson",
  "exportBoardMarkdown",
  "saveEdit",
  "undo",
  "redo",
  "cancelEdit",
  "clearSearch",
  "renameBoard",
  "deleteBoard",
  "manageTags",
  "focusSearch",
  "toggleBoardList",
  "toggleArchiveView",
  "setQuickCaptureShortcut",
  "useLightTheme",
  "useDarkTheme",
  "useSystemTheme",
  "backupDatabase",
  "revealDatabase",
  "revealBackups",
  "about",
];

/// macOS の OS が持っている項目。ほかの環境では出しようがない。
const MACOS_ONLY: Predefined[] = ["services", "hide", "hideOthers", "showAll"];

const macos = (): Section[] => sectionsFor("macos", null);
const drawn = (): Section[] => sectionsFor("linux", null);

function items(sections: Section[]): Item[] {
  return sections.flatMap((section) => section.items);
}

function appActions(sections: Section[]): AppAction[] {
  return items(sections).flatMap((item) => (item.kind === "app" ? [item.action] : []));
}

function windowActions(sections: Section[]): WindowAction[] {
  return items(sections).flatMap((item) => (item.kind === "window" ? [item.action] : []));
}

function predefinedItems(sections: Section[]): Predefined[] {
  return items(sections).flatMap((item) => (item.kind === "predefined" ? [item.item] : []));
}

function accelerators(sections: Section[]): [string, string][] {
  return items(sections).flatMap((item) => {
    if (item.kind === "app" || item.kind === "window") {
      return item.accelerator === null ? [] : [[item.action, item.accelerator] as [string, string]];
    }
    return [];
  });
}

describe("sectionsFor", () => {
  /// **画面が引き受ける操作は、どちらのメニューバーにも出ていること。**
  ///
  /// 足したのに並べ忘れると、dispatcher にだけ手が入って、押す道がどこにも
  /// 無い操作が残ります（実際に「アーカイブ表示を切り替え」でそうなりました）。
  it("は、どちらのメニューバーにも全部の操作を並べる", () => {
    for (const [bar, sections] of [
      ["macOS", macos()],
      ["drawn", drawn()],
    ] as const) {
      const onTheBar = appActions(sections);
      for (const action of APP_ACTIONS) {
        // 「ekanban について」だけは macOS では OS の項目が出す。
        if (action === "about" && bar === "macOS") continue;
        expect(onTheBar, `${action} is not on the ${bar} menu bar`).toContain(action);
      }
    }
  });

  /// ウィンドウの操作は macOS 以外のメニューバーから届く。macOS では OS の項目
  /// （閉じる・終了・フルスクリーン）が持つ。
  it("は、ウィンドウの操作を描くほうのメニューバーに並べる", () => {
    expect([...windowActions(drawn())].sort()).toEqual(
      ["closeWindow", "quit", "toggleFullscreen"],
    );
  });

  /// 受け入れ条件「macOS でたどれる操作は Linux・Windows でもたどれる」（#79）。
  it("は、macOS で出る操作をほかの OS でも出す", () => {
    const onDrawn = appActions(drawn());
    const missing = appActions(macos()).filter((action) => !onDrawn.includes(action));
    expect(missing).toEqual([]);

    // 「ekanban について」は macOS では OS の項目、ほかでは自前の項目。
    expect(predefinedItems(macos())).toContain("about");
    expect(onDrawn).toContain("about");
  });

  /// 受け入れ条件「Linux・Windows のメニューに macOS 専用の項目が入らない」（#79）。
  it("は、macOS 専用の項目をほかの OS に持ち込まない", () => {
    const onMacos = predefinedItems(macos());
    const onDrawn = predefinedItems(drawn());
    for (const item of MACOS_ONLY) {
      expect(onMacos, `${item} belongs on the macOS menu bar`).toContain(item);
      expect(onDrawn, `${item} is macOS-only`).not.toContain(item);
    }
  });

  /// 終了・フルスクリーン・ボード一覧には、どの OS でも届く手段がある（#53）。
  it("は、終了とフルスクリーンとボード一覧にどの OS でも届かせる", () => {
    expect(predefinedItems(macos())).toContain("quit");
    expect(predefinedItems(macos())).toContain("fullscreen");
    expect(windowActions(drawn())).toContain("quit");
    expect(windowActions(drawn())).toContain("toggleFullscreen");
    for (const sections of [macos(), drawn()]) {
      expect(appActions(sections)).toContain("toggleBoardList");
    }
  });

  /// 入力中の `Cmd+Z` が盤面を巻き戻さないこと（`docs/DESIGN.md`）。
  ///
  /// アクセラレータを付けた時点で、入力欄にフォーカスがあっても先に取られます。
  /// キーを webview で受けて振り分けるという決めごとは、**ここに割り当てを
  /// 書かないこと**で守られます。
  it("は、元に戻す・やり直すに割り当てを付けない", () => {
    for (const sections of [macos(), drawn()]) {
      const assigned = accelerators(sections).map(([id]) => id);
      expect(assigned).not.toContain("undo");
      expect(assigned).not.toContain("redo");
    }
  });

  it("は、同じ組み合わせを 2 つの操作に割り当てない", () => {
    for (const sections of [macos(), drawn()]) {
      const combinations = accelerators(sections).map(([, accelerator]) => accelerator);
      expect(new Set(combinations).size).toBe(combinations.length);
    }
  });

  /// 割り当ての書き方が muda の読める形であること。
  ///
  /// 読めない文字列はメニューを組む時点で `Err` になり、**メニューが掛からない
  /// まま終わります**。
  it("は、muda が読める形で割り当てを書く", () => {
    const modifiers = ["CmdOrCtrl", "Cmd", "Ctrl", "Alt", "Shift"];
    for (const sections of [macos(), drawn()]) {
      for (const [id, accelerator] of accelerators(sections)) {
        const parts = accelerator.split("+");
        const key = parts.pop() ?? "";
        for (const modifier of parts) {
          expect(modifiers, `${id} carries an unknown modifier ${modifier}`).toContain(modifier);
        }
        const known = /^[A-Z]$/.test(key) || /^F([1-9]|1[0-9]|2[0-4])$/.test(key);
        expect(known, `${id} carries an unknown key ${key}`).toBe(true);
      }
    }
  });

  /// 1 つのメニューの中に同じ操作が二度出ていないか。
  ///
  /// メニューをまたぐ重なりは数えません。macOS では「ウインドウを閉じる」が
  /// ファイルとウインドウに、「ekanban について」が ekanban とヘルプに出るのが
  /// 作法どおりです。
  it("は、1 つのメニューの中に同じ項目を 2 度出さない", () => {
    for (const sections of [macos(), drawn()]) {
      for (const section of sections) {
        const ids = section.items.flatMap((item) => {
          if (item.kind === "app" || item.kind === "window") return [item.action];
          if (item.kind === "predefined") return [item.item];
          return [];
        });
        expect(new Set(ids).size, `${section.name} lists an item twice`).toBe(ids.length);
      }
    }
  });

  /// 使えない環境のクイックキャプチャは、消さずに灰色にして理由を文言に入れる。
  it("は、割り当てを作れない環境で理由を文言に入れる", () => {
    const item = items(sectionsFor("linux", "ブラウザ版では作れません")).find(
      (each) => each.kind === "app" && each.action === "setQuickCaptureShortcut",
    );
    expect(item?.kind === "app" && item.enabled).toBe(false);
    expect(item?.kind === "app" && item.label).toContain("ブラウザ版では作れません");
  });
});

describe("webSections", () => {
  /// ページが描くメニューに、OS のものが混ざらないこと（ADR 0035）。
  ///
  /// OS の項目はブラウザの中に相手がおらず、ウィンドウの操作にはページの手が
  /// 届きません。**どちらも「押しても何も起きない項目」になる**ので出しません。
  it("は、OS が持っているものを落とす", () => {
    for (const platform of PLATFORMS) {
      const sections = webSections(platform, null);
      const kinds = new Set(items(sections).map((item) => item.kind));
      expect(kinds.has("predefined")).toBe(false);
      expect(kinds.has("window")).toBe(false);

      // 殻のメニューにある操作だけが出ていること。ページにだけ項目を足さない。
      const shell = appActions(sectionsFor(platform, null));
      for (const action of appActions(sections)) {
        expect(shell, `${action} は殻のメニューに無い`).toContain(action);
      }
      expect(appActions(sections).length).toBeGreaterThan(0);
    }
  });

  /// 区切り線が、端にも 2 つ続けても残らないこと。
  ///
  /// OS のものを落とすと、そのぶん区切り線が浮きます。**数えるのを描く側に
  /// させません**——出す側で畳んでおけば、描くほうは並べるだけで済みます。
  it("は、浮いた区切り線を残さない", () => {
    for (const platform of PLATFORMS) {
      for (const section of webSections(platform, null)) {
        expect(section.items.at(0)?.kind).not.toBe("separator");
        expect(section.items.at(-1)?.kind).not.toBe("separator");
        for (let at = 1; at < section.items.length; at += 1) {
          const pair = [section.items[at - 1]?.kind, section.items[at]?.kind];
          expect(pair, `${section.name}: 区切り線が続いている`).not.toEqual([
            "separator",
            "separator",
          ]);
        }
      }
    }
  });

  /// ブラウザに相手がいない項目は、消さずに灰色にして理由を出すこと。
  it("は、できないことを灰色にして理由を出す", () => {
    const drawnItems = items(webSections("linux", null));
    const find = (wanted: AppAction) => {
      const found = drawnItems.find((item) => item.kind === "app" && item.action === wanted);
      if (found?.kind !== "app") throw new Error(`${wanted} が出ていない`);
      return found;
    };

    for (const action of [
      "revealDatabase",
      "revealBackups",
      // ブラウザ版に SQLite のファイルが無い（ADR 0036）。
      "backupDatabase",
    ] as const) {
      expect(find(action).enabled, `${action} は押せないはず`).toBe(false);
      expect(find(action).label).toContain("ブラウザでは使えません");
    }
    // 盤面の持ち出しは残る。ここまで灰色にしない。
    expect(find("exportBoardJson").enabled).toBe(true);
    expect(find("exportBoardMarkdown").enabled).toBe(true);
  });
});
