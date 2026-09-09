/// 動いているアプリの版。組み立てるときに根の `Cargo.toml` から焼き込む
/// （`vite.config.ts`）。配るアプリでは Rust が `CARGO_PKG_VERSION` から
/// 渡すので、これを読むのはブラウザだけで動く組み立てだけ。
declare const __EKANBAN_VERSION__: string;
