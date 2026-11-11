# ADR-002: One PHP CLI worker for drop-in plugins

- **Status**: Accepted
- **Date**: 2026-11-07
- **Supersedes**: The [ADR-001](001-php-bridge-mode.en.md) conclusion that plugins are AI conversion only and that there is no PHP runtime, for the drop-in path
- **Related**: [crates/oxidepress-php/REAL-PLUGINS.md](../../crates/oxidepress-php/REAL-PLUGINS.md)

---

## Conclusion (TL;DR)

Drop-in PHP plugins run in one long-lived system `php` CLI worker. Native crates keep the rows OxidePress already owns: SEO, fields, forms, security, and shop. `oxidepress-rustify` is a parser skeleton and is not the runtime.

ADR-001 still rejects PHP-FPM per hook, one FPM call per request, and Rust as a proxy in front of a full WordPress process.

## Context

OxidePress serves a WordPress MySQL database from a Rust binary. Themes in this tree are Tera themes. PHP themes do not run.

ADR-001 (2026-03-07) chose full AI conversion and no PHP runtime. A site can still drop a PHP plugin into the plugins directory and expect its hooks to register. Rebuilding every plugin before that file can boot is not the path this tree is on.

The worker that landed in `oxidepress-php` is a single process. Rust sends length-prefixed JSON. The worker includes each active PHP plugin, returns hook and shortcode registrations, and runs a callback when Rust fires that tag. Echo from `wp_head` and `wp_footer` is appended to the theme HTML. `oxidepress-core` holds those hooks and shortcodes.

## Options considered

### PHP-FPM per hook — still rejected

ADR-001 option A. A page fires many hooks. One FastCGI round-trip per hook costs more than it saves. ADR-002 does not revive it.

### One PHP-FPM call per request — still rejected

ADR-001 option B. That is a Rust proxy in front of a WordPress bootstrap. ADR-002 does not revive it.

### Rust in front of a full WordPress process — still rejected

ADR-001 options B and D. Nginx in front of WordPress already does this. OxidePress stays the CMS. The worker is not a WordPress tree.

### AI conversion of every plugin as the current runtime — not this decision

ADR-001's conversion strategy can stay as later work. `oxidepress-rustify` parses a plugin header and literal `add_action`, `add_filter`, and `add_shortcode` calls, and emits a Rust skeleton. It does not execute PHP. It is not the drop-in runtime.

### One PHP CLI worker — accepted

Spawn `php` from `OXIDEPRESS_PHP` or `PATH` once, and only when at least one active plugin is `PluginType::Php`. If `php` is missing, the site still boots and those plugins are marked Error. Discovery reads a `Plugin Name:` header in the first 8 KiB of a top-level `*.php` file or a PHP file one directory deep.

The rows Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce store stay on the native screens: `oxidepress-seo`, `oxidepress-fields`, `oxidepress-forms`, `oxidepress-security`, and `oxidepress-commerce`. Those screens are not a claim that the WordPress.org plugins are fully reimplemented.

## Decision

One PHP CLI worker for drop-in plugins. Native crates for the rows OxidePress already owns.

## Rationale

A dropped-in file can register hooks without booting WordPress and without a FastCGI call per hook. The native screens already read and write the option and post rows those five plugins use. Splitting the two paths keeps the server a Rust CMS and gives the PHP file a place to run.

The probe on 2026-11-07 (PHP 8.3.36) is the behavior record: Akismet 5.7.3a16, Classic Editor 1.7.0, Classic Widgets 0.3, Disable Comments 2.9.1, Yoast Duplicate Post 4.7, Hello Dolly 1.7.2, and Health Check 1.7.1 finish the probe hooks. Contact Form 7 6.2.4.1 stops in `includes/filesystem.php` on `require_once ABSPATH . 'wp-admin/includes/file.php'`. This decision does not vendor `wp-admin/includes` or stub that file.

## Impact

What the worker does:

- Registers plugins under the WordPress basename (`hello-dolly/hello.php`). Activation also accepts the display name.
- Attaches hooks and shortcodes to `oxidepress-core`.
- Appends `wp_head` and `wp_footer` echo to the theme HTML.
- `wp_remote_get` and `wp_remote_post` reach public HTTP and HTTPS.

What it does not do:

- When plugins live in `wp-content/plugins`, `ABSPATH` is the directory that contains `wp-content`. An ancestor with `wp-load.php` is used otherwise. If neither exists, `ABSPATH` is the parent of the plugins directory. `wp-admin` and `wp-includes` are not vendored.
- Deactivating one plugin stops its hooks and shortcodes and leaves the others running. Turning it back on uses the copy already in the worker.
- `oxidepress-admin` stores the WordPress basename, including one that contains a slash. A plugin without a basename stores its display name. Restore accepts either form.
- `wp_enqueue_style` prints a `link` tag in `wp_head`. `wp_enqueue_script` prints a `script` tag in `wp_head`, or in `wp_footer` when the script is marked for the footer. The worker runs `wp_enqueue_scripts` once before the first of those prints when the host has not already fired it.
- An administrator opens a plugin page registered with `add_menu_page`, `add_options_page`, or `add_submenu_page` at `/wp-admin/admin.php?page={slug}`. The worker runs `admin_menu` once, then prints that page callback. `register_setting` options posted to `/wp-admin/options.php` are saved for that group only. `siteurl`, `home`, `active_plugins`, `template`, `stylesheet`, `wp_user_roles`, `users_can_register`, `default_role`, salts, and `rustpress_*` keys are refused even if a plugin registers them. A post type registered on `init` is stored in `wp_posts`. An administrator lists and edits it at `/wp-admin/edit.php?post_type={name}`. `get_permalink` is `/{slug}/`. Revisions and template types are refused. A `show_ui` type opens in the block editor, and saving it cannot switch the row to a refused type. `get_post_meta` and `update_post_meta` read and write `wp_postmeta` for that row. A value has to be text. `register_taxonomy` adds a `show_ui` taxonomy at `/wp-admin/edit-tags.php?taxonomy={name}`. `category`, `post_tag`, and `nav_menu` stay reserved. `wp_insert_term` stores a term in a plugin taxonomy, `category`, or `post_tag`, and `wp_set_object_terms` assigns it to a post. `wp_insert_comment` stores a comment on an allowed post. The default status is hold. `get_term_meta` and `update_term_meta` read and write `wp_termmeta`. A value has to be text. `get_comment_meta` and `update_comment_meta` read and write `wp_commentmeta`. A value has to be text. A missing comment is refused. Deleting the comment removes that meta.
- `is_admin()` is true for `/wp-admin` and `/wp-login.php`. `current_user_can()` follows the signed-in role, and a visitor is false. While a plugin settings page is drawn, `manage_options` stays true. `is_multisite()` is false. `register_activation_hook` runs once when an administrator activates that plugin. It does not run again on boot.
- Private addresses, internal hostnames, and non-HTTP schemes are refused. Redirects are not followed. A response larger than 1 MiB is refused.
- Health Check still warns when `ABSPATH . 'wp-includes/version.php'` is missing. Contact Form 7 still stops when `ABSPATH . 'wp-admin/includes/file.php'` is missing.

Requests for a `.php` file are still blocked by server middleware. The worker is a separate process the server starts. It is not a request uploading a `.php` file into the web root.
