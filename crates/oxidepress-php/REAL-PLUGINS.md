# Real WordPress plugins on the PHP worker

Date: 2026-11-07. Runtime: `oxidepress-php` with PHP 8.3.36 (cli).

Eight current plugins were downloaded from `https://downloads.wordpress.org/plugin/<slug>.zip` into a temporary directory and loaded with:

```text
cargo run -p oxidepress-php --bin probe -- <plugins-dir>
```

The probe includes each plugin, attaches its registrations, then fires `plugins_loaded`, `init`, `wp_loaded`, `admin_init`, `admin_menu`, `admin_notices`, `admin_head`, `wp_enqueue_scripts`, `wp_head`, and `wp_footer`. `is_admin()` is false, so this is a front-end boot. Admin-only files that a plugin includes only when `is_admin()` is true did not run.

## Before the boot helpers

| Plugin | Version | Result |
| --- | --- | --- |
| Akismet | 5.7.3a16 | Load failed: `Class "WP_Widget" not found` |
| Classic Editor | 1.7.0 | Loaded, then `has_action()` on `plugins_loaded` |
| Classic Widgets | 0.3 | Loaded and every probed hook returned |
| Contact Form 7 | 6.2.4.1 | Load failed: `untrailingslashit()` |
| Disable Comments | 2.9.1 | Load failed: `plugins_url()` |
| Yoast Duplicate Post | 4.7 | Load failed: `is_admin()` |
| Health Check | 1.7.1 | Load failed: `plugins_url()` |
| Hello Dolly | 1.7.2 | Loaded, then `wptexturize()` on `admin_notices` |

## After the boot helpers

The worker now provides the path, URL, locale, translation, query-arg, transient, and widget helpers those plugins call while booting. `wp_enqueue_script` / `wp_enqueue_style` and `register_setting` are accepted and do not yet print assets or draw a settings screen.

| Plugin | Version | Result |
| --- | --- | --- |
| Akismet | 5.7.3a16 | Loaded (4 actions). Every probed hook returned. |
| Classic Editor | 1.7.0 | Loaded (1 action). Every probed hook returned. |
| Classic Widgets | 0.3 | Loaded (2 filters). `use_widgets_block_editor` came back `false`. |
| Disable Comments | 2.9.1 | Loaded (15 actions, 1 filter). Every probed hook returned. |
| Yoast Duplicate Post | 4.7 | Loaded (1 action, 1 filter). Every probed hook returned. Admin screens stay unloaded because `is_admin()` is false. |
| Hello Dolly | 1.7.2 | Loaded. `admin_notices` echoed the lyric paragraph (`<p id="dolly">…Quote from Hello Dolly song…`). `admin_head` echoed the `#dolly` stylesheet (354 bytes). |
| Contact Form 7 | 6.2.4.1 | Loaded (18 actions, 8 filters). Stopped inside `plugins_loaded` on `register_post_type()`. |
| Health Check | 1.7.1 | Loaded (4 actions, 1 filter). Its `compat.php` warns because it `include`s `ABSPATH . 'wp-includes/version.php'`, and this process is not a WordPress tree. Stopped on `admin_init` at `add_submenu_page()`. |

## What this does not mean

The live OxidePress server still does not scan `wp-content/plugins` for `.php` files, and `wp_head` output from an action is not inserted into the theme yet. A plugin that boots here has registered its hooks. It has not gained post types, admin menus, or HTTP calls such as Akismet's spam API.

`fixtures/boot-surface` locks the helpers the passing plugins rely on: widget subclassing, `__return_false`, `plugins_url`, `path_join`, `get_user_locale`, `get_site_option`, `add_query_arg`, and an escaped `admin_notices` echo.
