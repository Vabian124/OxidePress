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

## Worker limits, checked 2026-11-09

`ABSPATH` follows the install layout. Plugins in `wp-content/plugins` use the directory that contains `wp-content`. An ancestor with `wp-load.php` is used otherwise. If neither exists, `ABSPATH` is the parent of the plugins directory. `wp-admin` and `wp-includes` are not vendored, so a missing `version.php` or `wp-admin/includes/file.php` still fails the same way.

Activating or deactivating a plugin writes `active_plugins` as the WordPress basename when the plugin has one, including a basename that contains a slash (`hello-dolly/hello.php`). A plugin without a basename still stores its display name. Restore accepts either form.

These are still true:

- `wp_enqueue_style` prints in `wp_head`. `wp_enqueue_script` prints in `wp_head`, or in `wp_footer` when marked for the footer. An administrator opens a plugin settings page at `/wp-admin/admin.php?page={slug}`. Registered options save at `/wp-admin/options.php`. `siteurl`, `active_plugins`, role options, salts, and `rustpress_*` keys are refused.
- A plugin that registers a post type on `init` stores rows with `wp_insert_post`, `get_post`, `get_posts`, `wp_update_post`, and `wp_delete_post`. An administrator lists and edits that type at `/wp-admin/edit.php?post_type={name}`. The public permalink is `/{slug}/`. Revisions and template types are refused. A `show_ui` type opens in the block editor, and saving it cannot switch the row to a refused type.
- Deactivating one plugin stops its hooks and shortcodes and leaves the others running. `is_admin()` is true for `/wp-admin` and `/wp-login.php`. `current_user_can()` follows the signed-in role, and a visitor is false. While a plugin settings page is drawn, `manage_options` stays true. `is_multisite()` is false. `register_activation_hook` runs once when an administrator activates that plugin. It does not run again on boot.

`wp_remote_get` and `wp_remote_post` reach public HTTP and HTTPS. Private addresses, internal hostnames, and other schemes are refused before a socket opens. Redirects are not followed. A response body larger than 1 MiB is refused. This is the class of call Akismet's spam API makes. It does not vendor `wp-admin/includes/file.php`.

## Next, in order

The numbered items in this plan are done. The remaining work, in order, is [TODO.md](TODO.md). [progress.json](progress.json) is the same list for a program. What is already true is listed under the worker limits above.

## Not this plan

- Multisite tables and the extra `wp_users` columns `spam` and `deleted`.
- Vendoring `wp-admin` or `wp-includes`, or stubbing `wp-admin/includes/file.php`. A dropped-in plugin boots in the one worker above. That is not a WordPress tree, and the five native screens stay the path for those stored rows.
- WooCommerce lookup tables, variations, coupons, shipping rates, tax, and payment gateways.
- Elementor and Jetpack. The plugin analyzer already marks those incompatible.
