# WordPress 7.1.4 single-site plan

Checked against `gaps/schema.json`, `gaps/blocks.json`, and the local site on 2026-11-07.

1:1 here means a single-site WordPress 7.1.4 database, the core block names, and the public and admin behavior those names imply. It does not mean executing PHP plugins. Multisite stays out: `gaps/schema.json` lists `wp_blogs`, `wp_blogmeta`, `wp_site`, `wp_sitemeta`, `wp_signups`, and `wp_registration_log` as absent on purpose.

Each item names one crate. Finish and commit that crate before opening the next item.

## Already aligned

- Single-site tables match the 7.1.4 `schema.php`. `missing_tables` is empty. `db_version` is 61833. Older databases still need the two `existing_database_repairs`.
- All 115 core block names from WordPress 7.1.4 are present in the renderer and the REST catalog. `missing_from_renderer` and `missing_from_rest` are empty. `core/query-loop`, `core/row`, and `core/stack` are OxidePress aliases.
- The local site boots on MySQL 8, serves Twenty Twenty-Five, and the shop, cart, and checkout use that theme. Footer labels that were `href="#"` resolve to published pages.
- SEO, custom fields, contact forms, and security screens render and store their rows. A shop manager can edit a product and cannot open plugins.
- The public home page uses a saved `_yoast_wpseo_title-homepage` and `_yoast_wpseo_metadesc-homepage`. The header site name stays the blog name.
- `core/query` repeats its template for the published posts on the render context. Page lists, breadcrumbs, and post navigation use the pages and permalinks on that context.
- The SEO sitemap checkboxes are stored and `/sitemap.xml` follows them.
- A field group whose location is `product` or `all` shows on the product editor and saves as post meta.
- `[contact-form-7 id="…"]` renders the stored form.
- An address in `wf_blocked_ips` is refused. `127.0.0.1` and `::1` cannot be added.
- `GET /wp-json/wp/v2/plugins` returns discovered manifests and their active or inactive state.
- Appearance → Themes lists the themes on disk and marks the theme the server is serving.
- Checkout decreases `_stock` and writes `_stock_status` for products that track stock.
- `/author/{slug}/`, `/category/{slug}/`, and `/tag/{slug}/` query the matching published posts and render the theme archive template.
- Field groups for `post`, `page`, `product`, and `all` show on the matching editor and save as post meta.
- Activating a plugin writes `active_plugins`. The next boot marks those discovered plugins active.
- The security screen reloads the firewall, rate limit, and login settings it stored, and a save says so.
- `wf_firewall_enabled=0` skips the firewall scan. The block list still refuses stored addresses. A missing option keeps the scan on.
- `wf_rate_limit_maxRequestsPerMin` is the per-minute ceiling. `wf_rate_limit_enabled=0` does not refuse.
- The public comment form accepts WordPress field names. An approved comment appears on the post.

## Next, in order

| # | Work | Crate | Done when |
|---|---|---|---|
| 1 | Login lockout uses the saved settings | `oxidepress-server` | `wf_login_sec_enabled`, `wf_login_sec_maxFailures`, and `wf_login_sec_lockoutMins` control the next login attempt. A disabled lockout does not lock the address. |
| 2 | Comment moderation | `oxidepress-server` | When `comment_moderation` is on, a new comment stays unapproved and the public page does not list it. When it is off, the comment is approved and listed. |
| 3 | Search results | `oxidepress-server` | `/?s=word` lists published posts whose title or content contains that word, and an empty search says so. |

## Not this plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Loading PHP from WordPress.org. Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce stay native screens that read and write the same kinds of rows.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack. The plugin analyzer already marks those incompatible.
