// wasm-pack が書き出す口の形（[ADR 0035]）。
//
// **生成物そのものは git に入りません**（`web/wasm/` は `.gitignore` にあり、
// `make web-demo` が組み立てます）。型検査は wasm を組まない場でも走るので、
// 口の形だけをここに置きます。
//
// 書いてあるのは 3 つだけです。`wasm-bindgen` の `.d.ts` は引数も答えも `any`
// なので、**そのまま使っても型は付きません**——どのみち `wasmModule.ts` が
// `unknown` に受け直します。ここが本物とずれたときは、ブラウザ版の e2e
// （`web/e2e-demo/`）が実物を叩いて落ちます。
//
// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
declare module "*/ekanban_web.js" {
  /** wasm を読み込む。`wasm-bindgen --target web` の既定の書き出し。 */
  export default function init(moduleOrPath?: unknown): Promise<unknown>;
  /** `crates/web` の `start`。 */
  export function start(platform: string): unknown;
  /** `crates/web` の `invoke`。 */
  export function invoke(command: string, args: unknown): unknown;
}
