// IME の変換中に押されたキーかどうかを、1 か所で決める。
//
// 1 行の欄では `Enter` で保存し、`Escape` で取り消します（`docs/DESIGN.md`
// 「画面の作り」）。**変換を確定する `Enter` と、変換を取り消す `Escape` は
// それに当ててはいけません。** 打ち終わっていない文字が、確定しただけで
// 保存され、取り消しただけで欄ごと閉じます。

/// 変換中の `keydown` かどうかを見るのに要るものだけ。
///
/// `KeyboardEvent` をそのまま受けないのは、`keyCode` が非推奨で、DOM の型から
/// 読むと lint が止めるからです。非推奨のものに触るのをこのファイルに閉じます。
export interface ComposingKey {
  readonly isComposing: boolean;
  readonly keyCode: number;
}

/// IME の変換に伴う `keydown` か。
///
/// **`isComposing` だけでは足りません。** WebKit は変換を確定する `Enter` の
/// `keydown` より先に `compositionend` を出すので、その `keydown` は
/// `isComposing === false` で届きます。macOS の WKWebView と Linux の
/// WebKitGTK がこれで、変換を確定しただけのつもりが保存になります（#124）。
/// Chromium（WebView2）は `keydown` が先なので `isComposing` で足ります。
///
/// 変換に伴う `keydown` は、どのエンジンでも `keyCode` が 229 になります。
/// 発火順に関わらず見分けられるのはこれだけなので、非推奨でもこちらを見ます。
export function isComposing(event: ComposingKey): boolean {
  return event.isComposing || event.keyCode === IME_KEY_CODE;
}

/// 「IME が処理したキー」を表す `keyCode`。実際に押されたキーは入っていません。
const IME_KEY_CODE = 229;
