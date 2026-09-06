# Repository Guidelines

## Project Structure

This is a local-first Rust desktop Kanban app built with GPUI Kit and SQLite. It is a Cargo workspace.

- `crates/core/` is `ekanban-core`: the board model, SQLite, backups, file locations. **It depends on no UI toolkit** — neither gpui nor tauri — and `script/check-core-independence` fails CI if one gets pulled in. See `docs/TAURI-MIGRATION.md` §1 for why.
  - `crates/core/src/model.rs` defines `Board`, `Column`, `Card`, and board operations such as moving and reindexing.
  - `crates/core/src/db/mod.rs` owns SQLite schema migration, loading, seeding, and transactional saves.
  - `crates/core/src/paths.rs`, `backup.rs`, `instance.rs`, `diagnostics.rs` hold the per-OS file locations, the daily generational backup, the one-process-per-database lock, and the crash log.
- `crates/app/` is `ekanban-app`: the command layer from `docs/TAURI-MIGRATION.md` §3 — one command per model operation, each applying, saving, and returning the whole new snapshot. **It does not depend on `tauri`**: the `#[tauri::command]` wrappers and the window arrive in stage 3, and the HTTP harness of §10 reuses these same functions. `crates/app/tests/commands.rs` calls every command and checks both the snapshot and what reached SQLite.
- `crates/gpui/` is the `ekanban` binary drawn with GPUI Kit. `src/main.rs` is the entry point; `src/lib.rs` opens the database and the window; `src/views/` contains rendering, input handling, and drag-and-drop.
  - **It is frozen** while the move to Tauri is under way (ADR 0017): fix only what stops it from being usable. It re-exports `ekanban_core`'s modules under their old names so the frozen code keeps reading `crate::db::…`, and it goes away when the migration lands.
- `web/src/ipc/types/` holds the TypeScript types **generated from the Rust ones** by `ts-rs`. Never edit them by hand: `cargo test -p ekanban-core` (or `make types`) rewrites them, and CI fails if the committed files differ from what the Rust types produce.
- Tests are colocated with implementation modules under `#[cfg(test)]`; CI configuration is in `.github/workflows/ci.yml`.

Keep SQL inside `crates/core/src/db/` and keep UI code independent of direct database queries.

The move from GPUI Kit to Tauri is designed in `docs/TAURI-MIGRATION.md`. Read it before adding anything that would have to be moved twice.

## Build, Test, and Development Commands

Run the application with `cargo run -p ekanban` (the workspace root has no package, so `cargo run` alone cannot pick a binary). It stores its database under the OS data directory resolved by `crates/core/src/paths.rs`; set `EKANBAN_DATABASE=/absolute/path/board.sqlite3` to use another file.

- `cargo fmt --all -- --check` checks formatting.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` runs lint checks as errors.
- `cargo test --workspace --all-features` runs the unit and database round-trip tests.
- `cargo build --workspace --all-features` verifies a complete build.
- `script/check-core-independence` verifies `ekanban-core` pulls in no UI toolkit.
- `make types-check` regenerates the TypeScript types and fails if they differ from what is committed.

`make check` runs all six.

These are the same checks enforced by GitHub Actions. The bundled SQLite dependency means no database server is required.

To see a change in the running app, start it on a virtual display (`Xvfb`) with an `EKANBAN_DATABASE` of its own, and match a window's PID against the process you started before sending it a click. A copy of the app the user is already running answers to the same window name, and clicking that one discards whatever they were editing. Capture with `import -window root`; `import -window <id>` leaves out menus and other popups. The アプリを動かして確かめるとき section of `docs/DEVELOPMENT.md` has the details.

## Coding Style and Naming

Use Rust 2021 conventions and four-space indentation; let `rustfmt` determine layout. Use `snake_case` for functions, variables, and modules, and `PascalCase` for types. Prefer `Result` with the existing `thiserror` error types for fallible operations. Preserve UTF-8/Japanese text handling and use one SQLite transaction for board moves or reorder operations.

## Testing Guidelines

Name tests after observable behavior, for example `moves_card_to_another_column`. Add model tests for ordering and invalid IDs, and database tests using `tempfile` rather than the real `.ekanban.sqlite3`.

View behavior is tested against a real window. `crates/gpui/src/views/board/view_tests.rs` uses GPUI's headless test platform through `#[gpui_kit::test]`: it opens a `BoardView` in a `TestAppContext` window, dispatches the actions and keystrokes a user would, and asserts on both the on-screen state and what reached SQLite. Wait with `run_until_parked()` rather than `sleep`, take the key bindings from `crate::menu::install` instead of redefining them, and read the saved result back through `Harness::stored_board`. See the テスト section of `docs/DEVELOPMENT.md` for the details.

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
