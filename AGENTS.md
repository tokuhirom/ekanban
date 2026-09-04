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

Run the application with `cargo run`. It creates `.ekanban.sqlite3` in the repository directory; set `EKANBAN_DATABASE=/path/to/board.sqlite3` to use another file.

- `cargo fmt --all -- --check` checks formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` runs lint checks as errors.
- `cargo test --all-features` runs the unit and database round-trip tests.
- `cargo build --all-features` verifies a complete build.

These are the same checks enforced by GitHub Actions. The bundled SQLite dependency means no database server is required.

## Coding Style and Naming

Use Rust 2021 conventions and four-space indentation; let `rustfmt` determine layout. Use `snake_case` for functions, variables, and modules, and `PascalCase` for types. Prefer `Result` with the existing `thiserror` error types for fallible operations. Preserve UTF-8/Japanese text handling and use one SQLite transaction for board moves or reorder operations.

## Testing Guidelines

Name tests after observable behavior, for example `moves_card_to_another_column`. Add model tests for ordering and invalid IDs, and database tests using `tempfile` rather than the real `.ekanban.sqlite3`. Run formatting, Clippy, and all-feature tests before submitting changes.

## Commits and Pull Requests

Currently, this repo is under development. you can commit to 'main' branch directly. and you can push to the remote directly.

## Planning and Documentation

Work in progress is tracked in GitHub issues, not in a roadmap document. When you pick up a task, read its issue for the scope and acceptance criteria, and close the issue when they are met.

`docs/DESIGN.md` holds only what outlives a single issue: the design rules new code must follow, what is out of scope, what was considered and deliberately rejected, and the completion checklist every change must satisfy. Do not add task lists or schedules to it. When you make a decision that constrains future work, or reverse an earlier one, record it there with the reasoning.
