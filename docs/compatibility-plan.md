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

## Next, in order

| # | Work | Crate | Done when |
|---|---|---|---|
| 1 | Firewall switch is enforced | `oxidepress-server` | Turning the firewall off stops `wf_firewall_enabled=0` from blocking requests. Turning it on scans them again. The block list still refuses stored addresses either way. |
| 2 | Rate limit uses the saved ceiling | `oxidepress-server` | `wf_rate_limit_maxRequestsPerMin` is the limit the next request is measured against, and a disabled rate limit does not refuse. |
| 3 | Comments on the public post | `oxidepress-server` | A visitor can post a comment on an open post, and the public page lists approved comments. |

## Not this plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Loading PHP from WordPress.org. Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce stay native screens that read and write the same kinds of rows.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack. The plugin analyzer already marks those incompatible.
