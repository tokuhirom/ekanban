# Repository Guidelines

## Project Structure

This is a local-first Kanban app: a Rust core, a Tauri shell, and a TypeScript webview, with everything in one SQLite file. It is a Cargo workspace.

- `crates/core/` is `ekanban-core`: the board model, SQLite, backups, file locations. **It depends on no UI toolkit** — `tauri` must not reach it — and `script/check-core-independence` fails CI if one gets pulled in. See `docs/DESIGN.md`「層の分け方」 for why.
  - `crates/core/src/model.rs` defines `Board`, `Column`, `Card`, and board operations such as moving and reindexing.
  - `crates/core/src/db/mod.rs` owns SQLite schema migration, loading, seeding, and transactional saves.
  - `crates/core/src/paths.rs`, `backup.rs`, `instance.rs`, `diagnostics.rs` hold the per-OS file locations, the daily generational backup, the one-process-per-database lock, and the crash log.
- `crates/app/` is `ekanban-app`, the Tauri binary (`ekanban`). `commands.rs` holds the commands described in `docs/DESIGN.md`「コマンドとイベント」 — one command per model operation, each applying, saving, and returning the whole new snapshot — and **knows nothing about `tauri`**, so the HTTP harness can reuse it. `ipc.rs` is nothing but `#[tauri::command]` wrappers over those functions; put no judgement there. `crates/app/tests/commands.rs` calls every command and checks both the snapshot and what reached SQLite.
  - `menu.rs` builds the menu bar **as data first** (`sections()`) and converts it to a Tauri menu afterwards, so the tests read the structure without opening a window; never branch on `cfg!` while assembling it, or the tests can only see one platform's bar. What a menu item does is split in two: `AppAction` goes to the webview over `app:action` (everything that touches the board, a draft, or the display state — the screen is what knows), `WindowAction` is done in Rust (close, quit, fullscreen).
  - `window.rs` remembers the window rectangle in `app_state`: it waits for the movement to settle before writing, and does not remember a full-screen or maximised window.
  - `capture.rs` and `shortcut.rs` are quick capture: the window (a second entry point, `web/capture.html`, with the app-wide menu removed) and the global shortcut. **The stored form of a shortcut never changed** (`ctrl-alt-shift-cmd-n`), so old databases keep working without a migration — `shortcut.rs` converts to and from Tauri's spelling instead; the webview sends `KeyboardEvent.code` and Rust builds and validates the combination.
  - Choosing where a file goes is the OS dialog (`ipc::choose_save_path`, an `async` command — a sync one runs on the main thread and freezes the window while the dialog is up); writing it is `commands`; saying it was written is a dialog inside the app (ADR 0016). The webview puts those three in order, so the harness can drive the same path with only the choosing replaced.
- `crates/harness/` is `ekanban-harness`, **development and test only, never shipped**: it puts `crates/app`'s commands on HTTP under the same names so the same screen runs in an ordinary browser (`docs/DESIGN.md`「テスト」). What answers is the real `ekanban-core`, which is the point — ADR 0021 forbids a fake backend written in TypeScript.
- `web/` is the webview: TypeScript + React + Vite (ADR 0019). `src/ipc/` is the one door to Rust, `src/state/` holds the snapshot and the single path that replaces it (`run()` — nothing else calls `setSnapshot`, and it is what decides whether a failure goes to a field or to a dialog), `src/board/` draws the board, `src/panel/` the card and tag editing panels, `src/shell/` the dialogs, the menu actions (`actions.ts` — each part subscribes to the actions it owns, so "save" reaches the panel holding the draft), the theme, the export/backup flow (`files.ts`), and the things a webview must switch off by hand (`harden.ts`). `src/panel/Description.tsx` lays a link layer behind the description textarea: **Rust finds the URLs** (`commands::description_links`, positions in UTF-16 units so they line up with JavaScript's string indices) and the layer only draws them. Drag-and-drop rides on `@dnd-kit/core` (ADR 0022), but **where a card lands is decided by our own `board/dnd.ts` and `board/keyboard.ts`** — the library carries, it does not decide, so it can be dropped. **Compiling `crates/app` needs `web/dist`**, so run `npm --prefix web ci && npm --prefix web run build` before `cargo build`/`cargo test` on a fresh checkout.
- `web/e2e/` drives the real board through the harness with Playwright: `harness.ts` starts one database and one `ekanban-harness` per test, and its `invoke()` reads the saved board back so a test can check **both** what is on screen and what reached SQLite.
- `web/src/ipc/types/` holds the TypeScript types **generated from the Rust ones** by `ts-rs`. Never edit them by hand: `make types` rewrites them, and CI fails if the committed files differ from what the Rust types produce.
- Tests are colocated with implementation modules under `#[cfg(test)]`; CI configuration is in `.github/workflows/ci.yml`.

Keep SQL inside `crates/core/src/db/` and keep UI code independent of direct database queries.

`docs/DESIGN.md` is the list of rules in force. Read it before adding anything that would have to be moved twice; if you add, rewrite, or break one of its rules, an ADR is required.

## Build, Test, and Development Commands

Run the app with `make dev` — a debug build has Tauri's `devUrl` baked in, so `cargo run -p ekanban-app` on its own shows a blank window with "Connection refused"; a build that embeds the screen instead is `tauri build --debug --no-bundle`. It stores its database under the OS data directory resolved by `crates/core/src/paths.rs`; set `EKANBAN_DATABASE=/absolute/path/board.sqlite3` to use another file.

- `cargo fmt --all -- --check` checks formatting.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` runs lint checks as errors.
- `cargo test --workspace --all-features` runs the unit and database round-trip tests.
- `cargo build --workspace --all-features` verifies a complete build.
- `script/check-core-independence` verifies `ekanban-core` pulls in no UI toolkit.
- `make types-check` regenerates the TypeScript types and fails if they differ from what is committed.
- `make web-check` runs the webview's `tsc --noEmit`, ESLint and Vitest. ESLint stands in for `unsafe_code = "forbid"` on the TypeScript side (§9), so a rule is not disabled to make a file pass.
- `make e2e` drives the board through the harness in **Chromium and WebKit**. Those are not the real webviews, but the three real ones only use two engines — WebView2 is Chromium, WKWebView and WebKitGTK are WebKit — so engine-family differences show up here. What cannot show up is Apple's platform layer (momentum scrolling, rubber-banding, trackpad).

`make check` runs all seven; `make e2e` is separate because it builds browsers.

**Never let two files differ only in case.** macOS and Windows resolve `./Archive` to `archive.ts`, so an import picks the wrong file and the build fails there while Linux is green — the CI cross jobs are where you find out. Component files are `PascalCase.tsx` and the plain modules beside them get a different word (`Archive.tsx` with `archived.ts`, `Description.tsx` with `links.ts`), not the same word in another case.

**Never decide platform behaviour from `navigator.userAgent`.** It is a string a webview can change — Playwright's Safari emulation calls itself `Macintosh` on Linux — and getting `secondary` wrong disables a whole key binding. Rust knows the platform at compile time and sends it in `StartupState.platform`.

These are the same checks enforced by GitHub Actions. The bundled SQLite dependency means no database server is required.

To see a change in the running app, start it on a virtual display (`Xvfb`) with an `EKANBAN_DATABASE` of its own, and match a window's PID against the process you started before sending it a click. A copy of the app the user is already running answers to the same window name, and clicking that one discards whatever they were editing. Capture with `import -window root`; `import -window <id>` leaves out menus and other popups. The アプリを動かして確かめるとき section of `docs/DEVELOPMENT.md` has the details.

## Coding Style and Naming

Use Rust 2021 conventions and four-space indentation; let `rustfmt` determine layout. Use `snake_case` for functions, variables, and modules, and `PascalCase` for types. Prefer `Result` with the existing `thiserror` error types for fallible operations. Preserve UTF-8/Japanese text handling and use one SQLite transaction for board moves or reorder operations.

## Testing Guidelines

Name tests after observable behavior, for example `moves_card_to_another_column`. Add model tests for ordering and invalid IDs, and database tests using `tempfile` rather than the real `.ekanban.sqlite3`.

Screen behaviour is tested through the harness with Playwright (`web/e2e/`): drive the board the way a person would, then read the board back with `invoke()` and assert on both. Never `sleep` — use `expect.poll` and the locator waits. What the harness cannot show is the Tauri shell itself (the real menu bar, the OS save dialog, the global shortcut, window bounds); that is checked by hand on a virtual display. See the テスト section of `docs/DEVELOPMENT.md` for the details.

Run formatting, Clippy, and all-feature tests before submitting changes.

## Commits and Pull Requests

Work goes through pull requests. A repository ruleset requires the `Check and test` CI job to pass on `main`, so pushing to `main` directly no longer works. Branch off `main`, open a PR that says `Closes #<issue>`, and enable auto-merge with `gh pr merge <n> --squash --auto --delete-branch`; it merges once CI is green and deletes the branch.

Write commit subjects in English, in the imperative mood, and explain in the body why the change was made rather than restating what it does.

## Planning and Documentation

Work in progress is tracked in GitHub issues, not in a roadmap document. When you pick up a task, read its issue for the scope and acceptance criteria, and close the issue when they are met.

`README.md` is user-facing: what the app does, how to install and run it, where its data lives. Developer-facing material (module layout, schema, bundling, signing, CI) belongs in `docs/DEVELOPMENT.md`, and usage in `docs/MANUAL.md`.

`docs/DESIGN.md` holds only what outlives a single issue: the design rules new code must follow, what is out of scope, what was considered and deliberately rejected, and the completion checklist every change must satisfy. Do not add task lists or schedules to it. It is the list of rules **as they stand now**, so rewrite an entry when the decision behind it changes.

`docs/adr/` keeps the reasoning those rules came from, one file per decision.

**A significant decision must be recorded as an ADR before its issue is closed** — it is item 10 on the completion checklist in `docs/DESIGN.md`. Decide whether one is needed while designing, not after the code is written. `docs/adr/README.md` has the test; it is needed when the change adds, rewrites or breaks a rule in `docs/DESIGN.md`, reverses an earlier decision, picks between real alternatives, changes something visible from outside (where data lives, its format, key bindings, per-platform differences), adds or swaps a dependency, or settles a question the issue's acceptance criteria did not cover.

Bug fixes, wording, refactoring, and implementing an issue exactly as its acceptance criteria describe need no ADR. Writing one for everything is how they stop being read.

Link the ADR from the matching rule in `docs/DESIGN.md`. **Never edit an existing ADR** — supersede it with a new one and mark the old one's status. Do not repeat the same text in both places: rules in `DESIGN.md`, reasoning in the ADR. See `docs/adr/README.md` for the template.
