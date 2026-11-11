# OxidePress — agent notes

OxidePress is a GPL-2.0-only fork of RustPress by Ryuji Yasukochi. Upstream history stays. `LICENSE` stays verbatim. Credit lives in `AUTHORS`.

A task touches one crate. Read this file, then [workflow.md](workflow.md), run the map, then open that crate.

```powershell
pwsh -File tools/task.ps1 -Keyword auth
```

## Verify

Scoped, while editing:

```powershell
cargo check -p <package>
cargo test -p <package> --lib
cargo fmt -p <package> -- --check
```

Before a PR:

```powershell
cargo check --workspace
cargo test --workspace --lib --bins -- --skip e2e
cargo fmt --all -- --check
```

## Schema gap

WordPress compatibility is a diff, not a reread. Regenerate it with:

```powershell
pwsh -File tools/wp-gap.ps1
```

The report is `gaps/schema.json`. It compares the pinned WordPress `schema.php` with `crates/oxidepress-migrate`. `existing_database_repairs` are the ALTER statements an older database still needs; `CREATE TABLE IF NOT EXISTS` does not apply them. Core blocks are `gaps/blocks.json` from `pwsh -File tools/wp-blocks.ps1`. Edit the named crate from a row in the report. Do not open WordPress source to rediscover the list.

## Do not read

`MASTERPLAN.md` and `docs/instructions/` are upstream planning notes. They are not the task list.

## Do not change

- `LICENSE`, `themes/`, `static/`, `wp-content/`, and the upstream credit in `AUTHORS`.
- Cookie `rustpress_session` and `wp_options` keys `_rustpress_session_`.
- Tables `rustpress_sessions`, `rustpress_cache`, `rustpress_migrations`.
- Template key `rustpress_version` (themes still read it).
- Env vars `RUSTPRESS_HOST`, `RUSTPRESS_PORT`, `RUSTPRESS_URL`, `RUSTPRESS_UPLOAD_DIR`.
- WordPress table names (`wp_posts`, `wp_options`, and the rest) and password hashes. Schema changes belong in `oxidepress-migrate` and have to match a row in `gaps/schema.json`.

`tools/rename.ps1` is the rebrand codemod. Dry-run is the default. It is not a general editor.

## Change notices

On a file you modify, keep any existing copyright lines. If the file has no fork notice yet, add:

```rust
// Modified by Vabian124 on YYYY-MM-DD to <what changed>.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
```

Use the date of the change. Describe the change you actually made.
