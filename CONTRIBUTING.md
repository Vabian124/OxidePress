# Contributing to OxidePress

OxidePress is an independent GPL-2.0-only fork of RustPress by Ryuji Yasukochi. See [AUTHORS](AUTHORS). One task, one crate, one pull request. Find the crate with `pwsh -File tools/task.ps1 -Keyword <word>` and read [AGENTS.md](AGENTS.md) before editing. The crate map is [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Requirements

- Rust 1.97+ (the CI toolchain)
- MySQL 8 or MariaDB 10.5+
- Docker, only for the end-to-end harness in `docker-compose.yml`

Copy `.env.example` to `.env` and set `DATABASE_URL`. `RUSTPRESS_HOST` and `RUSTPRESS_PORT` keep those names. A PHP plugin needs `php` on `PATH` or `OXIDEPRESS_PHP`. If `php` is missing, the site still boots.

```powershell
cargo run -p oxidepress-server
```

## Verify

Scoped, while editing:

```powershell
cargo check -p <package>
cargo test -p <package> --lib
cargo fmt -p <package> -- --check
```

Before a pull request:

```powershell
cargo check --workspace
cargo test --workspace --lib --bins -- --skip e2e
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

CI on `main` and on pull requests also runs `cargo audit` and `cargo build --release --workspace`. `RUSTFLAGS=-Dwarnings`. The audit ignores are `RUSTSEC-2023-0071` (rsa Marvin, no upstream fix), `RUSTSEC-2024-0436` (paste unmaintained), and `RUSTSEC-2026-0235` (rkyv 0.7 is an optional `rust_decimal` feature this workspace does not enable). That last ignore is not an rkyv upgrade. Wasmtime is pinned to `=50.0.1`. Release Build runs only on `v*` tags. The workflow is [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## Workflow

1. Fork [Vabian124/OxidePress](https://github.com/Vabian124/OxidePress) and clone your fork.
2. Branch from `main`. Name it `fix/<crate>-<thing>` or `feat/<crate>-<thing>`.
3. Run `pwsh -File tools/task.ps1 -Keyword <area>` and edit that crate.
4. Open a pull request and fill in the template.
5. Recent maintainer commits landed on `main`. Contributors still open pull requests from `fix/<crate>-<thing>` or `feat/<crate>-<thing>`.

Leave these alone unless the task names them: cookie `rustpress_session`, option keys `_rustpress_session_`, tables `rustpress_sessions` / `rustpress_cache` / `rustpress_migrations`, template key `rustpress_version`, env vars `RUSTPRESS_*`, WordPress `wp_*` tables, `themes/`, `static/`, `wp-content/`, and `LICENSE`. The upstream credit in `AUTHORS` stays.

## Reporting bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md). Include steps to reproduce. For a WordPress behavior difference, include the WordPress output and the OxidePress output.

## Feature requests

Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md). A code change names one crate. The open items are in [docs/compatibility-plan.md](docs/compatibility-plan.md).

## Crate table

| Crate | Role |
|-------|------|
| `oxidepress-server` | HTTP server, binary `oxidepress` |
| `oxidepress-cli` | CLI binary `oxidepress-cli` |
| `oxidepress-core` | Hooks, shortcodes, shared types |
| `oxidepress-db` | Entities and options |
| `oxidepress-query` | Query builder |
| `oxidepress-themes` | Tera themes. PHP themes do not run |
| `oxidepress-api` | REST endpoints |
| `oxidepress-admin` | Admin routes, including plugin activate |
| `oxidepress-auth` | Sessions, passwords, roles |
| `oxidepress-plugins` | Plugin registry |
| `oxidepress-php` | PHP CLI worker, the drop-in runtime |
| `oxidepress-rustify` | Parser skeleton. Not the runtime |
| `oxidepress-cache` | Cache |
| `oxidepress-cron` | Scheduled tasks |
| `oxidepress-migrate` | Schema migration |
| `oxidepress-blocks` | Block renderer and REST catalog |
| `oxidepress-seo` | Stored SEO rows |
| `oxidepress-fields` | Stored field groups |
| `oxidepress-forms` | Stored contact forms |
| `oxidepress-security` | Stored firewall and login rows |
| `oxidepress-commerce` | Stored shop rows |
| `oxidepress-i18n` | Translations |
| `oxidepress-multisite` | In the workspace. The single-site plan leaves it off |
| `oxidepress-e2e` | End-to-end tests |

## Code style

Run `cargo fmt` and `cargo clippy -- -D warnings`. Keep a change inside the crate the task names.

## License

By contributing, you agree that your contributions will be licensed under GPL-2.0-only.
