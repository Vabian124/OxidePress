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

The worker now provides the path, URL, locale, translation, query-arg, transient, and widget helpers those plugins call while booting, plus `register_post_type` (returns a `WP_Post_Type`), `add_submenu_page` and the other `add_*_page` menu helpers, upload-directory helpers (`wp_upload_dir`, `wp_get_upload_dir`, `wp_mkdir_p`, `wp_is_writable`, `wp_normalize_path`), and a small nonce/`get_bloginfo` surface. `wp_enqueue_style` prints in `wp_head`. `wp_enqueue_script` prints in `wp_head`, or in `wp_footer` when the script is marked for the footer. `register_setting` is accepted and does not draw a settings screen. `current_user_can()` returns false.

| Plugin | Version | Result |
| --- | --- | --- |
| Akismet | 5.7.3a16 | Loaded (4 actions). Every probed hook returned. |
| Classic Editor | 1.7.0 | Loaded (1 action). Every probed hook returned. |
| Classic Widgets | 0.3 | Loaded (2 filters). `use_widgets_block_editor` came back `false`. |
| Disable Comments | 2.9.1 | Loaded (15 actions, 1 filter). Every probed hook returned. |
| Yoast Duplicate Post | 4.7 | Loaded (1 action, 1 filter). Every probed hook returned. Admin screens stay unloaded because `is_admin()` is false. |
| Hello Dolly | 1.7.2 | Loaded. `admin_notices` echoed the lyric paragraph (`<p id="dolly">…Quote from Hello Dolly song…`). `admin_head` echoed the `#dolly` stylesheet (354 bytes). |
| Health Check | 1.7.1 | Loaded (4 actions, 1 filter). Every probed hook returned, including `admin_menu`. Its `compat.php` still warns because it `include`s `ABSPATH . 'wp-includes/version.php'`, and this process is not a WordPress tree. |
| Contact Form 7 | 6.2.4.1 | Loaded (18 actions, 8 filters). `register_post_type()` and the upload-directory helpers returned. Stopped inside `plugins_loaded` when `includes/filesystem.php` `require_once`s `ABSPATH . 'wp-admin/includes/file.php'` (and the `WP_Filesystem_*` classes next to it). That is WordPress core's filesystem API, which this worker does not ship. |

## On the server

The server scans the configured plugins directory for the same `Plugin Name` headers. PHP plugins are registered under the WordPress basename (`hello-dolly/hello.php`) and can also be activated by the display name. At startup, every active PHP plugin is included in one worker. Echo from `wp_head` and `wp_footer` is appended to the theme HTML. If `php` is missing, the site still boots and those plugins are marked as errors.

`ABSPATH` is the directory that contains `wp-content` when the plugins directory is `wp-content/plugins`. An ancestor that contains `wp-load.php` is used otherwise. If neither exists, `ABSPATH` is the parent of the plugins directory. `WP_PLUGIN_DIR` stays the configured plugins directory. `wp-admin` and `wp-includes` are not part of this tree, so Health Check still warns when `version.php` is missing and Contact Form 7 still stops when `wp-admin/includes/file.php` is missing.

Removing one plugin needs a process restart: the hook registry cannot drop a single callback. The admin activate route stores the WordPress basename, including one that contains a slash. Post types and admin menu slugs are recorded in the worker only. `wp_remote_get` and `wp_remote_post` reach public HTTP and HTTPS, refuse private addresses, and do not follow redirects. Akismet's spam API is that class of call. Contact Form 7 still stops on the missing WordPress filesystem file.

`fixtures/boot-surface` locks the helpers the passing plugins rely on: widget subclassing, `__return_false`, `plugins_url`, `path_join`, `get_user_locale`, `get_site_option`, `add_query_arg`, `register_post_type('boot_form')`, `add_submenu_page`, and an escaped `admin_notices` echo.
