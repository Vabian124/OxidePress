# WordPress 7.1.4 single-site plan

Checked against `gaps/schema.json`, `gaps/blocks.json`, and the local site on 2026-11-07.

1:1 here means a single-site WordPress 7.1.4 database, the core block names, and the public and admin behavior those names imply. A dropped-in PHP plugin can boot in one `oxidepress-php` worker and contribute hooks and head/footer echo. That boot is partial. It is not a WordPress process, and the native screens are not a claim that the WordPress.org plugins are fully reimplemented. Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce stay on `oxidepress-seo`, `oxidepress-fields`, `oxidepress-forms`, `oxidepress-security`, and `oxidepress-commerce`. The probe record is [crates/oxidepress-php/REAL-PLUGINS.md](../crates/oxidepress-php/REAL-PLUGINS.md). The decision is [ADR-002](adr/002-php-worker.en.md).

Multisite stays out: `gaps/schema.json` lists `wp_blogs`, `wp_blogmeta`, `wp_site`, `wp_sitemeta`, `wp_signups`, and `wp_registration_log` as absent on purpose.

Each code item names one crate. A docs item says `docs`. Finish that item before opening the next.

## Already aligned

- Single-site tables match the 7.1.4 `schema.php`. `missing_tables` is empty. `db_version` is 61833. Older databases still need the two `existing_database_repairs`.
- All 115 core block names from WordPress 7.1.4 are present in the renderer and the REST catalog. `missing_from_renderer` and `missing_from_rest` are empty. `core/query-loop`, `core/row`, and `core/stack` are OxidePress aliases.
- The local site boots on MySQL 8, serves Twenty Twenty-Five, and the shop, cart, and checkout use that theme. Footer labels that were `href="#"` resolve to published pages.
- SEO, custom fields, contact forms, and security screens render and store their rows. A shop manager can edit a product and cannot open plugins.
- The public home page reads `wpseo_titles` (`title-home-wpseo` and `metadesc-home-wpseo`). The first import copies a saved `_yoast_wpseo_title-homepage` and `_yoast_wpseo_metadesc-homepage` into those keys. Posts, search, date archives, and the 404 use the official templates and separator. The header site name stays the blog name. `/sitemap_index.xml` lists the post and page sitemaps, and `robots.txt` points at that index.
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
- `wf_login_sec_enabled`, `wf_login_sec_maxFailures`, and `wf_login_sec_lockoutMins` control the next login attempt. A disabled lockout does not lock the address.
- `comment_moderation=1` keeps a new comment off the public page. When it is off, the comment is approved and listed.
- `/?s=word` lists published posts whose title or content contains that word. An empty result says “No results found.”
- `/{year}/`, `/{year}/{month}/`, and `/{year}/{month}/{day}/` list the published posts from that date.
- The public post lists approved comments only, and the stored comment count counts those same rows.
- A logged-in comment stores that account’s display name, email, and user id when the form leaves the name blank.
- When `require_name_email` is on, or the option is missing, a guest comment without an email is refused. A logged-in account can still comment.
- A reply with `comment_parent` is listed under that comment.
- A new comment stores the visitor address. The comments screen shows that address under the author.
- A signed-in visitor sees “Logged in as” their display name on the comment form, and the form does not ask for a name or email.
- The comments screen names the post a comment belongs to. The All count stays the full total when another status is selected.
- `/comments/feed/` names the post and links to that post’s permalink with the comment anchor. A recent-comments widget uses the same permalink.
- Approving, holding, spamming, trashing, or deleting a comment rewrites the post’s stored comment count to the approved rows.
- `/shop/`, `/cart/`, `/checkout/`, and `/shop/product/{id}/` serve the same pages as the links without a trailing slash. `/product/{slug}/` is the product permalink. It shows the stored sale price, SKU, and stock, and Add to cart puts that product in the cart.

## Worker limits, checked 2026-11-07

These are still true in `oxidepress-php` and `oxidepress-admin`. Nothing newer in this tree has done the code items below.

- `ABSPATH` is the plugins directory plus a slash. That is wrong for a real WordPress root.
- `POST /admin/plugins/{name}/activate` cannot contain a slash, so the route stores the display name. Restore accepts the display name or the basename.
- `wp_enqueue_script`, `wp_enqueue_style`, `register_setting`, post types, and admin menus stay inside the worker. They do not print assets or draw a settings screen.
- HTTP from the worker is absent.
- Deactivating one plugin needs a process restart. `current_user_can()` is false. `is_admin()` and `is_multisite()` are false. Activation callbacks are not run on load.

## Next, in order

1. Docs and ADR-002. `docs`. This page, the READMEs, and [ADR-002](adr/002-php-worker.en.md).
2. Give the worker a real WordPress-root `ABSPATH`. Do not vendor `wp-admin` or `wp-includes`. `oxidepress-php`.
3. Teach the admin activate route to store a basename that contains a slash. `oxidepress-admin`.
4. `wp_enqueue_script`, `wp_enqueue_style`, and admin-menu registrations stay inside the worker. `docs`. This pass records that. They do not print assets or draw a settings screen. A later server screen for them would be `oxidepress-server`.
5. HTTP from the worker, including the class of call Akismet's spam API needs. `oxidepress-php`. Do not start this by stubbing Contact Form 7's `require_once` of `wp-admin/includes/file.php`.

## Not this plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Vendoring `wp-admin` or `wp-includes`, or stubbing `wp-admin/includes/file.php`. A dropped-in plugin boots in the one worker above. That is not a WordPress tree, and the five native screens stay the path for those stored rows.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack. The plugin analyzer already marks those incompatible.
