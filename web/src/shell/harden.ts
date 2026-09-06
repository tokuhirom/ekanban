// webview だから要る手当て（`docs/TAURI-MIGRATION.md` §4）。
//
// ネイティブでは考えなくてよかったものを、ここで 1 か所にまとめて切ります。
// 散らすと、どれを切ったか数えられなくなります。

export function hardenWebview(): void {
  // 既定の右クリックメニューを止める。カードの右クリックメニューを自分で
  // 出すためで、webview に「再読み込み」「検証」を出させないためでもある。
  document.addEventListener("contextmenu", (event) => {
    event.preventDefault();
  });

  // 再読み込み (Ctrl+R / F5) と devtools を、リリースビルドでは塞ぐ。
  // 開発中は残す——効かないと直せない。
  if (!import.meta.env.DEV) {
    document.addEventListener("keydown", (event) => {
      const reload = event.key === "F5" || ((event.ctrlKey || event.metaKey) && event.key === "r");
      const devtools =
        event.key === "F12" ||
        ((event.ctrlKey || event.metaKey) && event.shiftKey && /^[IJC]$/i.test(event.key));
      if (reload || devtools) event.preventDefault();
    });
  }

  // 画像とテキストの既定のドラッグを止める。カードを掴む操作と取り合いになる。
  document.addEventListener("dragstart", (event) => {
    event.preventDefault();
  });

  // 拡大縮小 (Ctrl+ホイール、ピンチ) を止める。盤面は自分で幅を決めているので、
  // 拡大されると桁の揃えが崩れるだけで、得るものがない。
  document.addEventListener(
    "wheel",
    (event) => {
      if (event.ctrlKey) event.preventDefault();
    },
    { passive: false },
  );
  for (const name of ["gesturestart", "gesturechange", "gestureend"]) {
    document.addEventListener(name, (event) => {
      event.preventDefault();
    });
  }

  // macOS の 2 本指スワイプによる履歴移動を止める。1 画面のアプリなので
  // 「戻る」先が無く、戻ると白い画面になる。
  history.pushState(null, "", location.href);
  window.addEventListener("popstate", () => {
    history.pushState(null, "", location.href);
  });
}
