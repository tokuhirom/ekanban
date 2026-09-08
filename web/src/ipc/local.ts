// `localStorage` に置く置き場所を、`Ipc` として組み立てる（[ADR 0041]、[ADR 0042]）。
//
// **ブラウザで開いたときの入口**です。ページに `?store=local` が付いていれば
// こちらを使い、付いていなければ Tauri（配るアプリ）です。
//
// 置き場所は `web/src/store/memory.ts`——配るアプリで SQLite が引き受けている
// ものの TypeScript 版で、盤面のコードはアプリと 1 文字も違いません。
//
// **窓をまたいで同じものを見ます。** ボードの窓とキャプチャの窓は同じ生まれ
// （origin）なので、`localStorage` の同じ鍵を読みます。配るアプリで 2 つの窓が
// 同じ SQLite を見るのと同じ形です。
//
// [ADR 0041]: ../../../docs/adr/0041-one-layer-of-screen-tests.md
// [ADR 0042]: ../../../docs/adr/0042-the-browser-build-is-the-same-typescript.md

import { MemoryStore } from "../store/memory";
import { browserIpc } from "./browser";
import type { Ipc } from "./index";
import type { Platform } from "./types/Platform";

/// 盤面を置く鍵。**前の版が置いた文字列と同じ**にしてあります。
export const STORAGE_KEY = "ekanban:board";

/// 画面に出す「どこにあるか」。パスではありません。
const PLACE = "このブラウザの localStorage";

/// ページの外まで届くキーの割り当てを作れない理由。
const NO_GLOBAL_SHORTCUT =
  "ブラウザ版では、ページの外まで届くキーの割り当てを作れません。アプリ版で使えます";

/// この組み立てを使うか。`?store=local` が付いていれば使います。
export function usesLocalStore(): boolean {
  return new URLSearchParams(location.search).get("store") === "local";
}

/// ページからファイルを 1 つ受け取らせる。
///
/// ブラウザに「保存する場所を選ぶ」はありません。名前だけ決めて、あとは
/// ダウンロードとして渡します。
function download(fileName: string, contents: string, type: string): void {
  const url = URL.createObjectURL(new Blob([contents], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  // **文書に入れてから押します。** 外に置いたままだと、`download` に付けた
  // 名前が読まれずに `download` という名前で落ちてくるブラウザがあります。
  document.body.append(link);
  link.click();
  link.remove();
  // 押した直後に外すと、まだ読まれていないことがある。次の間合いで捨てる。
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 0);
}

export function localIpc(platform: Platform): Ipc {
  const store = MemoryStore.decode(localStorage.getItem(STORAGE_KEY) ?? "");
  return browserIpc(store, {
    platform,
    place: PLACE,
    // **置き場所は 1 つで、窓ごとの写しではありません。** 配るアプリで 2 つの窓が
    // 同じ SQLite を見るのと同じ形にするため、何かする前に取り直します。
    refresh: () => {
      const written = localStorage.getItem(STORAGE_KEY);
      if (written !== null) store.replace(MemoryStore.decode(written));
    },
    persist: () => {
      // **書けなくても盤面は動きます。** 上限に当たったら（`localStorage` は
      // 数 MB で埋まります）次に開いたときに変更が消えますが、ここで落とすと
      // 開いている画面まで止まります。
      try {
        localStorage.setItem(STORAGE_KEY, store.encode());
      } catch (error: unknown) {
        console.error(`failed to write ${STORAGE_KEY}: ${String(error)}`);
      }
    },
    download,
    shortcutUnavailable: NO_GLOBAL_SHORTCUT,
    // ほかの窓が書いたら読み直す。**ブラウザが教えてくれます**——同じ生まれの
    // ページが `localStorage` を書き換えると `storage` が飛びます。配るアプリで
    // Rust が `board:changed` を投げるのと同じ役目です。
    watch: (handler) => {
      const listen = (event: StorageEvent): void => {
        if (event.key !== STORAGE_KEY) return;
        handler();
      };
      window.addEventListener("storage", listen);
      return () => {
        window.removeEventListener("storage", listen);
      };
    },
  });
}

/// どの OS で開かれているか。
///
/// **ここだけはブラウザに訊きます。** 配るアプリでは Rust がコンパイル時に
/// 知っていて `StartupState.platform` で渡します（[ADR 0009]）が、ブラウザには
/// 訊く相手がそこしかありません。取り違えると `secondary` が Cmd か Ctrl かを
/// 間違え、割り当てが丸ごと効かなくなります。
///
/// `userAgentData.platform` を先に見るのは、そこだけが「OS を訊く」ための API
/// だからです。無いブラウザでは `userAgent` に落ちます。**当たらなかったときは
/// Linux 扱い**——キーの割り当てが Ctrl 側になるだけで、押せなくなる項目は
/// ありません。
///
/// [ADR 0009]: ../../../docs/adr/0009-per-platform-key-bindings.md
export function detectPlatform(): Platform {
  const data: unknown = (navigator as { userAgentData?: unknown }).userAgentData;
  const reported =
    typeof data === "object" && data !== null && "platform" in data
      ? String(data.platform)
      : navigator.userAgent;
  if (/mac/i.test(reported)) return "macos";
  if (/win/i.test(reported)) return "windows";
  return "linux";
}
