# WordPress 7.1.4 single-site plan

Checked against `gaps/schema.json`, `gaps/blocks.json`, and the local site on 2026-11-07.

1:1 here means a single-site WordPress 7.1.4 database, the core block names, and the public and admin behavior those names imply. It does not mean executing PHP plugins. Multisite stays out: `gaps/schema.json` lists `wp_blogs`, `wp_blogmeta`, `wp_site`, `wp_sitemeta`, `wp_signups`, and `wp_registration_log` as absent on purpose.

Each item names one crate. Finish and commit that crate before opening the next item.

## Already aligned

- Single-site tables match the 7.1.4 `schema.php`. `missing_tables` is empty. `db_version` is 61833. Older databases still need the two `existing_database_repairs`.
- All 115 core block names from WordPress 7.1.4 are present in the renderer and the REST catalog. `missing_from_renderer` and `missing_from_rest` are empty. `core/query-loop`, `core/row`, and `core/stack` are OxidePress aliases.
- The local site boots on MySQL 8, serves Twenty Twenty-Five, and the shop, cart, and checkout use that theme. Footer labels that were `href="#"` resolve to published pages.
- SEO, custom fields, contact forms, and security screens render and store their rows. A shop manager can edit a product and cannot open plugins.

## Next, in order

These are the behaviors a visitor or editor still hits. The names exist. The output does not match WordPress yet.

| # | Work | Crate | Done when |
|---|---|---|---|
| 1 | Homepage title and meta description | `oxidepress-server` | A saved `_yoast_wpseo_title-homepage` and `_yoast_wpseo_metadesc-homepage` are the public home `<title>` and meta description. The header site name stays the blog name. |
| 2 | Query loop | `oxidepress-blocks` | `core/query` renders its inner template once per post supplied on the render context, and `core/query-total` reports that count. |
| 3 | Fill the query | `oxidepress-server` | Rendering a query block loads the published posts for that query and passes them in. |
| 4 | Page list | `oxidepress-blocks` | `core/page-list` lists the pages on the render context. A context with no pages still shows Home. |
| 5 | Breadcrumbs | `oxidepress-blocks` | `core/breadcrumbs` shows Home and the current post or page title, linked to its permalink. |
| 6 | Post navigation | `oxidepress-blocks` | `core/post-navigation-link` uses the previous and next permalinks on the render context. An empty side renders no link. |
| 7 | Wire navigation context | `oxidepress-server` | Single posts pass previous and next permalinks. Page lists pass published pages. |
| 8 | Sitemap checkboxes | `oxidepress-server` | The SEO screen stores and reloads include-posts and include-pages, and `/sitemap.xml` follows them. |
| 9 | Custom field on the editor | `oxidepress-server` | A field group whose location is `product` or `post` shows its fields on that editor and saves them as post meta. |
| 10 | Contact form shortcode | `oxidepress-forms` | `[contact-form-7 id="…"]` renders the stored `wpcf7_contact_form` content. |

## After that

| Work | Crate | Done when |
|---|---|---|
| Blocked addresses | `oxidepress-security` | An address stored in `wf_blocked_ips` is refused on the next request. The server's own address cannot be added. |
| Plugin list | `oxidepress-api` | `GET /wp-json/wp/v2/plugins` returns the discovered manifests and their active or inactive state. |
| Themes screen | `oxidepress-server` | Appearance → Themes lists the themes on disk and the active one. |
| Stock | `oxidepress-commerce` | A completed checkout decreases `_stock` and writes `_stock_status`. |
| Author, category, and tag archives | `oxidepress-server` | `/author/{slug}/`, `/category/{slug}/`, and `/tag/{slug}/` list the matching published posts. |

## Not this plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Loading PHP from WordPress.org. Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce stay native screens that read and write the same kinds of rows.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack. The plugin analyzer already marks those incompatible.
