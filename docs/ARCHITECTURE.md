# OxidePress Architecture

OxidePress is a GPL-2.0-only fork of RustPress by Ryuji Yasukochi. It serves a WordPress MySQL database from a Rust binary. Themes in this tree are Tera themes. PHP themes do not run.

```
                    HTTP Request
                         |
                         v
                  ┌─────────────┐
                  │   Axum      │  oxidepress-server (binary: oxidepress)
                  │   Router    │  Routes, middleware, static files
                  └──────┬──────┘
                         |
          ┌──────────────┼──────────────┐
          v              v              v
   ┌────────────┐ ┌────────────┐ ┌────────────┐
   │ Frontend   │ │ REST API   │ │ Admin      │
   │ Routes     │ │ /wp-json/  │ │ /wp-admin/ │
   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
         |               |              |
         v               v              v
   ┌────────────┐ ┌────────────┐ ┌────────────┐
   │ oxidepress-│ │ oxidepress-│ │ oxidepress-│
   │ themes     │ │ api        │ │ admin      │
   │ (Tera)     │ │            │ │            │
   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
         |               |              |
         └───────────────┼──────────────┘
                         v
                  ┌─────────────┐
                  │ oxidepress- │  Query builder
                  │ query       │
                  └──────┬──────┘
                         v
                  ┌─────────────┐
                  │ oxidepress- │  SeaORM entities, options
                  │ db          │
                  └──────┬──────┘
                         v
                  ┌─────────────┐
                  │   MySQL     │  WordPress wp_* tables
                  └─────────────┘

   One optional process, started only when an active plugin is PHP:

                  ┌─────────────┐
                  │ php CLI     │  oxidepress-php
                  │ one worker  │  length-prefixed JSON
                  └──────┬──────┘
                         v
                  oxidepress-core hooks and shortcodes
                  wp_head / wp_footer echo appended to the theme HTML
```

## Crate map

| Crate | Role |
|-------|------|
| `oxidepress-server` | HTTP server. Binary name `oxidepress` |
| `oxidepress-cli` | Command-line binary `oxidepress-cli` |
| `oxidepress-core` | Shared types, hooks, shortcodes |
| `oxidepress-db` | SeaORM entities and the options API |
| `oxidepress-query` | Query builder |
| `oxidepress-themes` | Tera themes and template tags. PHP themes do not run |
| `oxidepress-api` | WordPress-style REST endpoints |
| `oxidepress-admin` | Admin routes, including plugin activate and deactivate |
| `oxidepress-auth` | Sessions, passwords, roles |
| `oxidepress-plugins` | Plugin registry. Discovery of manifests, WASM files, and PHP headers |
| `oxidepress-php` | The PHP CLI worker. This is the drop-in runtime |
| `oxidepress-rustify` | Parser that emits a Rust skeleton. Not the runtime |
| `oxidepress-cache` | Cache |
| `oxidepress-cron` | Scheduled tasks |
| `oxidepress-migrate` | Schema migration against the WordPress tables |
| `oxidepress-i18n` | Translations |
| `oxidepress-blocks` | Block rendering and the REST block catalog |
| `oxidepress-e2e` | End-to-end tests |
| `oxidepress-seo` | Native screen for stored SEO rows |
| `oxidepress-fields` | Native screen for stored field groups |
| `oxidepress-forms` | Native screen for stored contact forms |
| `oxidepress-security` | Native screen for stored firewall and login rows |
| `oxidepress-commerce` | Native screen for stored shop rows |
| `oxidepress-multisite` | Present in the workspace. The single-site plan does not turn multisite on |

The five native screens read and write the same kinds of rows as Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce. They are not a claim that those WordPress.org plugins are fully reimplemented.

## PHP worker

`oxidepress-php` is one long-lived system `php` process. Rust speaks length-prefixed JSON. Hooks and shortcodes attach to `oxidepress-core`. Startup spawns that process only when an active plugin is `PluginType::Php`. The program is `OXIDEPRESS_PHP` or `php` on `PATH`. If `php` is missing, the site still boots and those plugins are marked Error.

When plugins live in `wp-content/plugins`, `ABSPATH` is the directory that contains `wp-content`. An ancestor with `wp-load.php` is used otherwise. If neither exists, `ABSPATH` is the parent of the plugins directory. Core files are not vendored. Post types, admin menus, and script and style registrations stay inside the worker. HTTP from the worker is absent. The probe record is [crates/oxidepress-php/REAL-PLUGINS.md](../crates/oxidepress-php/REAL-PLUGINS.md). The decision is [ADR-002](adr/002-php-worker.en.md).

Requests for a `.php` file are blocked by middleware in `oxidepress-server`, apart from the routes the server itself handles. The worker is a separate process. It is not a request uploading a `.php` file into the web root.

## Names that stay

- Session cookie `rustpress_session`, and `wp_options` keys `_rustpress_session_`.
- Tables `rustpress_sessions`, `rustpress_cache`, `rustpress_migrations`.
- Template key `rustpress_version`. Themes still read it.
- Env vars `RUSTPRESS_HOST`, `RUSTPRESS_PORT`, `RUSTPRESS_URL`, `RUSTPRESS_UPLOAD_DIR`. The server also reads `DATABASE_URL`, `SITE_URL`, `PLUGINS_DIR`, and `SKIP_MIGRATIONS`.
- WordPress table names (`wp_posts`, `wp_options`, and the rest).

## Database

`SKIP_MIGRATIONS=true` uses the `wp_*` tables already in `DATABASE_URL`. Schema drift is `gaps/schema.json`, from `pwsh -File tools/wp-gap.ps1`, compared with `oxidepress-migrate`.

## Directory layout

```
OxidePress/
├── crates/              # oxidepress-* crates
├── templates/           # Tera templates, including admin
├── themes/              # Tera theme assets. PHP themes do not run
├── static/
├── docs/
│   └── adr/             # Architecture Decision Records
├── gaps/                # schema.json and blocks.json
└── wp-content/plugins/  # default PLUGINS_DIR
```

## Development workflow

Read [AGENTS.md](../AGENTS.md), then [CONTRIBUTING.md](../CONTRIBUTING.md). One task, one crate. Find it with `pwsh -File tools/task.ps1 -Keyword <word>`.
