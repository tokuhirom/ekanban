# Repository Guidelines

## Project Structure

This is a local-first Rust desktop Kanban app built with GPUI Kit and SQLite.

- `src/main.rs` is the binary entry point; `src/lib.rs` initializes the database and window.
- `src/model.rs` defines `Board`, `Column`, `Card`, and board operations such as moving and reindexing.
- `src/db/mod.rs` owns SQLite schema migration, loading, seeding, and transactional saves.
- `src/views/` contains GPUI rendering, input handling, and drag-and-drop behavior.
- Tests are colocated with implementation modules under `#[cfg(test)]`; CI configuration is in `.github/workflows/ci.yml`.

Keep SQL inside `src/db/` and keep UI code independent of direct database queries.

## Build, Test, and Development Commands

Run the application with `cargo run`. It stores its database under the OS data directory resolved by `src/paths.rs`; set `EKANBAN_DATABASE=/absolute/path/board.sqlite3` to use another file.

- `cargo fmt --all -- --check` checks formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` runs lint checks as errors.
- `cargo test --all-features` runs the unit and database round-trip tests.
- `cargo build --all-features` verifies a complete build.

These are the same checks enforced by GitHub Actions. The bundled SQLite dependency means no database server is required.

## Coding Style and Naming

Use Rust 2021 conventions and four-space indentation; let `rustfmt` determine layout. Use `snake_case` for functions, variables, and modules, and `PascalCase` for types. Prefer `Result` with the existing `thiserror` error types for fallible operations. Preserve UTF-8/Japanese text handling and use one SQLite transaction for board moves or reorder operations.

## Testing Guidelines

Name tests after observable behavior, for example `moves_card_to_another_column`. Add model tests for ordering and invalid IDs, and database tests using `tempfile` rather than the real `.ekanban.sqlite3`.

View behavior is tested against a real window. `src/views/board/view_tests.rs` uses GPUI's headless test platform through `#[gpui_kit::test]`: it opens a `BoardView` in a `TestAppContext` window, dispatches the actions and keystrokes a user would, and asserts on both the on-screen state and what reached SQLite. Wait with `run_until_parked()` rather than `sleep`, take the key bindings from `crate::menu::install` instead of redefining them, and read the saved result back through `Harness::stored_board`. See the テスト section of `docs/DEVELOPMENT.md` for the details.

Run formatting, Clippy, and all-feature tests before submitting changes.

## Commits and Pull Requests

Work goes through pull requests. A repository ruleset requires the `Check and test` CI job to pass on `main`, so pushing to `main` directly no longer works. Branch off `main`, open a PR that says `Closes #<issue>`, and enable auto-merge with `gh pr merge <n> --squash --auto --delete-branch`; it merges once CI is green and deletes the branch.

Write commit subjects in English, in the imperative mood, and explain in the body why the change was made rather than restating what it does.

## Planning and Documentation

Work in progress is tracked in GitHub issues, not in a roadmap document. When you pick up a task, read its issue for the scope and acceptance criteria, and close the issue when they are met.

`README.md` is user-facing: what the app does, how to install and run it, where its data lives. Developer-facing material (module layout, schema, bundling, signing, CI) belongs in `docs/DEVELOPMENT.md`, and usage in `docs/MANUAL.md`.

`docs/DESIGN.md` holds only what outlives a single issue: the design rules new code must follow, what is out of scope, what was considered and deliberately rejected, and the completion checklist every change must satisfy. Do not add task lists or schedules to it. It is the list of rules **as they stand now**, so rewrite an entry when the decision behind it changes.

`docs/adr/` keeps the reasoning those rules came from, one file per decision. Add an ADR when a decision has alternatives worth recording or would otherwise be re-litigated; link to it from the matching rule in `docs/DESIGN.md`. **Never edit an existing ADR** — supersede it with a new one and mark the old one's status. Do not repeat the same text in both places: rules in `DESIGN.md`, reasoning in the ADR. See `docs/adr/README.md` for the template.
