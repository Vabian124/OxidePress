# Local alpha status

Checked on 2026-11-07 against a local OxidePress site (Twenty Twenty-Five, MySQL 8).

The shop is OxidePress commerce. It stores a product and an order in `wp_posts` using the WooCommerce post types `product` and `shop_order`, and a shop manager can edit the price and picture. It is a small slice of that data model: one simple product, a cart cookie, and a checkout that records the order. Variations, coupons, shipping rates, tax, payment gateways, stock changes, and the WooCommerce lookup tables are not implemented. A picture is a URL in post meta, not an attachment in the media library. The cart and the thank-you page live in memory and reset when the server restarts. The order row remains.

PHP plugins from WordPress.org do not run. The names Yoast, ACF, Contact Form 7, Wordfence, and WooCommerce in the admin are OxidePress screens that read and write the same kinds of rows.

## Five plugin screens

| Screen | What was saved | What a visitor sees |
|---|---|---|
| SEO | Homepage title “Linen Workshop” and its meta description stay in the Yoast option keys. | The public homepage title stays the site title. Per-post Yoast keys are applied when a post has them. The sitemap checkboxes are shown and are not stored. |
| Custom Fields | Field group “Care card” is an `acf-field-group` post with one `acf-field` child and a product location. | The product editor does not show the field. |
| Contact Forms | “Ask the workshop” is a `wpcf7_contact_form` post. | No shortcode renders it. The built-in shortcodes are caption, audio, video, gallery, and embed. |
| Security | `203.0.113.50` stays in the `wf_blocked_ips` option. The scan runs on the dashboard. | The block list is not consulted when a request arrives. Saving firewall, rate-limit, or login settings does not change the checked boxes on the next load. |
| hello-oxide | Plugins shows the manifest and activation returns Active. | The manifest does not add HTML. Activation is in memory, so a restart marks hello-oxide, site-notice, and oxide-store inactive again. |

The four admin screens were blank until they used the shared admin context. The first custom-field save stored a group with zero fields because the form posted `field_label[]`, which the handler did not read. A single label is now accepted, and the saved location is shown on the list.
