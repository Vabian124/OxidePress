# OxidePress — agent notes

OxidePress is a GPL-2.0-only fork of RustPress by Ryuji Yasukochi. Upstream history stays. `LICENSE` stays verbatim. Credit lives in `AUTHORS`.

A task touches one crate. Read this file, run the map, then open that crate.

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

## Do not change

- `LICENSE`, `themes/`, `static/`, `wp-content/`, and the upstream credit in `AUTHORS`.
- Cookie `rustpress_session` and `wp_options` keys `_rustpress_session_`.
- Tables `rustpress_sessions`, `rustpress_cache`, `rustpress_migrations`.
- Template key `rustpress_version` (themes still read it).
- Env vars `RUSTPRESS_HOST`, `RUSTPRESS_PORT`, `RUSTPRESS_URL`, `RUSTPRESS_UPLOAD_DIR`.
- WordPress tables (`wp_posts`, `wp_options`, and the rest) and password hashes.

`tools/rename.ps1` is the rebrand codemod. Dry-run is the default. It is not a general editor.

## Change notices

On a file you modify, keep any existing copyright lines. If the file has no fork notice yet, add:

```rust
// Modified by Vabian124 on YYYY-MM-DD to <what changed>.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
```

Use the date of the change. Describe the change you actually made.
