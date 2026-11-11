# OxidePress TODO

Updated 2026-11-11. The behavior record is [compatibility-plan.md](compatibility-plan.md). Machine-readable status is [progress.json](progress.json). The local site note is [alpha-status.md](alpha-status.md).

The numbered single-site plan is done. Take the open items in order. One item, one crate, then update both this file and `progress.json`.

## Open

None. The later list is next.

## Later

None. The open list is empty.

## Out of plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Vendoring `wp-admin` or `wp-includes`, or stubbing `wp-admin/includes/file.php`.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack.

## Done

- Single-site tables, core blocks, theme, comments, archives, SEO, fields, forms, security, and the shop slice. [compatibility-plan.md](compatibility-plan.md).
- One PHP worker: hooks, shortcodes, head and footer echo. [ADR-002](adr/002-php-worker.en.md).
- `ABSPATH` follows the install layout. `51aa88b`.
- A plugin basename may contain a slash. `bed2158`.
- `wp_remote_get` and `wp_remote_post` reach public HTTP and refuse private addresses. `557af24`.
- Enqueued styles and scripts print. `f78e4c8`.
- Plugin settings pages save registered options and refuse protected keys. `9f32b31`.
- A plugin post type stores rows. An administrator lists and edits them. The public address is `/{slug}/`. Revisions and template types are refused. `321fb88`.
- A `show_ui` plugin post type opens in the block editor. Saving it cannot switch the row to a refused type. `9934a4a`.
- `get_post_meta`, `update_post_meta`, `add_post_meta`, and `delete_post_meta` read and write `wp_postmeta`. A value has to be text. An array is refused. `ff248f8`.
- `register_taxonomy` adds a `show_ui` taxonomy to the admin list at `/wp-admin/edit-tags.php?taxonomy={name}`. `category`, `post_tag`, `nav_menu`, and `post_format` stay reserved. `156c559`.
- `register_activation_hook` runs once when an administrator activates that plugin. Boot does not run it again.
- `is_admin()` follows `/wp-admin` and `/wp-login.php`. `current_user_can()` follows the signed-in role. A visitor is false.
- Deactivating one plugin stops its hooks and shortcodes. The other plugins keep running.
