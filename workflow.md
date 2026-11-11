# OxidePress workflow

Read [AGENTS.md](AGENTS.md) first. This file is the loop for one feature. The behavior record is [docs/compatibility-plan.md](docs/compatibility-plan.md). The lists are [docs/TODO.md](docs/TODO.md) and [docs/progress.json](docs/progress.json).

OxidePress serves a WordPress database from a Rust binary. Themes are Tera themes. A dropped-in PHP plugin boots in one worker. Native screens stay the path for stored SEO, fields, forms, security, and shop rows.

Do one feature, then stop. Ship it before starting the next.

## 1. Check the tracking files

Open these and see that they describe the same tree:

- [docs/TODO.md](docs/TODO.md)
- [docs/progress.json](docs/progress.json)
- [docs/compatibility-plan.md](docs/compatibility-plan.md)
- [docs/alpha-status.md](docs/alpha-status.md)
- [README.md](README.md) and [README.ja.md](README.ja.md)
- [docs/adr/002-php-worker.en.md](docs/adr/002-php-worker.en.md) and [docs/adr/002-php-worker.md](docs/adr/002-php-worker.md)
- [crates/oxidepress-php/REAL-PLUGINS.md](crates/oxidepress-php/REAL-PLUGINS.md)

`progress.json` `"open"` and `"later"` match the Open and Later sections in `TODO.md`. `"commit"` is the latest feature commit, not a docs-only pointer. `"updated"` is the date of that feature. If a sentence in one file is missing from the others, fix the files before writing code.

Do not read `MASTERPLAN.md` or `docs/instructions/`. Those are not the task list.

## 2. Get the goal

Take the next item under Open, then Later. The numbered single-site plan is already done. An empty Open section means Later is next. An empty Later section means the next gap is a plugin call the worker still does not store, and that call is not on the out-of-plan list.

Stay inside the product. A plugin contributes hooks, shortcodes, head and footer echo, and rows the host writes. It does not become WordPress.

Leave these alone:

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Vendoring `wp-admin` or `wp-includes`, or stubbing `wp-admin/includes/file.php`.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack.

## 3. Find the missing feature

Name one behavior a plugin can call and a row OxidePress can store. Look at the worker limits in the compatibility plan and at `REAL-PLUGINS.md` before choosing it. Do not rebuild a feature that is already in Done.

Map the crate, then open that crate:

```powershell
pwsh -File tools/task.ps1 -Keyword <area>
```

One feature, one crate, when the work fits in one crate. A plugin call that the worker, the server host, and an admin screen all need can span those crates. Do not edit `crates.map.json` to make the map accept it.

## 4. Build

Match the shape of the last shipped plugin feature.

- Add the PHP function in `crates/oxidepress-php/php/worker.php`.
- Add a `PhpHost` method with a default, so a host that does not store that row still compiles.
- Store it in `MemHost` for tests and in `OptionHost` for MySQL.
- Keep the host call from re-entering the PHP worker. Do not call another `OptionHost` method that also waits on the runtime from inside one that is already waiting.
- Refuse internal types, reserved names, protected option keys, and values the column cannot hold. Say that limit in the docs.
- On a Rust or PHP file you change, keep the existing copyright lines. If it has no fork notice, add the three lines from `AGENTS.md` and describe the change you made.

Do not change `LICENSE`, `themes/`, `static/`, `wp-content/`, or the upstream credit in `AUTHORS`. Do not rename `rustpress_session`, `_rustpress_session_` keys, the `rustpress_sessions`, `rustpress_cache`, or `rustpress_migrations` tables, `rustpress_version`, the `RUSTPRESS_*` env vars, or `wp_*` table names.

`oxidepress-server` has no library target. Its tests run as `cargo test -p oxidepress-server --bin oxidepress`.

## 5. Test

Set the local PHP binary when the test spawns the worker:

```powershell
$env:OXIDEPRESS_PHP = "C:\Users\fcsta\AppData\Local\Microsoft\WinGet\Packages\PHP.PHP.8.3_Microsoft.Winget.Source_8wekyb3d8bbwe\php.exe"
```

While editing:

```powershell
cargo test -p <package> --lib
cargo fmt -p <package> -- --check
cargo clippy -p <package> -- -D warnings
```

`cargo test` takes one filter. Run each new test on its own. CI does not install PHP, so a worker test must skip when `php` is missing.

Before pushing a wide change:

```powershell
cargo check --workspace
cargo test --workspace --lib --bins -- --skip e2e
cargo fmt --all -- --check
```

CI uses Rust 1.97 with `RUSTFLAGS=-Dwarnings`. A local Clippy that is newer can flag code CI still accepts. Do not edit that code unless a CI run fails on it.

Say what you did not run. A Tera render or a `MemHost` round trip is not a click in the admin, and an `OptionHost` path is not a live MySQL run until it was run.

## 6. Document

Update the same files from step 1 in the same commit as the feature, except the commit hash. The hash does not exist yet.

- Add a Done line to `docs/TODO.md`.
- Add a `done` item to `docs/progress.json` and extend `"summary"`.
- Add one sentence to the worker limits in `docs/compatibility-plan.md`, both READMEs, both ADR-002 files, `docs/alpha-status.md`, and `REAL-PLUGINS.md`.
- Write the Japanese files in Japanese.
- Leave Open and Later empty when they are empty. Do not invent a next item.

## 7. Commit

Commit only the feature you just finished. `feat:` for behavior, `fix:` for a break, `docs:` for tracking only. Pass the message with a PowerShell here-string. Do not commit secrets, `themes/twentytwentyfive/static/theme-generated.css`, or anything in the do-not-change list.

After that commit exists, put its hash on the Done line and on the `progress.json` item. Set the top-level `"commit"` to that feature hash, not to the docs commit that records it. Commit that pointer on its own.

Do not force-push. Do not skip hooks.

## 8. Push and watch CI

`origin` is `https://github.com/Vabian124/OxidePress.git`. `gh` defaults to the upstream RustPress repo, so pass the fork every time:

```powershell
git push origin HEAD
gh run list --repo Vabian124/OxidePress --limit 1
gh run watch <id> --repo Vabian124/OxidePress --exit-status
```

Green means Check, Test, Format, Clippy, Security Audit, and Build Release. Release Build runs only on a `v*` tag and stays skipped on `main`.

If Security Audit fails on an advisory that has a patched release, bump that crate and push again. Keep these ignores: `RUSTSEC-2023-0071`, `RUSTSEC-2024-0436`, `RUSTSEC-2026-0235`, and `RUSTSEC-2026-0459`. `core2` 0.4.0 stays yanked because `rav1e` still requires that line. A warning there is not a failed job.

Watch until the run exits 0. Then stop.
