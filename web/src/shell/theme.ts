// テーマの適用（`docs/DESIGN.md`「画面の作り」）。
//
// 選ばれたテーマは `app_state` に入り、起動のときに `StartupState.theme` で
// 届きます。ここがすることは、それを CSS に見える形にするだけです。
//
// **「システムに合わせる」を JavaScript で判定しません。** 属性を外すと、
// `styles.css` の `prefers-color-scheme` がそのまま効きます。OS の設定が
// 変わったときも、こちらが何もしなくても切り替わります。gpui 版が
// `observe_window_appearance` で拾い直していたところです。

import type { ThemePreference } from "../ipc/types/ThemePreference";

export function applyTheme(theme: ThemePreference): void {
  const root = document.documentElement;
  if (theme === "system") {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
}
