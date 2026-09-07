fn main() {
    // 殻を外した組み立て（`crates/web`、[ADR 0035]）には、埋め込む `web/dist` も
    // 生成する tauri.conf.json の定義もありません。**build script は `cfg!` で
    // feature を読めない**ので、cargo が立てる環境変数を見ます。
    //
    // [ADR 0035]: ../../docs/adr/0035-a-browser-build-of-the-real-core.md
    #[cfg(feature = "shell")]
    tauri_build::build();
}
