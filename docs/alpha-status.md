# Local alpha status

Checked on 2026-11-07 against a local OxidePress site (Twenty Twenty-Five, MySQL 8). Reconciled the same day with [compatibility-plan.md](compatibility-plan.md) and the server code. The PHP worker record is [crates/oxidepress-php/REAL-PLUGINS.md](../crates/oxidepress-php/REAL-PLUGINS.md).

The shop is OxidePress commerce. It stores a product and an order in `wp_posts` using the WooCommerce post types `product` and `shop_order`, and a shop manager can edit the price and picture. It is a small slice of that data model: one simple product, a cart cookie, and a checkout that records the order. Checkout decreases `_stock` and writes `_stock_status` for products that track stock. Variations, coupons, shipping rates, tax, payment gateways, and the WooCommerce lookup tables are not implemented. A picture is a URL in post meta, not an attachment in the media library. The cart and the thank-you page live in memory and reset when the server restarts. The order row remains.

The shop, product, cart, and checkout pages use the active theme, including its stylesheets, header, and footer. Footer labels in the theme that were left as `href="#"` are filled from published pages. Blog opens the home page, Shop opens `/shop`, and About, FAQs, Authors, Events, Patterns, and Themes open their pages.

Themes in this tree are Tera themes. PHP themes do not run.

A dropped-in PHP plugin can boot in one `oxidepress-php` worker and contribute hooks and head/footer echo. That is partial. It is not WordPress. `ABSPATH` is the WordPress root when the plugins directory is `wp-content/plugins` or an ancestor contains `wp-load.php`. Otherwise it is the parent of the plugins directory. Core files are not vendored. Deactivating one plugin needs a process restart. The admin activate route stores the WordPress basename, including one that contains a slash. Enqueue and admin-menu registrations stay inside the worker. `wp_remote_get` and `wp_remote_post` reach public HTTP and HTTPS and refuse private addresses. `current_user_can()`, `is_admin()`, and `is_multisite()` are false. Activation callbacks are not run on load.

The names Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce in the admin are OxidePress screens (`oxidepress-seo`, `oxidepress-fields`, `oxidepress-forms`, `oxidepress-security`, `oxidepress-commerce`) that read and write the same kinds of rows. They are not a claim that the WordPress.org plugins are fully reimplemented.

## Five plugin screens

| Screen | What was saved | What a visitor sees |
|---|---|---|
| SEO | Homepage title and meta description stay in `wpseo_titles` (`title-home-wpseo`, `metadesc-home-wpseo`). The sitemap checkboxes are stored. | The public home page reads those keys. `/sitemap.xml` follows the checkboxes. `/sitemap_index.xml` lists the post and page sitemaps. |
| Custom Fields | A field group for `post`, `page`, `product`, or `all` is stored and saves as post meta. | The matching editor shows the group, including a product location on the product editor. |
| Contact Forms | A form is a `wpcf7_contact_form` post. | `[contact-form-7 id="…"]` renders the stored form. The built-in shortcodes caption, audio, video, gallery, and embed still render too. |
| Security | An address stays in `wf_blocked_ips`. Firewall, rate-limit, and login settings are stored. `127.0.0.1` and `::1` cannot be added. | A stored address is refused on the next request. `wf_firewall_enabled=0` skips the firewall scan and still refuses stored addresses. The rate-limit and login options control the next attempt. |
| Plugins | Activating a plugin writes `active_plugins` as the WordPress basename when the plugin has one. The next boot marks discovered plugins active. | Restore accepts the display name or the basename. |

The four admin screens render with the shared admin context. A single custom-field label is accepted, and the saved location is shown on the list.
