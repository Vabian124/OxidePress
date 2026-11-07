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

## Next, in order

The named single-site list above is in. These are the next holes a visitor or editor still hits.

| # | Work | Crate | Done when |
|---|---|---|---|
| 1 | Custom fields on posts and pages | `oxidepress-server` | A field group whose location is `post` or `page` shows on that editor and saves as post meta. The product editor already does this. The block editor does not. |
| 2 | Plugin activation survives restart | `oxidepress-server` | Activating a discovered plugin writes `active_plugins`, and the next boot loads that list. |
| 3 | Security screen reloads what it saved | `oxidepress-server` | Firewall, rate limit, and login protection checkboxes show the stored options, and a successful save says so. |

## Not this plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Loading PHP from WordPress.org. Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce stay native screens that read and write the same kinds of rows.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack. The plugin analyzer already marks those incompatible.
