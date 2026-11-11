# OxidePress

Independent fork of [RustPress](https://github.com/LegacyToRustProject/RustPress) by Ryuji Yasukochi. Upstream went inactive, so this tree continues the same GPL-2.0-only work under a new name. The original `LICENSE` is unchanged. See [AUTHORS](AUTHORS).

[![CI](https://github.com/Vabian124/OxidePress/actions/workflows/ci.yml/badge.svg)](https://github.com/Vabian124/OxidePress/actions/workflows/ci.yml)
[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.97+](https://img.shields.io/badge/Rust-1.97%2B-orange.svg)](https://www.rust-lang.org/)

[日本語](README.ja.md)

OxidePress serves a WordPress database from a Rust binary. Themes in this tree are Tera themes. PHP themes do not run. A dropped-in PHP plugin can boot in one worker and contribute hooks and head/footer echo. That is partial. It is not WordPress. The native screens remain the path for stored SEO, fields, forms, security, and shop rows. Full AI conversion of every plugin is not the current runtime. [`oxidepress-rustify`](crates/oxidepress-rustify) may stay as a later skeleton.

The schema and behavior record against WordPress 7.1.4 is [docs/compatibility-plan.md](docs/compatibility-plan.md). Remaining work is [docs/TODO.md](docs/TODO.md), and the same list is [docs/progress.json](docs/progress.json). The feature loop is [workflow.md](workflow.md). What the local site was doing on 2026-11-07 is [docs/alpha-status.md](docs/alpha-status.md).

## Two plugin paths

1. Native Rust screens read and write the same kinds of rows as Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce. Those crates are `oxidepress-seo`, `oxidepress-fields`, `oxidepress-forms`, `oxidepress-security`, and `oxidepress-commerce`. They are not a claim that the WordPress.org plugins are fully reimplemented.
2. Drop-in PHP. [`oxidepress-php`](crates/oxidepress-php) is the runtime. `oxidepress-rustify` is a parser skeleton and is not the runtime.

The PHP runtime is one long-lived system `php` CLI worker. Rust talks to it with length-prefixed JSON. Hooks and shortcodes attach to `oxidepress-core`. It is not PHP-FPM per hook, not one FPM call per request, and not Rust sitting in front of a full WordPress process. [ADR-001](docs/adr/001-php-bridge-mode.en.md) rejected those. [ADR-002](docs/adr/002-php-worker.en.md) accepts the single worker for the drop-in path.

## What a dropped-in plugin can do

Discovery reads a `Plugin Name:` header in the first 8 KiB of a top-level `*.php` file, or a PHP file one directory deep. It does not recurse further. Plugins register under the WordPress basename (`hello-dolly/hello.php`). Activation also accepts the display name.

Startup spawns one worker only if at least one active plugin is `PluginType::Php`. The program is `OXIDEPRESS_PHP` or `php` on `PATH`. If `php` is missing, the site still boots and those plugins are marked Error.

Echo from `wp_head` and `wp_footer` is appended to the theme HTML.

The probe on 2026-11-07 (PHP 8.3.36) is [crates/oxidepress-php/REAL-PLUGINS.md](crates/oxidepress-php/REAL-PLUGINS.md). Akismet, Classic Editor, Classic Widgets, Disable Comments, Yoast Duplicate Post, Hello Dolly, and Health Check finished the probe hooks. Contact Form 7 stopped while loading WordPress's filesystem API. This tree does not vendor `wp-admin` or `wp-includes`, and it does not stub that require.

## What a dropped-in plugin cannot do

- `ABSPATH` is the directory that contains `wp-content` when plugins live in `wp-content/plugins`. An ancestor with `wp-load.php` is used otherwise. If neither exists, `ABSPATH` is the parent of the plugins directory. It is not the plugins directory, and this tree does not vendor `wp-admin` or `wp-includes`.
- Deactivating one plugin stops its hooks and shortcodes and leaves the others running. Turning it back on uses the copy already in the worker.
- Activating a plugin stores its WordPress basename, including one that contains a slash. A plugin without a basename stores its display name. Restore accepts either form.
- `wp_enqueue_style` prints in `wp_head`. `wp_enqueue_script` prints in `wp_head`, or in `wp_footer` when the script asks for the footer. An administrator opens a plugin settings page at `/wp-admin/admin.php?page={slug}`. Options that page registered are saved at `/wp-admin/options.php`. `siteurl`, `active_plugins`, role options, salts, and `rustpress_*` keys are refused.
- A plugin that registers a post type on `init` can store, read, update, and delete those rows. An administrator lists and edits them at `/wp-admin/edit.php?post_type={name}`. The public address is `/{slug}/`. Revisions and template types are refused. A `show_ui` type opens in the block editor, and saving it cannot switch the row to a refused type. `get_post_meta` and `update_post_meta` read and write `wp_postmeta` for that row. A value has to be text. `register_taxonomy` adds a `show_ui` taxonomy at `/wp-admin/edit-tags.php?taxonomy={name}`. `category`, `post_tag`, and `nav_menu` stay reserved. `wp_insert_term` stores a term in a plugin taxonomy, `category`, or `post_tag`, and `wp_set_object_terms` assigns it to a post. `wp_insert_comment` stores a comment on an allowed post. The default status is hold. `get_term_meta` and `update_term_meta` read and write `wp_termmeta`. A value has to be text. `get_comment_meta` and `update_comment_meta` read and write `wp_commentmeta`. A value has to be text. A missing comment is refused. Deleting the comment removes that meta.
- `is_admin()` is true for `/wp-admin` and `/wp-login.php`. `current_user_can()` follows the signed-in role, and a visitor is false. While a plugin settings page is drawn, `manage_options` stays true. `is_multisite()` is false. `register_activation_hook` runs once when an administrator activates that plugin. It does not run again on boot.
- `wp_remote_get` and `wp_remote_post` reach public HTTP and HTTPS. Private addresses, internal hostnames, and other schemes are refused. Redirects are not followed.
- Health Check still warns when `ABSPATH . 'wp-includes/version.php'` is missing. Contact Form 7 still stops when `ABSPATH . 'wp-admin/includes/file.php'` is missing.

Multisite, WooCommerce lookup tables, variations, coupons, shipping, tax, payment gateways, Elementor, and Jetpack are outside the current plan.

## Run

Rust 1.97 or newer, and MySQL 8 or MariaDB 10.5 or newer. Copy `.env.example` to `.env` and point `DATABASE_URL` at the WordPress database. `RUSTPRESS_HOST` and `RUSTPRESS_PORT` keep those names. The server listens on `127.0.0.1:3000` when they are unset. `SKIP_MIGRATIONS=true` uses the tables already in that database.

```powershell
cargo run -p oxidepress-server
```

The release binary is `oxidepress` (`oxidepress.exe` on Windows), from `oxidepress-server`. The CLI binary is `oxidepress-cli`.

`docker-compose.yml` is the end-to-end harness. It is not a product install.

## CI

On `main` and on pull requests, GitHub Actions runs Check, Test, Format, Clippy, Build Release, and Security Audit. `RUSTFLAGS=-Dwarnings`. The toolchain is Rust 1.97. Release Build runs only on `v*` tags.

Wasmtime is pinned to `=50.0.1` because the 42 line has no patched release that clears the later advisories. `cargo audit` ignores three advisories, for the reasons in [`.github/workflows/ci.yml`](.github/workflows/ci.yml):

- `RUSTSEC-2023-0071` — rsa Marvin attack, no upstream fix
- `RUSTSEC-2024-0436` — paste is unmaintained
- `RUSTSEC-2026-0235` — rkyv 0.7 is an optional `rust_decimal` feature this workspace does not enable

That last ignore is not an rkyv upgrade.

## Docs

- [Architecture](docs/ARCHITECTURE.md)
- [Contributing](CONTRIBUTING.md)
- [Compatibility plan](docs/compatibility-plan.md)
- [ADR index](docs/adr/README.md)

## License

GPL-2.0-only, the same license as WordPress. See [LICENSE](LICENSE).
