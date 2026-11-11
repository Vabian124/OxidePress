<?php
// Modified by Vabian124 on 2026-11-11 to read and write term meta through the host.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//
// Long-lived plugin worker. Rust sends length-prefixed JSON requests on STDIN.
// A request may nest: this process writes a "call" frame and reads a "result"
// frame before it writes the final "ok" frame. Echoed plugin output is buffered
// so it never mixes with those frames.

ini_set('display_errors', 'stderr');
ini_set('display_startup_errors', '0');
ini_set('html_errors', '0');
error_reporting(E_ALL);
if (function_exists('stream_set_write_buffer')) {
    stream_set_write_buffer(STDOUT, 0);
}
if (function_exists('stream_set_read_buffer')) {
    stream_set_read_buffer(STDIN, 0);
}

$oxidepress_callbacks = array();
$oxidepress_batch = array();
$oxidepress_hooks = array('action' => array(), 'filter' => array());
$oxidepress_shortcodes = array();
$oxidepress_disabled = array();
$oxidepress_request_id = 0;
$oxidepress_plugin_file = '';
$oxidepress_fatal_sent = false;
$oxidepress_did = array();
$oxidepress_post_types = array();
$oxidepress_taxonomies = array();
$oxidepress_admin_menus = array();
$oxidepress_registered_settings = array();
$oxidepress_settings_sections = array();
$oxidepress_settings_fields = array();
$oxidepress_admin_depth = 0;
$oxidepress_admin_ready = false;
$oxidepress_rendering_page = '';
$oxidepress_settings_nonce = '';
$oxidepress_scripts = array();
$oxidepress_styles = array();
$oxidepress_script_queue = array();
$oxidepress_style_queue = array();
$oxidepress_done_scripts = array();
$oxidepress_done_styles = array();
$oxidepress_asset_hooks = false;
$GLOBALS['wp_version'] = '6.8.3';

$oxidepress_plugins_dir = oxidepress_env('OXIDEPRESS_PLUGINS_DIR', '');
if (!defined('ABSPATH')) {
    define('ABSPATH', oxidepress_abspath_from_plugins($oxidepress_plugins_dir));
}
if (!defined('WPINC')) {
    define('WPINC', 'wp-includes');
}
if (!defined('WP_CONTENT_DIR')) {
    define('WP_CONTENT_DIR', oxidepress_content_dir($oxidepress_plugins_dir, ABSPATH));
}
if (!defined('WP_PLUGIN_DIR')) {
    $oxidepress_plugin_dir = rtrim(oxidepress_normalize_path($oxidepress_plugins_dir), '/');
    if ($oxidepress_plugin_dir === '') {
        $oxidepress_plugin_dir = rtrim(oxidepress_normalize_path(WP_CONTENT_DIR), '/') . '/plugins';
    }
    define('WP_PLUGIN_DIR', $oxidepress_plugin_dir);
}
if (!defined('WP_CONTENT_URL')) {
    define('WP_CONTENT_URL', site_url('wp-content'));
}
if (!defined('WP_PLUGIN_URL')) {
    define('WP_PLUGIN_URL', site_url('wp-content/plugins'));
}

register_shutdown_function('oxidepress_shutdown');

while (true) {
    try {
        $frame = oxidepress_read_frame();
    } catch (Throwable $read_error) {
        oxidepress_write_frame(array(
            'id' => $oxidepress_request_id,
            'ok' => false,
            'error' => $read_error->getMessage(),
        ));
        continue;
    }
    if ($frame === null) {
        exit(0);
    }

    $oxidepress_request_id = isset($frame['id']) ? $frame['id'] : 0;
    $op = isset($frame['op']) ? (string)$frame['op'] : '';

    try {
        if ($op === 'load') {
            $oxidepress_batch = array();
            $path = isset($frame['path']) ? (string)$frame['path'] : '';
            $oxidepress_plugin_file = $path;
            if ($path === '' || !is_file($path)) {
                throw new RuntimeException('plugin file not found: ' . $path);
            }
            ob_start();
            try {
                include $path;
                if (ob_get_level() > 0) {
                    ob_end_clean();
                }
            } catch (Throwable $load_error) {
                if (ob_get_level() > 0) {
                    ob_end_clean();
                }
                throw $load_error;
            }
            if (!$oxidepress_asset_hooks) {
                oxidepress_register('action', 'wp_head', 'oxidepress_print_head_assets', 8, 1);
                oxidepress_register('action', 'wp_footer', 'oxidepress_print_footer_assets', 20, 1);
                $oxidepress_asset_hooks = true;
            }
            oxidepress_write_frame(array(
                'id' => $oxidepress_request_id,
                'ok' => true,
                'registrations' => array_values($oxidepress_batch),
            ));
        } elseif ($op === 'invoke') {
            oxidepress_invoke($frame);
        } elseif ($op === 'admin_menus') {
            $pages = oxidepress_admin_page_list();
            oxidepress_write_frame(array(
                'id' => $oxidepress_request_id,
                'ok' => true,
                'pages' => $pages,
            ));
        } elseif ($op === 'admin_page') {
            $slug = isset($frame['slug']) ? (string)$frame['slug'] : '';
            $nonce = isset($frame['nonce']) ? (string)$frame['nonce'] : '';
            $rendered = oxidepress_render_admin_page($slug, $nonce);
            $rendered['id'] = $oxidepress_request_id;
            oxidepress_write_frame($rendered);
        } elseif ($op === 'post_types') {
            oxidepress_write_frame(array(
                'id' => $oxidepress_request_id,
                'ok' => true,
                'types' => oxidepress_list_post_types(),
            ));
        } elseif ($op === 'taxonomies') {
            oxidepress_write_frame(array(
                'id' => $oxidepress_request_id,
                'ok' => true,
                'taxonomies' => oxidepress_list_taxonomies(),
            ));
        } elseif ($op === 'admin_save') {
            $group = isset($frame['group']) ? (string)$frame['group'] : '';
            $values = isset($frame['values']) && is_array($frame['values']) ? $frame['values'] : array();
            $saved = oxidepress_save_settings($group, $values);
            oxidepress_write_frame(array(
                'id' => $oxidepress_request_id,
                'ok' => true,
                'saved' => $saved,
            ));
        } elseif ($op === 'shutdown') {
            exit(0);
        } else {
            oxidepress_write_frame(array(
                'id' => $oxidepress_request_id,
                'ok' => false,
                'error' => 'unknown op',
            ));
        }
    } catch (Throwable $error) {
        oxidepress_write_frame(array(
            'id' => $oxidepress_request_id,
            'ok' => false,
            'error' => $error->getMessage(),
        ));
    }
}

function oxidepress_invoke($frame)
{
    global $oxidepress_callbacks, $oxidepress_disabled, $oxidepress_request_id;

    $id = $oxidepress_request_id;
    $kind = isset($frame['kind']) ? (string)$frame['kind'] : 'action';
    $cb_id = isset($frame['callback_id']) ? (int)$frame['callback_id'] : -1;

    if (!array_key_exists($cb_id, $oxidepress_callbacks)) {
        oxidepress_write_frame(array(
            'id' => $id,
            'ok' => false,
            'error' => 'unknown callback ' . $cb_id,
        ));
        return;
    }

    if (!empty($oxidepress_disabled[$cb_id])) {
        if ($kind === 'filter') {
            oxidepress_write_frame(array(
                'id' => $id,
                'ok' => true,
                'value' => array_key_exists('value', $frame) ? $frame['value'] : null,
                'output' => '',
            ));
        } elseif ($kind === 'shortcode') {
            oxidepress_write_frame(array(
                'id' => $id,
                'ok' => true,
                'value' => '',
                'output' => '',
            ));
        } else {
            oxidepress_write_frame(array(
                'id' => $id,
                'ok' => true,
                'output' => '',
            ));
        }
        return;
    }

    $callback = $oxidepress_callbacks[$cb_id];
    ob_start();
    try {
        if ($kind === 'filter') {
            $value = array_key_exists('value', $frame) ? $frame['value'] : null;
            $result = call_user_func($callback, $value);
            $output = oxidepress_capture_output();
            oxidepress_write_frame(array(
                'id' => $id,
                'ok' => true,
                'value' => $result,
                'output' => $output,
            ));
        } elseif ($kind === 'shortcode') {
            $atts = isset($frame['atts']) && is_array($frame['atts']) ? $frame['atts'] : array();
            $content = isset($frame['content']) ? (string)$frame['content'] : '';
            $tag = isset($frame['tag']) ? (string)$frame['tag'] : '';
            $result = call_user_func($callback, $atts, $content, $tag);
            $output = oxidepress_capture_output();
            oxidepress_write_frame(array(
                'id' => $id,
                'ok' => true,
                'value' => $result,
                'output' => $output,
            ));
        } else {
            $args = array_key_exists('args', $frame) ? $frame['args'] : null;
            call_user_func($callback, $args);
            $output = oxidepress_capture_output();
            oxidepress_write_frame(array(
                'id' => $id,
                'ok' => true,
                'output' => $output,
            ));
        }
    } catch (Throwable $error) {
        oxidepress_capture_output();
        oxidepress_write_frame(array(
            'id' => $id,
            'ok' => false,
            'error' => $error->getMessage(),
        ));
    }
}

function oxidepress_capture_output()
{
    if (ob_get_level() < 1) {
        return '';
    }
    $output = ob_get_clean();
    return $output === false ? '' : $output;
}

function oxidepress_register($kind, $tag, $callback, $priority, $accepted_args)
{
    global $oxidepress_callbacks, $oxidepress_batch, $oxidepress_hooks, $oxidepress_shortcodes;

    $id = count($oxidepress_callbacks);
    $oxidepress_callbacks[] = $callback;
    $priority = (int)$priority;
    $tag = (string)$tag;
    $oxidepress_batch[] = array(
        'kind' => $kind,
        'tag' => $tag,
        'callback_id' => $id,
        'priority' => $priority,
    );
    if ($kind === 'action' || $kind === 'filter') {
        if (!isset($oxidepress_hooks[$kind][$tag])) {
            $oxidepress_hooks[$kind][$tag] = array();
        }
        $oxidepress_hooks[$kind][$tag][] = array(
            'id' => $id,
            'priority' => $priority,
            'accepted_args' => (int)$accepted_args,
        );
    } elseif ($kind === 'shortcode') {
        $oxidepress_shortcodes[$tag] = $id;
    }
    return $id;
}

function oxidepress_remove($kind, $tag, $callback, $priority)
{
    global $oxidepress_callbacks, $oxidepress_hooks, $oxidepress_disabled, $oxidepress_batch;

    $tag = (string)$tag;
    $priority = (int)$priority;
    if (!isset($oxidepress_hooks[$kind][$tag])) {
        return false;
    }

    $kept = array();
    $removed_id = null;
    foreach ($oxidepress_hooks[$kind][$tag] as $entry) {
        $existing = $oxidepress_callbacks[$entry['id']];
        if ($removed_id === null
            && (int)$entry['priority'] === $priority
            && oxidepress_same_callback($existing, $callback)
        ) {
            $removed_id = $entry['id'];
            $oxidepress_disabled[$entry['id']] = true;
            continue;
        }
        $kept[] = $entry;
    }
    $oxidepress_hooks[$kind][$tag] = $kept;
    if ($removed_id === null) {
        return false;
    }

    $batch = array();
    foreach ($oxidepress_batch as $reg) {
        if ((int)$reg['callback_id'] !== (int)$removed_id) {
            $batch[] = $reg;
        }
    }
    $oxidepress_batch = $batch;
    return true;
}

function oxidepress_same_callback($left, $right)
{
    if (is_string($left) && is_string($right)) {
        return strcasecmp($left, $right) === 0;
    }
    return $left === $right;
}

function oxidepress_by_priority($left, $right)
{
    if ($left['priority'] == $right['priority']) {
        return 0;
    }
    return ($left['priority'] < $right['priority']) ? -1 : 1;
}

function add_action($tag, $callback, $priority = 10, $accepted_args = 1)
{
    oxidepress_register('action', $tag, $callback, $priority, $accepted_args);
    return true;
}

function add_filter($tag, $callback, $priority = 10, $accepted_args = 1)
{
    oxidepress_register('filter', $tag, $callback, $priority, $accepted_args);
    return true;
}

function remove_action($tag, $callback, $priority = 10)
{
    return oxidepress_remove('action', $tag, $callback, $priority);
}

function remove_filter($tag, $callback, $priority = 10)
{
    return oxidepress_remove('filter', $tag, $callback, $priority);
}

function do_action($tag, ...$args)
{
    global $oxidepress_hooks, $oxidepress_callbacks, $oxidepress_disabled, $oxidepress_did;

    $tag = (string)$tag;
    if (!isset($oxidepress_did[$tag])) {
        $oxidepress_did[$tag] = 0;
    }
    $oxidepress_did[$tag]++;
    $entries = isset($oxidepress_hooks['action'][$tag]) ? $oxidepress_hooks['action'][$tag] : array();
    usort($entries, 'oxidepress_by_priority');
    foreach ($entries as $entry) {
        if (!empty($oxidepress_disabled[$entry['id']])) {
            continue;
        }
        $accepted = isset($entry['accepted_args']) ? (int)$entry['accepted_args'] : count($args);
        call_user_func_array($oxidepress_callbacks[$entry['id']], array_slice($args, 0, $accepted));
    }
}

function apply_filters($tag, $value, ...$extra)
{
    global $oxidepress_hooks, $oxidepress_callbacks, $oxidepress_disabled;

    $tag = (string)$tag;
    $entries = isset($oxidepress_hooks['filter'][$tag]) ? $oxidepress_hooks['filter'][$tag] : array();
    usort($entries, 'oxidepress_by_priority');
    foreach ($entries as $entry) {
        if (!empty($oxidepress_disabled[$entry['id']])) {
            continue;
        }
        $args = array_merge(array($value), $extra);
        $accepted = isset($entry['accepted_args']) ? (int)$entry['accepted_args'] : count($args);
        $callback_args = array_slice($args, 0, $accepted);
        if (count($callback_args) === 0) {
            $callback_args = array($value);
        }
        $value = call_user_func_array($oxidepress_callbacks[$entry['id']], $callback_args);
    }
    return $value;
}

function add_shortcode($tag, $callback)
{
    oxidepress_register('shortcode', $tag, $callback, 10, 1);
    return true;
}

function remove_shortcode($tag)
{
    global $oxidepress_shortcodes, $oxidepress_disabled, $oxidepress_batch;

    $tag = (string)$tag;
    if (isset($oxidepress_shortcodes[$tag])) {
        $oxidepress_disabled[$oxidepress_shortcodes[$tag]] = true;
        unset($oxidepress_shortcodes[$tag]);
    }
    $kept = array();
    foreach ($oxidepress_batch as $reg) {
        if (isset($reg['kind'], $reg['tag']) && $reg['kind'] === 'shortcode' && $reg['tag'] === $tag) {
            if (isset($reg['callback_id'])) {
                $oxidepress_disabled[$reg['callback_id']] = true;
            }
            continue;
        }
        $kept[] = $reg;
    }
    $oxidepress_batch = $kept;
}

function shortcode_atts($pairs, $atts, $shortcode = '')
{
    if (!is_array($atts)) {
        $atts = array();
    }
    $out = array();
    foreach ((array)$pairs as $name => $default) {
        if (array_key_exists($name, $atts)) {
            $out[$name] = $atts[$name];
        } else {
            $out[$name] = $default;
        }
    }
    return $out;
}

function register_activation_hook($file, $callback)
{
    oxidepress_register('activation', (string)$file, $callback, 10, 1);
}

function register_deactivation_hook($file, $callback)
{
    oxidepress_register('deactivation', (string)$file, $callback, 10, 1);
}

function get_option($option, $default = false)
{
    $value = oxidepress_call('get_option', array('name' => (string)$option));
    if ($value === null) {
        return $default;
    }
    return $value;
}

function update_option($option, $value, $autoload = null)
{
    return (bool)oxidepress_call('update_option', array(
        'name' => (string)$option,
        'value' => oxidepress_option_string($value),
    ));
}

function add_option($option, $value = '', $deprecated = '', $autoload = 'yes')
{
    $existing = oxidepress_call('get_option', array('name' => (string)$option));
    if ($existing !== null) {
        return false;
    }
    return update_option($option, $value, $autoload);
}

function delete_option($option)
{
    return (bool)oxidepress_call('delete_option', array('name' => (string)$option));
}

function get_site_option($option, $default = false, $deprecated = true)
{
    return get_option($option, $default);
}

function get_transient($transient)
{
    return get_option('_transient_' . $transient, false);
}

function set_transient($transient, $value, $expiration = 0)
{
    return update_option('_transient_' . $transient, $value);
}

function delete_transient($transient)
{
    return delete_option('_transient_' . $transient);
}

function oxidepress_option_string($value)
{
    if (is_bool($value)) {
        return $value ? '1' : '0';
    }
    if ($value === null) {
        return '';
    }
    if (is_array($value) || is_object($value)) {
        $encoded = json_encode($value);
        return $encoded === false ? '' : $encoded;
    }
    return (string)$value;
}

function esc_html($text)
{
    return htmlspecialchars(oxidepress_text($text), ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
}

function esc_attr($text)
{
    return esc_html($text);
}

function __($text, $domain = 'default')
{
    return oxidepress_text($text);
}

function _e($text, $domain = 'default')
{
    echo __($text, $domain);
}

function wp_die($message = '', $title = '', $args = array())
{
    throw new RuntimeException('wp_die: ' . oxidepress_text($message));
}

function plugin_dir_path($file)
{
    return rtrim(str_replace('\\', '/', dirname((string)$file)), '/') . '/';
}

function plugin_basename($file)
{
    $file = oxidepress_normalize_path($file);
    $plugins = rtrim(oxidepress_normalize_path(oxidepress_env('OXIDEPRESS_PLUGINS_DIR', '')), '/');
    $prefix = $plugins . '/';
    if ($plugins !== '' && strncmp($file, $prefix, strlen($prefix)) === 0) {
        return substr($file, strlen($prefix));
    }
    $dir = basename(dirname($file));
    $base = basename($file);
    if ($dir === '' || $dir === '.' || $dir === '/' || $dir === '\\') {
        return $base;
    }
    return $dir . '/' . $base;
}

function plugin_dir_url($file)
{
    $site = rtrim(oxidepress_env('OXIDEPRESS_SITE_URL', 'http://localhost'), '/');
    $dir = dirname(plugin_basename($file));
    $dir = oxidepress_normalize_path($dir);
    if ($dir === '.' || $dir === '') {
        $suffix = '';
    } else {
        $suffix = trim($dir, '/') . '/';
    }
    return $site . '/wp-content/plugins/' . $suffix;
}

function trailingslashit($value)
{
    return rtrim(oxidepress_normalize_path($value), '/') . '/';
}

function untrailingslashit($value)
{
    return rtrim(oxidepress_normalize_path($value), '/');
}

function path_join($base, $path)
{
    $path = (string)$path;
    if ($path !== '' && ($path[0] === '/' || $path[0] === '\\' || preg_match('/^[A-Za-z]:[\\\\\\/]/', $path))) {
        return oxidepress_normalize_path($path);
    }
    return rtrim(oxidepress_normalize_path($base), '/') . '/' . ltrim(str_replace('\\', '/', $path), '/');
}

function plugins_url($path = '', $plugin = '')
{
    if ($plugin === '' || $plugin === null) {
        $base = rtrim(oxidepress_env('OXIDEPRESS_SITE_URL', 'http://localhost'), '/') . '/wp-content/plugins';
    } else {
        $base = untrailingslashit(plugin_dir_url($plugin));
    }
    $path = ltrim(str_replace('\\', '/', (string)$path), '/');
    if ($path === '') {
        return $base;
    }
    return $base . '/' . $path;
}

function site_url($path = '', $scheme = null)
{
    $base = rtrim(oxidepress_env('OXIDEPRESS_SITE_URL', 'http://localhost'), '/');
    $path = ltrim(str_replace('\\', '/', (string)$path), '/');
    if ($path === '') {
        return $base;
    }
    return $base . '/' . $path;
}

function home_url($path = '', $scheme = null)
{
    return site_url($path, $scheme);
}

function admin_url($path = '', $scheme = 'admin')
{
    $path = ltrim(str_replace('\\', '/', (string)$path), '/');
    if ($path === '') {
        return site_url('wp-admin/');
    }
    return site_url('wp-admin/' . $path);
}

function is_admin()
{
    global $oxidepress_admin_depth;
    if ((int)$oxidepress_admin_depth > 0) {
        return true;
    }
    return (bool)oxidepress_call('is_admin', array());
}

function is_multisite()
{
    return false;
}

function wptexturize($text, $reset = false)
{
    return oxidepress_text($text);
}

function get_locale()
{
    return 'en_US';
}

function get_user_locale($user_id = 0)
{
    return get_locale();
}

function __return_true()
{
    return true;
}

function __return_false()
{
    return false;
}

function __return_null()
{
    return null;
}

function __return_empty_string()
{
    return '';
}

function __return_empty_array()
{
    return array();
}

function __return_zero()
{
    return 0;
}

function _x($text, $context, $domain = 'default')
{
    return oxidepress_text($text);
}

function _n($single, $plural, $number, $domain = 'default')
{
    return ((int)$number === 1) ? oxidepress_text($single) : oxidepress_text($plural);
}

function _nx($single, $plural, $number, $context, $domain = 'default')
{
    return _n($single, $plural, $number, $domain);
}

function esc_html_x($text, $context, $domain = 'default')
{
    return esc_html($text);
}

function esc_attr_x($text, $context, $domain = 'default')
{
    return esc_attr($text);
}

function add_query_arg($key, $value = false, $url = null)
{
    $args = func_get_args();
    if (isset($args[0]) && is_array($args[0])) {
        $pairs = $args[0];
        $target = isset($args[1]) ? (string)$args[1] : '';
    } else {
        $pairs = array((string)$key => $value);
        $target = isset($args[2]) ? (string)$args[2] : '';
    }
    $fragment = '';
    $hash = strpos($target, '#');
    if ($hash !== false) {
        $fragment = substr($target, $hash);
        $target = substr($target, 0, $hash);
    }
    $parts = explode('?', $target, 2);
    $base = $parts[0];
    $query = array();
    if (isset($parts[1]) && $parts[1] !== '') {
        parse_str($parts[1], $query);
    }
    foreach ($pairs as $name => $item) {
        if ($item === false || $item === null) {
            unset($query[$name]);
        } else {
            $query[$name] = $item;
        }
    }
    $built = http_build_query($query);
    return $base . ($built === '' ? '' : '?' . $built) . $fragment;
}

function remove_query_arg($key, $query = false)
{
    if (!is_array($key)) {
        $key = array($key);
    }
    $pairs = array();
    foreach ($key as $name) {
        $pairs[$name] = false;
    }
    return add_query_arg($pairs, $query === false ? '' : (string)$query);
}

function esc_html__($text, $domain = 'default')
{
    return esc_html(__($text, $domain));
}

function esc_attr__($text, $domain = 'default')
{
    return esc_attr(__($text, $domain));
}

function esc_html_e($text, $domain = 'default')
{
    echo esc_html__($text, $domain);
}

function esc_attr_e($text, $domain = 'default')
{
    echo esc_attr__($text, $domain);
}

function esc_url($url, $protocols = null, $_context = 'display')
{
    $url = esc_url_raw($url, $protocols);
    if ($url === '') {
        return '';
    }
    return htmlspecialchars($url, ENT_QUOTES | ENT_SUBSTITUTE, 'UTF-8');
}

function esc_url_raw($url, $protocols = null)
{
    $url = trim(oxidepress_text($url));
    if ($url === '') {
        return '';
    }
    if (preg_match('/^(javascript|data):/i', $url)) {
        return '';
    }
    return $url;
}

function sanitize_url($url, $protocols = null)
{
    return esc_url_raw($url, $protocols);
}

function sanitize_key($key)
{
    $key = strtolower(oxidepress_text($key));
    $key = preg_replace('/[^a-z0-9_\-]/', '', $key);
    return $key === null ? '' : $key;
}

function absint($value)
{
    return abs((int)$value);
}

function wp_json_encode($data, $options = 0, $depth = 512)
{
    $encoded = json_encode($data, $options, $depth);
    return $encoded === false ? false : $encoded;
}

function wp_create_nonce($action = -1)
{
    return substr(md5('oxidepress|' . oxidepress_text($action)), 0, 10);
}

function wp_verify_nonce($nonce, $action = -1)
{
    return hash_equals(wp_create_nonce($action), (string)$nonce) ? 1 : false;
}

function nocache_headers()
{
}

function current_user_can($capability, ...$args)
{
    global $oxidepress_admin_depth;
    $capability = (string)$capability;
    if ((int)$oxidepress_admin_depth > 0 && $capability === 'manage_options') {
        return true;
    }
    return (bool)oxidepress_call('current_user_can', array(
        'capability' => $capability,
    ));
}

function wp_localize_script($handle, $object_name, $data)
{
    return true;
}

function wp_upload_dir($time = null, $create_dir = true, $refresh = false)
{
    $baseurl = site_url('wp-content/uploads');
    $basedir = rtrim(oxidepress_normalize_path(WP_CONTENT_DIR), '/') . '/uploads';
    return array(
        'path' => $basedir,
        'url' => $baseurl,
        'subdir' => '',
        'basedir' => $basedir,
        'baseurl' => $baseurl,
        'error' => false,
    );
}

function wp_get_upload_dir()
{
    return wp_upload_dir();
}

function wp_mkdir_p($target)
{
    $target = oxidepress_normalize_path($target);
    if ($target === '' || $target === '.' || $target === '/') {
        return false;
    }
    if (is_dir($target)) {
        return true;
    }
    return mkdir($target, 0777, true);
}

function wp_is_writable($path)
{
    $path = oxidepress_normalize_path($path);
    if ($path === '') {
        return false;
    }
    if (file_exists($path)) {
        return is_writable($path);
    }
    $parent = dirname($path);
    return $parent !== '' && is_dir($parent) && is_writable($parent);
}

function wp_normalize_path($path)
{
    return oxidepress_normalize_path($path);
}

function get_bloginfo($show = '', $filter = 'raw')
{
    switch ((string)$show) {
        case 'version':
            return isset($GLOBALS['wp_version']) ? (string)$GLOBALS['wp_version'] : '';
        case 'url':
        case 'wpurl':
        case 'home':
            return site_url();
        case 'name':
            $name = get_option('blogname', 'OxidePress');
            return $name === false ? 'OxidePress' : (string)$name;
        case 'description':
            $description = get_option('blogdescription', '');
            return $description === false ? '' : (string)$description;
        case 'charset':
            return 'UTF-8';
        case 'language':
            return get_locale();
        default:
            return '';
    }
}

function register_post_type($post_type, $args = array())
{
    global $oxidepress_post_types;

    $post_type = sanitize_key($post_type);
    if ($post_type === '') {
        return new WP_Error('invalid_post_type', 'post type is empty');
    }
    if (!is_array($args)) {
        $args = array();
    }
    $object = new WP_Post_Type($post_type, $args);
    $oxidepress_post_types[$post_type] = $object;
    return $object;
}

function oxidepress_list_post_types()
{
    global $oxidepress_post_types;

    if (!did_action('init')) {
        do_action('init');
    }
    $list = array();
    foreach ($oxidepress_post_types as $name => $object) {
        $name = (string)$name;
        if (!oxidepress_token($name, 20) || oxidepress_type_is_internal($name)) {
            continue;
        }
        $args = isset($object->args) && is_array($object->args) ? $object->args : array();
        $public = !empty($object->public);
        $show_ui = array_key_exists('show_ui', $args) ? !empty($args['show_ui']) : $public;
        $show_in_menu = array_key_exists('show_in_menu', $args) ? !empty($args['show_in_menu']) : $show_ui;
        $label = isset($args['label']) ? oxidepress_text($args['label']) : '';
        if ($label === '' && isset($object->label)) {
            $label = (string)$object->label;
        }
        if ($label === '') {
            $label = $name;
        }
        $singular = $label;
        if (isset($object->labels->singular_name) && (string)$object->labels->singular_name !== '') {
            $singular = (string)$object->labels->singular_name;
        }
        $list[] = array(
            'name' => $name,
            'label' => $label,
            'singular' => $singular,
            'public' => $public,
            'show_ui' => $show_ui,
            'show_in_menu' => $show_in_menu,
        );
        if (count($list) >= 40) {
            break;
        }
    }
    return $list;
}

function oxidepress_type_is_internal($name)
{
    return in_array($name, array(
        'revision',
        'nav_menu_item',
        'custom_css',
        'customize_changeset',
        'oembed_cache',
        'user_request',
        'wp_block',
        'wp_template',
        'wp_template_part',
        'wp_global_styles',
        'wp_navigation',
        'wp_font_family',
        'wp_font_face',
    ), true);
}

function oxidepress_taxonomy_is_reserved($name)
{
    return in_array($name, array(
        'category',
        'post_tag',
        'nav_menu',
        'link_category',
        'post_format',
    ), true);
}

class WP_Taxonomy
{
    public $name = '';
    public $object_type = array();
    public $label = '';
    public $labels;
    public $public = true;
    public $hierarchical = false;
    public $args = array();

    public function __construct($taxonomy, $object_type, $args)
    {
        $this->name = $taxonomy;
        $this->object_type = array_values($object_type);
        $this->args = is_array($args) ? $args : array();
        $labels = isset($this->args['labels']) && is_array($this->args['labels']) ? $this->args['labels'] : array();
        $this->labels = (object)$labels;
        $this->label = isset($this->args['label']) ? oxidepress_text($this->args['label']) : '';
        if ($this->label === '' && isset($labels['name'])) {
            $this->label = oxidepress_text($labels['name']);
        }
        if ($this->label === '') {
            $this->label = $taxonomy;
        }
        if (!isset($this->labels->singular_name) || (string)$this->labels->singular_name === '') {
            $this->labels->singular_name = $this->label;
        }
        $this->public = array_key_exists('public', $this->args) ? !empty($this->args['public']) : true;
        $this->hierarchical = !empty($this->args['hierarchical']);
    }
}

function register_taxonomy($taxonomy, $object_type, $args = array())
{
    global $oxidepress_taxonomies;

    $taxonomy = sanitize_key($taxonomy);
    if ($taxonomy === '' || !oxidepress_token($taxonomy, 32) || oxidepress_taxonomy_is_reserved($taxonomy)) {
        return new WP_Error('invalid_taxonomy', 'taxonomy is not allowed');
    }
    if (!is_array($object_type)) {
        $object_type = array($object_type);
    }
    $types = array();
    foreach ($object_type as $type) {
        $type = sanitize_key($type);
        if ($type !== '' && oxidepress_token($type, 20) && !oxidepress_type_is_internal($type)) {
            $types[] = $type;
        }
        if (count($types) >= 8) {
            break;
        }
    }
    if (count($types) === 0) {
        return new WP_Error('invalid_taxonomy', 'taxonomy has no object type');
    }
    if (!is_array($args)) {
        $args = array();
    }
    $object = new WP_Taxonomy($taxonomy, $types, $args);
    $oxidepress_taxonomies[$taxonomy] = $object;
    return $object;
}

function taxonomy_exists($taxonomy)
{
    global $oxidepress_taxonomies;

    $taxonomy = sanitize_key($taxonomy);
    if (oxidepress_taxonomy_is_reserved($taxonomy)) {
        return true;
    }
    return isset($oxidepress_taxonomies[$taxonomy]);
}

function oxidepress_list_taxonomies()
{
    global $oxidepress_taxonomies;

    if (!did_action('init')) {
        do_action('init');
    }
    $list = array();
    foreach ($oxidepress_taxonomies as $name => $object) {
        $name = (string)$name;
        if (!oxidepress_token($name, 32) || oxidepress_taxonomy_is_reserved($name)) {
            continue;
        }
        $args = isset($object->args) && is_array($object->args) ? $object->args : array();
        $public = !empty($object->public);
        $show_ui = array_key_exists('show_ui', $args) ? !empty($args['show_ui']) : $public;
        $show_in_menu = array_key_exists('show_in_menu', $args) ? !empty($args['show_in_menu']) : $show_ui;
        $types = array();
        $object_type = isset($object->object_type) && is_array($object->object_type) ? $object->object_type : array();
        foreach ($object_type as $type) {
            $type = (string)$type;
            if (oxidepress_token($type, 20) && !oxidepress_type_is_internal($type)) {
                $types[] = $type;
            }
        }
        if (count($types) === 0) {
            continue;
        }
        $label = isset($object->label) ? oxidepress_text($object->label) : $name;
        if ($label === '') {
            $label = $name;
        }
        $singular = $label;
        if (isset($object->labels->singular_name) && (string)$object->labels->singular_name !== '') {
            $singular = oxidepress_text($object->labels->singular_name);
        }
        $list[] = array(
            'name' => $name,
            'label' => $label,
            'singular' => $singular,
            'object_types' => $types,
            'public' => $public,
            'hierarchical' => !empty($object->hierarchical),
            'show_ui' => $show_ui,
            'show_in_menu' => $show_in_menu,
        );
        if (count($list) >= 40) {
            break;
        }
    }
    return $list;
}

if (!defined('OBJECT')) {
    define('OBJECT', 'OBJECT');
}
if (!defined('ARRAY_A')) {
    define('ARRAY_A', 'ARRAY_A');
}
if (!defined('ARRAY_N')) {
    define('ARRAY_N', 'ARRAY_N');
}

class WP_Post
{
    public $ID = 0;
    public $post_author = 0;
    public $post_date = '';
    public $post_content = '';
    public $post_title = '';
    public $post_excerpt = '';
    public $post_status = '';
    public $post_name = '';
    public $post_type = 'post';
    public $post_parent = 0;
    public $guid = '';
    public $menu_order = 0;
    public $comment_count = 0;
}

function oxidepress_post_from_row($row)
{
    if (!is_array($row) || !isset($row['id'])) {
        return null;
    }
    $post = new WP_Post();
    $post->ID = (int)$row['id'];
    $post->post_author = isset($row['author']) ? (int)$row['author'] : 0;
    $post->post_date = isset($row['date']) ? (string)$row['date'] : '';
    $post->post_content = isset($row['content']) ? (string)$row['content'] : '';
    $post->post_title = isset($row['title']) ? (string)$row['title'] : '';
    $post->post_excerpt = isset($row['excerpt']) ? (string)$row['excerpt'] : '';
    $post->post_status = isset($row['status']) ? (string)$row['status'] : '';
    $post->post_name = isset($row['name']) ? (string)$row['name'] : '';
    $post->post_type = isset($row['type']) ? (string)$row['type'] : 'post';
    $post->post_parent = isset($row['parent']) ? (int)$row['parent'] : 0;
    $post->guid = isset($row['guid']) ? (string)$row['guid'] : '';
    $post->menu_order = isset($row['menu_order']) ? (int)$row['menu_order'] : 0;
    $post->comment_count = isset($row['comment_count']) ? (int)$row['comment_count'] : 0;
    return $post;
}

function get_post($post = null, $output = 'OBJECT', $filter = 'raw')
{
    if ($post instanceof WP_Post) {
        return $post;
    }
    $id = (int)$post;
    if ($id <= 0) {
        return null;
    }
    $row = oxidepress_call('get_post', array('post_id' => $id));
    return oxidepress_post_from_row($row);
}

function wp_insert_post($postarr, $wp_error = false, $fire_after_hooks = true)
{
    if (!is_array($postarr)) {
        return $wp_error ? new WP_Error('invalid_post', 'post is not an array') : 0;
    }
    if (!empty($postarr['ID'])) {
        return wp_update_post($postarr, $wp_error);
    }
    $type = isset($postarr['post_type']) ? sanitize_key($postarr['post_type']) : 'post';
    if ($type === '') {
        $type = 'post';
    }
    $status = isset($postarr['post_status']) ? sanitize_key($postarr['post_status']) : 'draft';
    if ($status === '') {
        $status = 'draft';
    }
    $result = oxidepress_call('insert_post', array('post' => array(
        'title' => isset($postarr['post_title']) ? (string)$postarr['post_title'] : '',
        'content' => isset($postarr['post_content']) ? (string)$postarr['post_content'] : '',
        'excerpt' => isset($postarr['post_excerpt']) ? (string)$postarr['post_excerpt'] : '',
        'status' => $status,
        'name' => isset($postarr['post_name']) ? (string)$postarr['post_name'] : '',
        'type' => $type,
        'parent' => isset($postarr['post_parent']) ? (int)$postarr['post_parent'] : 0,
    )));
    $id = is_numeric($result) ? (int)$result : 0;
    if ($id <= 0) {
        return $wp_error ? new WP_Error('db_insert_error', 'post was not stored') : 0;
    }
    return $id;
}

function wp_update_post($postarr, $wp_error = false, $fire_after_hooks = true)
{
    if (!is_array($postarr)) {
        return $wp_error ? new WP_Error('invalid_post', 'post is not an array') : 0;
    }
    $id = isset($postarr['ID']) ? (int)$postarr['ID'] : 0;
    $current = get_post($id);
    if (!$current) {
        return $wp_error ? new WP_Error('invalid_post', 'post was not found') : 0;
    }
    $ok = oxidepress_call('update_post', array(
        'post_id' => $id,
        'post' => array(
            'title' => array_key_exists('post_title', $postarr) ? (string)$postarr['post_title'] : $current->post_title,
            'content' => array_key_exists('post_content', $postarr) ? (string)$postarr['post_content'] : $current->post_content,
            'excerpt' => array_key_exists('post_excerpt', $postarr) ? (string)$postarr['post_excerpt'] : $current->post_excerpt,
            'status' => array_key_exists('post_status', $postarr) ? sanitize_key($postarr['post_status']) : $current->post_status,
            'name' => array_key_exists('post_name', $postarr) ? (string)$postarr['post_name'] : $current->post_name,
            'type' => $current->post_type,
            'parent' => array_key_exists('post_parent', $postarr) ? (int)$postarr['post_parent'] : (int)$current->post_parent,
        ),
    ));
    if (!$ok) {
        return $wp_error ? new WP_Error('db_update_error', 'post was not updated') : 0;
    }
    return $id;
}

function wp_delete_post($postid, $force_delete = false)
{
    $id = (int)$postid;
    if ($id <= 0) {
        return false;
    }
    $ok = oxidepress_call('delete_post', array(
        'post_id' => $id,
        'force' => (bool)$force_delete,
    ));
    return $ok ? get_post($id) || true : false;
}

function get_posts($args = null)
{
    if (!is_array($args)) {
        $args = array();
    }
    $limit = 5;
    if (isset($args['numberposts'])) {
        $limit = (int)$args['numberposts'];
    } elseif (isset($args['posts_per_page'])) {
        $limit = (int)$args['posts_per_page'];
    }
    if ($limit === -1) {
        $limit = 20;
    }
    $rows = oxidepress_call('query_posts', array('query' => array(
        'type' => isset($args['post_type']) ? sanitize_key($args['post_type']) : 'post',
        'status' => isset($args['post_status']) ? sanitize_key($args['post_status']) : 'publish',
        'name' => isset($args['name']) ? (string)$args['name'] : '',
        'search' => isset($args['s']) ? (string)$args['s'] : '',
        'limit' => $limit,
    )));
    if (!is_array($rows)) {
        return array();
    }
    $posts = array();
    foreach ($rows as $row) {
        $post = oxidepress_post_from_row($row);
        if ($post) {
            $posts[] = $post;
        }
    }
    return $posts;
}

function get_permalink($post = 0, $leavename = false)
{
    $post = get_post($post);
    if (!$post || $post->post_name === '') {
        return false;
    }
    return rtrim(site_url(), '/') . '/' . rawurlencode($post->post_name) . '/';
}

function oxidepress_meta_text($value)
{
    if (is_array($value) || is_object($value)) {
        return null;
    }
    if ($value === null) {
        return '';
    }
    return (string)$value;
}

function get_post_meta($post_id, $key = '', $single = false)
{
    $id = (int)$post_id;
    $key = (string)$key;
    if ($id <= 0) {
        return $single ? '' : array();
    }
    $rows = oxidepress_call('get_post_meta', array(
        'post_id' => $id,
        'meta_key' => $key,
    ));
    if (!is_array($rows)) {
        return $single ? '' : array();
    }
    if ($key === '') {
        $grouped = array();
        foreach ($rows as $row) {
            if (!is_array($row)) {
                continue;
            }
            $name = isset($row['key']) ? (string)$row['key'] : '';
            $grouped[$name][] = isset($row['value']) ? (string)$row['value'] : '';
        }
        return $grouped;
    }
    $values = array();
    foreach ($rows as $row) {
        if (is_array($row)) {
            $values[] = isset($row['value']) ? (string)$row['value'] : '';
        }
    }
    if ($single) {
        return count($values) ? $values[0] : '';
    }
    return $values;
}

function update_post_meta($post_id, $meta_key, $meta_value, $prev_value = '')
{
    $id = (int)$post_id;
    $key = (string)$meta_key;
    $value = oxidepress_meta_text($meta_value);
    $prev = oxidepress_meta_text($prev_value);
    if ($id <= 0 || $key === '' || $value === null || $prev === null) {
        return false;
    }
    return (bool)oxidepress_call('update_post_meta', array(
        'post_id' => $id,
        'meta_key' => $key,
        'value' => $value,
        'prev' => $prev,
    ));
}

function add_post_meta($post_id, $meta_key, $meta_value, $unique = false)
{
    $id = (int)$post_id;
    $key = (string)$meta_key;
    $value = oxidepress_meta_text($meta_value);
    if ($id <= 0 || $key === '' || $value === null) {
        return false;
    }
    return (bool)oxidepress_call('add_post_meta', array(
        'post_id' => $id,
        'meta_key' => $key,
        'value' => $value,
        'unique' => (bool)$unique,
    ));
}

function delete_post_meta($post_id, $meta_key, $meta_value = '')
{
    $id = (int)$post_id;
    $key = (string)$meta_key;
    $prev = oxidepress_meta_text($meta_value);
    if ($id <= 0 || $key === '' || $prev === null) {
        return false;
    }
    return (bool)oxidepress_call('delete_post_meta', array(
        'post_id' => $id,
        'meta_key' => $key,
        'prev' => $prev,
    ));
}

class WP_Term
{
    public $term_id = 0;
    public $term_taxonomy_id = 0;
    public $name = '';
    public $slug = '';
    public $taxonomy = '';
    public $description = '';
    public $parent = 0;
    public $count = 0;
}

function oxidepress_term_from_row($row)
{
    if (!is_array($row) || !isset($row['term_id'])) {
        return null;
    }
    $term = new WP_Term();
    $term->term_id = (int)$row['term_id'];
    $term->term_taxonomy_id = isset($row['term_taxonomy_id']) ? (int)$row['term_taxonomy_id'] : 0;
    $term->name = isset($row['name']) ? (string)$row['name'] : '';
    $term->slug = isset($row['slug']) ? (string)$row['slug'] : '';
    $term->taxonomy = isset($row['taxonomy']) ? (string)$row['taxonomy'] : '';
    $term->description = isset($row['description']) ? (string)$row['description'] : '';
    $term->parent = isset($row['parent']) ? (int)$row['parent'] : 0;
    $term->count = isset($row['count']) ? (int)$row['count'] : 0;
    return $term;
}

function wp_insert_term($term, $taxonomy, $args = array())
{
    $name = oxidepress_text($term);
    $taxonomy = sanitize_key($taxonomy);
    if ($name === '' || $taxonomy === '') {
        return new WP_Error('invalid_term', 'term is not allowed');
    }
    if (!is_array($args)) {
        $args = array();
    }
    $row = oxidepress_call('insert_term', array('term' => array(
        'taxonomy' => $taxonomy,
        'name' => $name,
        'slug' => isset($args['slug']) ? (string)$args['slug'] : '',
        'description' => isset($args['description']) ? (string)$args['description'] : '',
        'parent' => isset($args['parent']) ? (int)$args['parent'] : 0,
    )));
    if (!is_array($row) || !isset($row['term_id'])) {
        return new WP_Error('term_exists', 'term was not stored');
    }
    return array(
        'term_id' => (int)$row['term_id'],
        'term_taxonomy_id' => isset($row['term_taxonomy_id']) ? (int)$row['term_taxonomy_id'] : 0,
    );
}

function get_terms($args = array(), $deprecated = '')
{
    if (is_string($args)) {
        $args = array('taxonomy' => $args);
    }
    if (!is_array($args)) {
        $args = array();
    }
    $taxonomy = isset($args['taxonomy']) ? sanitize_key($args['taxonomy']) : '';
    if ($taxonomy === '') {
        return array();
    }
    $rows = oxidepress_call('get_terms', array('taxonomy' => $taxonomy));
    if (!is_array($rows)) {
        return array();
    }
    $name = isset($args['name']) ? (string)$args['name'] : '';
    $terms = array();
    foreach ($rows as $row) {
        $term = oxidepress_term_from_row($row);
        if (!$term) {
            continue;
        }
        if ($name !== '' && $term->name !== $name) {
            continue;
        }
        $terms[] = $term;
        $limit = isset($args['number']) ? (int)$args['number'] : 40;
        if ($limit > 0 && count($terms) >= $limit) {
            break;
        }
    }
    return $terms;
}

function wp_set_object_terms($object_id, $terms, $taxonomy, $append = false)
{
    $id = (int)$object_id;
    $taxonomy = sanitize_key($taxonomy);
    if ($id <= 0 || $taxonomy === '') {
        return new WP_Error('invalid_term', 'term is not allowed');
    }
    if (!is_array($terms)) {
        $terms = array($terms);
    }
    $ids = array();
    foreach ($terms as $term) {
        if (count($ids) >= 20) {
            break;
        }
        if (is_int($term) || (is_string($term) && ctype_digit($term))) {
            $ids[] = (int)$term;
            continue;
        }
        $name = oxidepress_text($term);
        if ($name === '') {
            continue;
        }
        $existing = get_terms(array('taxonomy' => $taxonomy, 'name' => $name, 'number' => 1));
        if ($existing) {
            $ids[] = (int)$existing[0]->term_id;
            continue;
        }
        $created = wp_insert_term($name, $taxonomy);
        if (is_array($created) && isset($created['term_id'])) {
            $ids[] = (int)$created['term_id'];
        }
    }
    $ok = oxidepress_call('set_object_terms', array(
        'post_id' => $id,
        'taxonomy' => $taxonomy,
        'terms' => $ids,
        'append' => (bool)$append,
    ));
    if (!$ok) {
        return new WP_Error('invalid_term', 'terms were not assigned');
    }
    return $ids;
}

function wp_get_object_terms($object_ids, $taxonomies, $args = array())
{
    $id = is_array($object_ids) ? (int)reset($object_ids) : (int)$object_ids;
    $taxonomy = is_array($taxonomies) ? (string)reset($taxonomies) : (string)$taxonomies;
    $taxonomy = sanitize_key($taxonomy);
    if ($id <= 0 || $taxonomy === '') {
        return array();
    }
    $rows = oxidepress_call('object_terms', array(
        'post_id' => $id,
        'taxonomy' => $taxonomy,
    ));
    if (!is_array($rows)) {
        return array();
    }
    $terms = array();
    foreach ($rows as $row) {
        $term = oxidepress_term_from_row($row);
        if ($term) {
            $terms[] = $term;
        }
    }
    return $terms;
}

function wp_delete_term($term, $taxonomy, $args = array())
{
    $id = (int)$term;
    $taxonomy = sanitize_key($taxonomy);
    if ($id <= 0 || $taxonomy === '') {
        return false;
    }
    return (bool)oxidepress_call('delete_term', array(
        'term_id' => $id,
        'taxonomy' => $taxonomy,
    ));
}

class WP_Comment
{
    public $comment_ID = 0;
    public $comment_post_ID = 0;
    public $comment_author = '';
    public $comment_author_email = '';
    public $comment_content = '';
    public $comment_approved = '0';
    public $comment_parent = 0;
}

function oxidepress_comment_from_row($row)
{
    if (!is_array($row) || !isset($row['id'])) {
        return null;
    }
    $comment = new WP_Comment();
    $comment->comment_ID = (int)$row['id'];
    $comment->comment_post_ID = isset($row['post_id']) ? (int)$row['post_id'] : 0;
    $comment->comment_author = isset($row['author']) ? (string)$row['author'] : '';
    $comment->comment_author_email = isset($row['email']) ? (string)$row['email'] : '';
    $comment->comment_content = isset($row['content']) ? (string)$row['content'] : '';
    $comment->comment_approved = isset($row['status']) ? (string)$row['status'] : '0';
    $comment->comment_parent = isset($row['parent']) ? (int)$row['parent'] : 0;
    return $comment;
}

function wp_insert_comment($commentdata)
{
    if (!is_array($commentdata)) {
        return false;
    }
    $row = oxidepress_call('insert_comment', array('comment' => array(
        'post_id' => isset($commentdata['comment_post_ID']) ? (int)$commentdata['comment_post_ID'] : 0,
        'author' => isset($commentdata['comment_author']) ? (string)$commentdata['comment_author'] : '',
        'email' => isset($commentdata['comment_author_email']) ? (string)$commentdata['comment_author_email'] : '',
        'content' => isset($commentdata['comment_content']) ? (string)$commentdata['comment_content'] : '',
        'status' => isset($commentdata['comment_approved']) ? (string)$commentdata['comment_approved'] : '0',
        'parent' => isset($commentdata['comment_parent']) ? (int)$commentdata['comment_parent'] : 0,
    )));
    if (!is_array($row) || !isset($row['id'])) {
        return false;
    }
    return (int)$row['id'];
}

function get_comment($comment = null, $output = 'OBJECT')
{
    if ($comment instanceof WP_Comment) {
        return $comment;
    }
    $id = (int)$comment;
    if ($id <= 0) {
        return null;
    }
    $row = oxidepress_call('get_comment', array('comment_id' => $id));
    return oxidepress_comment_from_row($row);
}

function get_comments($args = array())
{
    if (!is_array($args)) {
        $args = array();
    }
    $status = isset($args['status']) ? (string)$args['status'] : 'approve';
    $limit = isset($args['number']) ? (int)$args['number'] : 10;
    $rows = oxidepress_call('query_comments', array('query' => array(
        'post_id' => isset($args['post_id']) ? (int)$args['post_id'] : 0,
        'status' => $status,
        'limit' => $limit,
    )));
    if (!is_array($rows)) {
        return array();
    }
    $comments = array();
    foreach ($rows as $row) {
        $comment = oxidepress_comment_from_row($row);
        if ($comment) {
            $comments[] = $comment;
        }
    }
    return $comments;
}

function wp_delete_comment($comment_id, $force_delete = false)
{
    $id = (int)$comment_id;
    if ($id <= 0) {
        return false;
    }
    return (bool)oxidepress_call('delete_comment', array(
        'comment_id' => $id,
        'force' => (bool)$force_delete,
    ));
}

function get_term_meta($term_id, $key = '', $single = false)
{
    $id = (int)$term_id;
    $key = (string)$key;
    if ($id <= 0) {
        return $single ? '' : array();
    }
    $rows = oxidepress_call('get_term_meta', array(
        'term_id' => $id,
        'meta_key' => $key,
    ));
    if (!is_array($rows)) {
        return $single ? '' : array();
    }
    if ($key === '') {
        $grouped = array();
        foreach ($rows as $row) {
            if (!is_array($row)) {
                continue;
            }
            $name = isset($row['key']) ? (string)$row['key'] : '';
            $grouped[$name][] = isset($row['value']) ? (string)$row['value'] : '';
        }
        return $grouped;
    }
    $values = array();
    foreach ($rows as $row) {
        if (is_array($row)) {
            $values[] = isset($row['value']) ? (string)$row['value'] : '';
        }
    }
    if ($single) {
        return count($values) ? $values[0] : '';
    }
    return $values;
}

function update_term_meta($term_id, $meta_key, $meta_value, $prev_value = '')
{
    $id = (int)$term_id;
    $key = (string)$meta_key;
    $value = oxidepress_meta_text($meta_value);
    $prev = oxidepress_meta_text($prev_value);
    if ($id <= 0 || $key === '' || $value === null || $prev === null) {
        return false;
    }
    return (bool)oxidepress_call('update_term_meta', array(
        'term_id' => $id,
        'meta_key' => $key,
        'value' => $value,
        'prev' => $prev,
    ));
}

function delete_term_meta($term_id, $meta_key, $meta_value = '')
{
    $id = (int)$term_id;
    $key = (string)$meta_key;
    $prev = oxidepress_meta_text($meta_value);
    if ($id <= 0 || $key === '' || $prev === null) {
        return false;
    }
    return (bool)oxidepress_call('delete_term_meta', array(
        'term_id' => $id,
        'meta_key' => $key,
        'prev' => $prev,
    ));
}

function post_type_exists($post_type)
{
    global $oxidepress_post_types;
    return isset($oxidepress_post_types[sanitize_key($post_type)]);
}

function add_menu_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $icon_url = '', $position = null)
{
    return oxidepress_add_menu('', $page_title, $menu_title, $capability, $menu_slug, $callback);
}

function add_submenu_page($parent_slug, $page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return oxidepress_add_menu($parent_slug, $page_title, $menu_title, $capability, $menu_slug, $callback);
}

function add_management_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('tools.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_options_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('options-general.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_plugins_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('plugins.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_theme_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('themes.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_users_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('users.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_dashboard_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('index.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_posts_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('edit.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_media_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('upload.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_pages_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('edit.php?post_type=page', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function add_comments_page($page_title, $menu_title, $capability, $menu_slug, $callback = '', $position = null)
{
    return add_submenu_page('edit-comments.php', $page_title, $menu_title, $capability, $menu_slug, $callback, $position);
}

function oxidepress_add_menu($parent_slug, $page_title, $menu_title, $capability, $menu_slug, $callback)
{
    global $oxidepress_admin_menus;

    $parent = (string)$parent_slug;
    $slug = (string)$menu_slug;
    $hook = ($parent === '' ? 'toplevel_page_' : $parent . '_page_') . $slug;
    $title = oxidepress_text($menu_title);
    if ($title === '') {
        $title = oxidepress_text($page_title);
    }
    if ($title === '') {
        $title = $slug;
    }
    $oxidepress_admin_menus[] = array(
        'parent' => $parent,
        'slug' => $slug,
        'hook' => $hook,
        'page_title' => oxidepress_text($page_title) === '' ? $title : oxidepress_text($page_title),
        'menu_title' => $title,
        'capability' => (string)$capability,
        'callback_id' => oxidepress_keep_callback($callback),
    );
    return $hook;
}

class WP_Post_Type
{
    public $name;
    public $label;
    public $labels;
    public $public = false;
    public $args = array();

    public function __construct($name, $args)
    {
        $this->name = $name;
        $this->args = $args;
        $labels = isset($args['labels']) && is_array($args['labels']) ? $args['labels'] : array();
        $this->labels = (object)$labels;
        $this->label = isset($labels['name']) ? (string)$labels['name'] : $name;
        $this->public = !empty($args['public']);
    }
}

class WP_Error
{
    public $errors = array();
    public $error_data = array();

    public function __construct($code = '', $message = '', $data = '')
    {
        if ($code !== '') {
            $this->errors[$code] = array((string)$message);
            if ($data !== '') {
                $this->error_data[$code] = $data;
            }
        }
    }

    public function get_error_message($code = '')
    {
        if ($code === '') {
            $code = $this->get_error_code();
        }
        if (!isset($this->errors[$code][0])) {
            return '';
        }
        return $this->errors[$code][0];
    }

    public function get_error_code()
    {
        $codes = array_keys($this->errors);
        return isset($codes[0]) ? $codes[0] : '';
    }
}

function is_wp_error($thing)
{
    return $thing instanceof WP_Error;
}

function wp_remote_get($url, $args = array())
{
    if (!is_array($args)) {
        $args = array();
    }
    $args['method'] = 'GET';
    return oxidepress_remote($url, $args);
}

function wp_remote_post($url, $args = array())
{
    if (!is_array($args)) {
        $args = array();
    }
    $args['method'] = 'POST';
    return oxidepress_remote($url, $args);
}

function wp_remote_request($url, $args = array())
{
    return oxidepress_remote($url, $args);
}

function wp_safe_remote_get($url, $args = array())
{
    return wp_remote_get($url, $args);
}

function wp_safe_remote_post($url, $args = array())
{
    return wp_remote_post($url, $args);
}

function wp_remote_retrieve_body($response)
{
    if (is_wp_error($response) || !is_array($response)) {
        return '';
    }
    return isset($response['body']) ? (string)$response['body'] : '';
}

function wp_remote_retrieve_response_code($response)
{
    if (is_wp_error($response) || !is_array($response)) {
        return '';
    }
    return isset($response['response']['code']) ? (int)$response['response']['code'] : '';
}

function wp_remote_retrieve_response_message($response)
{
    if (is_wp_error($response) || !is_array($response)) {
        return '';
    }
    return isset($response['response']['message']) ? (string)$response['response']['message'] : '';
}

function oxidepress_remote($url, $args)
{
    if (!is_array($args)) {
        $args = array();
    }
    $method = isset($args['method']) ? strtoupper((string)$args['method']) : 'GET';
    $headers = array();
    if (isset($args['headers']) && is_array($args['headers'])) {
        foreach ($args['headers'] as $name => $value) {
            if (is_int($name)) {
                continue;
            }
            $headers[(string)$name] = oxidepress_text($value);
        }
    }
    $body = '';
    if (isset($args['body'])) {
        if (is_array($args['body'])) {
            $body = http_build_query($args['body']);
        } else {
            $body = oxidepress_text($args['body']);
        }
    }
    $timeout = isset($args['timeout']) ? (int)$args['timeout'] : 5;
    if ($timeout < 1) {
        $timeout = 1;
    }
    if ($timeout > 15) {
        $timeout = 15;
    }
    $result = oxidepress_call('http', array(
        'method' => $method,
        'url' => (string)$url,
        'headers' => $headers,
        'body' => $body,
        'timeout' => $timeout,
    ));
    if (!is_array($result)) {
        return new WP_Error('http_request_failed', 'http host returned no result');
    }
    if (isset($result['error']) && $result['error'] !== null && $result['error'] !== '') {
        return new WP_Error('http_request_failed', (string)$result['error']);
    }
    $code = isset($result['status']) ? (int)$result['status'] : 0;
    $message = isset($result['reason']) ? (string)$result['reason'] : '';
    return array(
        'headers' => array(),
        'body' => isset($result['body']) ? (string)$result['body'] : '',
        'response' => array(
            'code' => $code,
            'message' => $message,
        ),
        'cookies' => array(),
        'filename' => null,
    );
}

function sanitize_text_field($text)
{
    $text = oxidepress_text($text);
    $text = preg_replace('/[\r\n\t ]+/', ' ', $text);
    if ($text === null) {
        return '';
    }
    return trim(strip_tags($text));
}

function wp_parse_args($args, $defaults = array())
{
    if (is_object($args)) {
        $args = get_object_vars($args);
    } elseif (is_string($args)) {
        parse_str($args, $parsed);
        $args = $parsed;
    }
    if (!is_array($args)) {
        $args = array();
    }
    if (!is_array($defaults)) {
        $defaults = array();
    }
    return array_merge($defaults, $args);
}

function has_action($tag, $callback = false)
{
    return oxidepress_has_hook('action', $tag, $callback);
}

function has_filter($tag, $callback = false)
{
    return oxidepress_has_hook('filter', $tag, $callback);
}

function did_action($tag)
{
    global $oxidepress_did;
    $tag = (string)$tag;
    return isset($oxidepress_did[$tag]) ? (int)$oxidepress_did[$tag] : 0;
}

function oxidepress_has_hook($kind, $tag, $callback)
{
    global $oxidepress_hooks, $oxidepress_callbacks, $oxidepress_disabled;
    $tag = (string)$tag;
    if (empty($oxidepress_hooks[$kind][$tag])) {
        return false;
    }
    foreach ($oxidepress_hooks[$kind][$tag] as $entry) {
        if (!empty($oxidepress_disabled[$entry['id']])) {
            continue;
        }
        if ($callback === false) {
            return true;
        }
        if (oxidepress_same_callback($oxidepress_callbacks[$entry['id']], $callback)) {
            return (int)$entry['priority'];
        }
    }
    return false;
}

function load_plugin_textdomain($domain, $deprecated = false, $plugin_rel_path = false)
{
    return true;
}

function register_setting($option_group, $option_name, $args = array())
{
    global $oxidepress_registered_settings;

    $option_name = (string)$option_name;
    $option_group = (string)$option_group;
    if (!oxidepress_token($option_name, 191) || !oxidepress_token($option_group, 80)) {
        return false;
    }
    if (oxidepress_option_is_protected($option_name)) {
        return false;
    }
    $type = 'string';
    $sanitize_id = null;
    if (is_callable($args)) {
        $sanitize_id = oxidepress_keep_callback($args);
    } elseif (is_array($args)) {
        if (isset($args['type'])) {
            $candidate = (string)$args['type'];
            if ($candidate === 'boolean' || $candidate === 'integer' || $candidate === 'number' || $candidate === 'string') {
                $type = $candidate;
            }
        }
        if (isset($args['sanitize_callback']) && is_callable($args['sanitize_callback'])) {
            $sanitize_id = oxidepress_keep_callback($args['sanitize_callback']);
        }
    }
    $oxidepress_registered_settings[$option_name] = array(
        'group' => $option_group,
        'type' => $type,
        'sanitize_id' => $sanitize_id,
    );
    return true;
}

function add_settings_section($id, $title, $callback, $page)
{
    global $oxidepress_settings_sections;

    $page = (string)$page;
    if ($page === '') {
        return;
    }
    if (!isset($oxidepress_settings_sections[$page])) {
        $oxidepress_settings_sections[$page] = array();
    }
    $oxidepress_settings_sections[$page][] = array(
        'id' => (string)$id,
        'title' => oxidepress_text($title),
        'callback_id' => oxidepress_keep_callback($callback),
    );
}

function add_settings_field($id, $title, $callback, $page, $section = 'default', $args = array())
{
    global $oxidepress_settings_fields;

    $page = (string)$page;
    if ($page === '') {
        return;
    }
    if (!isset($oxidepress_settings_fields[$page])) {
        $oxidepress_settings_fields[$page] = array();
    }
    $oxidepress_settings_fields[$page][] = array(
        'id' => (string)$id,
        'title' => oxidepress_text($title),
        'section' => (string)$section,
        'callback_id' => oxidepress_keep_callback($callback),
    );
}

function settings_fields($option_group)
{
    global $oxidepress_rendering_page, $oxidepress_settings_nonce;

    echo '<input type="hidden" name="option_page" value="' . esc_attr($option_group) . '" />';
    echo '<input type="hidden" name="action" value="update" />';
    if ($oxidepress_rendering_page !== '') {
        echo '<input type="hidden" name="oxidepress_page" value="' . esc_attr($oxidepress_rendering_page) . '" />';
    }
    if ($oxidepress_settings_nonce !== '') {
        echo '<input type="hidden" id="_wpnonce" name="_wpnonce" value="' . esc_attr($oxidepress_settings_nonce) . '" />';
    }
}

function do_settings_sections($page)
{
    global $oxidepress_settings_sections, $oxidepress_settings_fields, $oxidepress_callbacks;

    $page = (string)$page;
    $sections = isset($oxidepress_settings_sections[$page]) ? $oxidepress_settings_sections[$page] : array();
    if (!$sections) {
        $sections = array(array('id' => 'default', 'title' => '', 'callback_id' => null));
    }
    foreach ($sections as $section) {
        if ($section['title'] !== '') {
            echo '<h2>' . esc_html($section['title']) . '</h2>';
        }
        if ($section['callback_id'] !== null && isset($oxidepress_callbacks[$section['callback_id']])) {
            call_user_func($oxidepress_callbacks[$section['callback_id']]);
        }
        echo '<table class="form-table"><tbody>';
        $fields = isset($oxidepress_settings_fields[$page]) ? $oxidepress_settings_fields[$page] : array();
        foreach ($fields as $field) {
            if ((string)$field['section'] !== (string)$section['id']) {
                continue;
            }
            echo '<tr><th scope="row">' . esc_html($field['title']) . '</th><td>';
            if ($field['callback_id'] !== null && isset($oxidepress_callbacks[$field['callback_id']])) {
                call_user_func($oxidepress_callbacks[$field['callback_id']]);
            }
            echo '</td></tr>';
        }
        echo '</tbody></table>';
    }
}

function submit_button($text = '', $type = 'primary', $name = 'submit', $wrap = true, $other_attributes = '')
{
    if ($text === '' || $text === null) {
        $text = 'Save Changes';
    }
    echo '<p class="submit"><button type="submit" name="' . esc_attr($name) . '" class="button button-primary">' . esc_html(oxidepress_text($text)) . '</button></p>';
}

function checked($checked, $current = true, $display = true)
{
    $result = ((string)$checked === (string)$current) ? ' checked="checked"' : '';
    if ($display) {
        echo $result;
    }
    return $result;
}

function selected($selected, $current = true, $display = true)
{
    $result = ((string)$selected === (string)$current) ? ' selected="selected"' : '';
    if ($display) {
        echo $result;
    }
    return $result;
}

function oxidepress_keep_callback($callback)
{
    global $oxidepress_callbacks;

    if (!is_callable($callback)) {
        return null;
    }
    $id = count($oxidepress_callbacks);
    $oxidepress_callbacks[] = $callback;
    return $id;
}

function oxidepress_token($value, $max)
{
    $value = (string)$value;
    $length = strlen($value);
    if ($length < 1 || $length > $max) {
        return false;
    }
    return preg_match('/^[A-Za-z0-9_-]+$/', $value) === 1;
}

function oxidepress_option_is_protected($name)
{
    $blocked = array(
        'siteurl',
        'home',
        'active_plugins',
        'template',
        'stylesheet',
        'wp_user_roles',
        'cron',
        'rewrite_rules',
        'uninstall_plugins',
        'users_can_register',
        'default_role',
        'admin_email',
        'auth_key',
        'secure_auth_key',
        'logged_in_key',
        'nonce_key',
        'auth_salt',
        'secure_auth_salt',
        'logged_in_salt',
        'nonce_salt',
    );
    if (in_array($name, $blocked, true)) {
        return true;
    }
    if (strpos($name, '_rustpress_') === 0 || strpos($name, 'rustpress_') === 0) {
        return true;
    }
    return false;
}

function oxidepress_enter_admin_screen()
{
    global $oxidepress_admin_depth, $oxidepress_admin_ready;

    $oxidepress_admin_depth = (int)$oxidepress_admin_depth + 1;
    if ($oxidepress_admin_ready) {
        return;
    }
    $oxidepress_admin_ready = true;
    if (!did_action('admin_menu')) {
        do_action('admin_menu');
    }
}

function oxidepress_leave_admin_screen()
{
    global $oxidepress_admin_depth;
    $oxidepress_admin_depth = max(0, (int)$oxidepress_admin_depth - 1);
}

function oxidepress_admin_page_list()
{
    global $oxidepress_admin_menus;

    oxidepress_enter_admin_screen();
    try {
        $pages = array();
        foreach ($oxidepress_admin_menus as $menu) {
            if (!oxidepress_token($menu['slug'], 80)) {
                continue;
            }
            $pages[] = array(
                'slug' => $menu['slug'],
                'parent' => $menu['parent'],
                'page_title' => $menu['page_title'],
                'menu_title' => $menu['menu_title'],
            );
            if (count($pages) >= 40) {
                break;
            }
        }
        return $pages;
    } finally {
        oxidepress_leave_admin_screen();
    }
}

function oxidepress_render_admin_page($slug, $nonce)
{
    global $oxidepress_admin_menus, $oxidepress_callbacks, $oxidepress_rendering_page, $oxidepress_settings_nonce;

    if (!oxidepress_token($slug, 80)) {
        return array('ok' => false, 'error' => 'unknown page');
    }
    if (!preg_match('/^[A-Za-z0-9]{0,128}$/', (string)$nonce)) {
        $nonce = '';
    }
    oxidepress_enter_admin_screen();
    try {
        $page = null;
        foreach ($oxidepress_admin_menus as $menu) {
            if ($menu['slug'] === $slug) {
                $page = $menu;
                break;
            }
        }
        if ($page === null) {
            return array('ok' => false, 'error' => 'unknown page');
        }
        $oxidepress_rendering_page = $slug;
        $oxidepress_settings_nonce = (string)$nonce;
        ob_start();
        if ($page['callback_id'] !== null && isset($oxidepress_callbacks[$page['callback_id']])) {
            call_user_func($oxidepress_callbacks[$page['callback_id']]);
        }
        $html = ob_get_clean();
        if ($html === false) {
            $html = '';
        }
        if (stripos($html, '<form') === false && oxidepress_page_has_fields($slug)) {
            ob_start();
            oxidepress_default_settings_form($slug);
            $extra = ob_get_clean();
            if ($extra !== false) {
                $html .= $extra;
            }
        }
    } finally {
        $oxidepress_rendering_page = '';
        $oxidepress_settings_nonce = '';
        oxidepress_leave_admin_screen();
    }
    $html = oxidepress_rewrite_options_form($html);
    if (strlen($html) > 200000) {
        $html = substr($html, 0, 200000);
    }
    return array(
        'ok' => true,
        'page_title' => $page['page_title'],
        'menu_title' => $page['menu_title'],
        'html' => $html,
    );
}

function oxidepress_page_has_fields($slug)
{
    global $oxidepress_settings_fields;
    return !empty($oxidepress_settings_fields[$slug]);
}

function oxidepress_default_settings_form($slug)
{
    $group = $slug;
    global $oxidepress_registered_settings;
    foreach ($oxidepress_registered_settings as $setting) {
        if ($setting['group'] === $slug) {
            $group = $slug;
            break;
        }
    }
    echo '<form method="post" action="/wp-admin/options.php">';
    settings_fields($group);
    do_settings_sections($slug);
    submit_button();
    echo '</form>';
}

function oxidepress_rewrite_options_form($html)
{
    $html = str_replace(
        array('action="options.php"', "action='options.php'"),
        array('action="/wp-admin/options.php"', "action='/wp-admin/options.php'"),
        $html
    );
    $site = rtrim(oxidepress_env('OXIDEPRESS_SITE_URL', 'http://localhost'), '/');
    return str_replace($site . '/wp-admin/options.php', '/wp-admin/options.php', $html);
}

function oxidepress_save_settings($group, $values)
{
    global $oxidepress_registered_settings, $oxidepress_callbacks;

    if (!oxidepress_token($group, 80)) {
        return array();
    }
    oxidepress_enter_admin_screen();
    try {
    $saved = array();
    $count = 0;
    foreach ($values as $name => $value) {
        $name = (string)$name;
        if (!isset($oxidepress_registered_settings[$name])) {
            continue;
        }
        if ($count >= 40) {
            break;
        }
        $count++;
        $setting = $oxidepress_registered_settings[$name];
        if ($setting['group'] !== $group || oxidepress_option_is_protected($name)) {
            continue;
        }
        $clean = oxidepress_clean_setting($value, $setting, $oxidepress_callbacks);
        if ($clean === null) {
            continue;
        }
        if (update_option($name, $clean)) {
            $saved[] = $name;
        }
    }
    return $saved;
    } finally {
        oxidepress_leave_admin_screen();
    }
}

function oxidepress_clean_setting($value, $setting, $callbacks)
{
    if (is_array($value) || is_object($value)) {
        return null;
    }
    $value = str_replace("\0", '', (string)$value);
    if (strlen($value) > 10000) {
        $value = substr($value, 0, 10000);
    }
    if ($setting['sanitize_id'] !== null && isset($callbacks[$setting['sanitize_id']])) {
        try {
            $clean = call_user_func($callbacks[$setting['sanitize_id']], $value);
        } catch (Throwable $error) {
            return null;
        }
        if ($clean === false || $clean === null || is_array($clean) || is_object($clean)) {
            return null;
        }
        $value = str_replace("\0", '', (string)$clean);
        if (strlen($value) > 10000) {
            $value = substr($value, 0, 10000);
        }
    }
    $type = $setting['type'];
    if ($type === 'boolean') {
        return ($value === '1' || $value === 'on' || $value === 'true' || $value === 'yes') ? '1' : '0';
    }
    if ($type === 'integer') {
        if (!preg_match('/^-?[0-9]{1,12}$/', $value)) {
            return null;
        }
        return $value;
    }
    if ($type === 'number') {
        if (!is_numeric($value)) {
            return null;
        }
        return $value;
    }
    return trim(strip_tags($value));
}

function add_allowed_options($new_options, $options = '')
{
    return is_array($new_options) ? $new_options : array();
}

function wp_enqueue_script($handle, $src = '', $deps = array(), $ver = false, $args = array())
{
    global $oxidepress_scripts, $oxidepress_script_queue;

    $handle = (string)$handle;
    if ($src !== '' || !isset($oxidepress_scripts[$handle])) {
        wp_register_script($handle, $src, $deps, $ver, $args);
    }
    if (!in_array($handle, $oxidepress_script_queue, true)) {
        $oxidepress_script_queue[] = $handle;
    }
}

function wp_enqueue_style($handle, $src = '', $deps = array(), $ver = false, $media = 'all')
{
    global $oxidepress_styles, $oxidepress_style_queue;

    $handle = (string)$handle;
    if ($src !== '' || !isset($oxidepress_styles[$handle])) {
        wp_register_style($handle, $src, $deps, $ver, $media);
    }
    if (!in_array($handle, $oxidepress_style_queue, true)) {
        $oxidepress_style_queue[] = $handle;
    }
}

function wp_register_script($handle, $src, $deps = array(), $ver = false, $args = array())
{
    global $oxidepress_scripts;

    $in_footer = false;
    if (is_array($args)) {
        $in_footer = !empty($args['in_footer']);
    } else {
        $in_footer = (bool)$args;
    }
    $oxidepress_scripts[(string)$handle] = array(
        'src' => (string)$src,
        'deps' => oxidepress_dep_list($deps),
        'ver' => $ver,
        'in_footer' => $in_footer,
    );
    return true;
}

function wp_register_style($handle, $src, $deps = array(), $ver = false, $media = 'all')
{
    global $oxidepress_styles;

    $media = (string)$media;
    if ($media === '') {
        $media = 'all';
    }
    $oxidepress_styles[(string)$handle] = array(
        'src' => (string)$src,
        'deps' => oxidepress_dep_list($deps),
        'ver' => $ver,
        'media' => $media,
    );
    return true;
}

function oxidepress_print_head_assets()
{
    if (!did_action('wp_enqueue_scripts')) {
        do_action('wp_enqueue_scripts');
    }
    oxidepress_print_styles();
    oxidepress_print_scripts(false);
}

function oxidepress_print_footer_assets()
{
    if (!did_action('wp_enqueue_scripts')) {
        do_action('wp_enqueue_scripts');
    }
    oxidepress_print_styles();
    oxidepress_print_scripts(true);
}

function oxidepress_print_styles()
{
    global $oxidepress_style_queue;

    $stack = array();
    foreach ($oxidepress_style_queue as $handle) {
        oxidepress_print_one_style($handle, $stack);
    }
}

function oxidepress_print_one_style($handle, &$stack)
{
    global $oxidepress_styles, $oxidepress_done_styles;

    if (isset($oxidepress_done_styles[$handle]) || isset($stack[$handle])) {
        return;
    }
    if (!isset($oxidepress_styles[$handle])) {
        return;
    }
    $stack[$handle] = true;
    $item = $oxidepress_styles[$handle];
    foreach ($item['deps'] as $dep) {
        oxidepress_print_one_style($dep, $stack);
    }
    unset($stack[$handle]);
    $oxidepress_done_styles[$handle] = true;
    $src = oxidepress_asset_src($item['src'], $item['ver']);
    if ($src === '') {
        return;
    }
    echo '<link rel="stylesheet" id="' . esc_attr($handle) . '-css" href="' . esc_url($src) . '" media="' . esc_attr($item['media']) . '" />' . "\n";
}

function oxidepress_print_scripts($in_footer)
{
    global $oxidepress_script_queue;

    $stack = array();
    foreach ($oxidepress_script_queue as $handle) {
        oxidepress_print_one_script($handle, $in_footer, $stack);
    }
}

function oxidepress_print_one_script($handle, $in_footer, &$stack)
{
    global $oxidepress_scripts, $oxidepress_done_scripts;

    if (isset($oxidepress_done_scripts[$handle]) || isset($stack[$handle])) {
        return;
    }
    if (!isset($oxidepress_scripts[$handle])) {
        return;
    }
    $item = $oxidepress_scripts[$handle];
    $stack[$handle] = true;
    foreach ($item['deps'] as $dep) {
        oxidepress_print_one_script($dep, $in_footer, $stack);
    }
    unset($stack[$handle]);
    $want_footer = !empty($item['in_footer']);
    if ($want_footer !== $in_footer && !$in_footer) {
        return;
    }
    $oxidepress_done_scripts[$handle] = true;
    $src = oxidepress_asset_src($item['src'], $item['ver']);
    if ($src === '') {
        return;
    }
    echo '<script id="' . esc_attr($handle) . '-js" src="' . esc_url($src) . '"></script>' . "\n";
}

function oxidepress_dep_list($deps)
{
    if (!is_array($deps)) {
        return array();
    }
    $list = array();
    foreach ($deps as $dep) {
        $dep = (string)$dep;
        if ($dep !== '') {
            $list[] = $dep;
        }
    }
    return $list;
}

function oxidepress_asset_src($src, $ver)
{
    $src = trim((string)$src);
    if ($src === '') {
        return '';
    }
    if ($ver === false || $ver === null || $ver === '') {
        return $src;
    }
    $join = strpos($src, '?') === false ? '?' : '&';
    return $src . $join . 'ver=' . rawurlencode((string)$ver);
}

class WP_Widget
{
    public $id_base;
    public $name;
    public $option_name;
    public $widget_options = array();
    public $control_options = array();
    public $number = false;
    public $id = false;
    public $updated = false;

    public function __construct($id_base, $name, $widget_options = array(), $control_options = array())
    {
        $this->id_base = $id_base;
        $this->name = $name;
        $this->option_name = 'widget_' . $id_base;
        $this->widget_options = is_array($widget_options) ? $widget_options : array();
        $this->control_options = is_array($control_options) ? $control_options : array();
    }

    public function get_field_id($field_name)
    {
        return 'widget-' . $this->id_base . '-' . $field_name;
    }

    public function get_field_name($field_name)
    {
        return 'widget-' . $this->id_base . '[' . $field_name . ']';
    }
}

function oxidepress_abspath_from_plugins($plugins_dir)
{
    $plugins = rtrim(oxidepress_normalize_path($plugins_dir), '/');
    if ($plugins === '') {
        $cwd = rtrim(oxidepress_normalize_path(getcwd()), '/');
        return oxidepress_trail($cwd);
    }

    $parent = oxidepress_dir_name($plugins);
    $grand = oxidepress_dir_name($parent);
    if (oxidepress_path_is(oxidepress_base_name($plugins), 'plugins')
        && oxidepress_path_is(oxidepress_base_name($parent), 'wp-content')
        && $grand !== ''
        && $grand !== $parent
    ) {
        return oxidepress_trail($grand);
    }

    $cursor = $parent;
    for ($step = 0; $step < 8; $step++) {
        if ($cursor === '' || $cursor === '/' || $cursor === '.') {
            break;
        }
        if (is_file($cursor . '/wp-load.php')) {
            return oxidepress_trail($cursor);
        }
        $next = oxidepress_dir_name($cursor);
        if ($next === '' || $next === $cursor) {
            break;
        }
        $cursor = $next;
    }

    if ($parent !== '' && $parent !== $plugins) {
        return oxidepress_trail($parent);
    }
    return oxidepress_trail($plugins);
}

function oxidepress_content_dir($plugins_dir, $abspath)
{
    $plugins = rtrim(oxidepress_normalize_path($plugins_dir), '/');
    if ($plugins === '') {
        return rtrim(oxidepress_normalize_path($abspath), '/') . '/wp-content';
    }
    $parent = oxidepress_dir_name($plugins);
    if (oxidepress_path_is(oxidepress_base_name($parent), 'wp-content')) {
        return $parent;
    }
    $beside = rtrim(oxidepress_normalize_path($abspath), '/') . '/wp-content';
    if (is_dir($beside)) {
        return rtrim(oxidepress_normalize_path($beside), '/');
    }
    return $parent === '' ? rtrim(oxidepress_normalize_path($abspath), '/') : $parent;
}

function oxidepress_dir_name($path)
{
    $path = rtrim(oxidepress_normalize_path($path), '/');
    if ($path === '' || $path === '/') {
        return '';
    }
    $slash = strrpos($path, '/');
    if ($slash === false) {
        return '';
    }
    if ($slash === 0) {
        return '/';
    }
    return substr($path, 0, $slash);
}

function oxidepress_base_name($path)
{
    $path = rtrim(oxidepress_normalize_path($path), '/');
    $slash = strrpos($path, '/');
    if ($slash === false) {
        return $path;
    }
    return substr($path, $slash + 1);
}

function oxidepress_trail($path)
{
    $path = rtrim(oxidepress_normalize_path($path), '/');
    if ($path === '') {
        return '/';
    }
    return $path . '/';
}

function oxidepress_path_is($name, $expected)
{
    if (DIRECTORY_SEPARATOR === '\\') {
        return strcasecmp($name, $expected) === 0;
    }
    return $name === $expected;
}

function oxidepress_normalize_path($path)
{
    $path = str_replace('\\', '/', (string)$path);
    $collapsed = preg_replace('#/+#', '/', $path);
    if ($collapsed === null) {
        return $path;
    }
    return $collapsed;
}

function oxidepress_text($value)
{
    if (is_array($value) || is_object($value)) {
        $encoded = json_encode($value);
        return $encoded === false ? '' : $encoded;
    }
    if ($value === null || $value === false) {
        return '';
    }
    return (string)$value;
}

function oxidepress_env($name, $default)
{
    $value = getenv($name);
    if ($value === false || $value === '') {
        return $default;
    }
    return $value;
}

function oxidepress_call($method, $params)
{
    global $oxidepress_request_id;

    $frame = array_merge(array(
        'id' => $oxidepress_request_id,
        'call' => $method,
    ), $params);
    oxidepress_write_frame($frame);

    while (true) {
        $reply = oxidepress_read_frame();
        if ($reply === null) {
            throw new RuntimeException('php host disconnected');
        }
        if (isset($reply['call'])) {
            throw new RuntimeException('unexpected host call from rust');
        }
        if (array_key_exists('result', $reply)) {
            return $reply['result'];
        }
    }
}

function oxidepress_read_frame()
{
    $header = oxidepress_read_exact(4);
    if ($header === null) {
        return null;
    }
    $parts = unpack('Vlen', $header);
    $len = (int)$parts['len'];
    if ($len <= 0 || $len > 8388608) {
        throw new RuntimeException('invalid frame length ' . $len);
    }
    $body = oxidepress_read_exact($len);
    if ($body === null) {
        throw new RuntimeException('truncated frame');
    }
    $decoded = json_decode($body, true);
    if (!is_array($decoded)) {
        throw new RuntimeException('frame is not a json object');
    }
    return $decoded;
}

function oxidepress_read_exact($count)
{
    $buf = '';
    while (strlen($buf) < $count) {
        $chunk = fread(STDIN, $count - strlen($buf));
        if ($chunk === false || $chunk === '') {
            return null;
        }
        $buf .= $chunk;
    }
    return $buf;
}

function oxidepress_write_frame($data)
{
    $paused = array();
    while (ob_get_level() > 0) {
        $chunk = ob_get_clean();
        $paused[] = ($chunk === false) ? '' : $chunk;
    }

    $flags = JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES;
    if (defined('JSON_INVALID_UTF8_SUBSTITUTE')) {
        $flags |= JSON_INVALID_UTF8_SUBSTITUTE;
    }
    $json = json_encode($data, $flags);
    if ($json === false) {
        $id = isset($data['id']) ? $data['id'] : 0;
        $json = json_encode(array(
            'id' => $id,
            'ok' => false,
            'error' => 'failed to encode frame',
        ));
    }

    $packet = pack('V', strlen($json)) . $json;
    $written = 0;
    $total = strlen($packet);
    while ($written < $total) {
        $count = fwrite(STDOUT, substr($packet, $written));
        if ($count === false || $count === 0) {
            oxidepress_resume_buffers($paused);
            throw new RuntimeException('failed to write frame');
        }
        $written += $count;
    }
    fflush(STDOUT);
    oxidepress_resume_buffers($paused);
}

function oxidepress_resume_buffers($paused)
{
    $index = count($paused);
    while ($index > 0) {
        $index--;
        ob_start();
        if ($paused[$index] !== '') {
            echo $paused[$index];
        }
    }
}

function oxidepress_shutdown()
{
    global $oxidepress_request_id, $oxidepress_fatal_sent;

    if ($oxidepress_fatal_sent) {
        return;
    }
    $err = error_get_last();
    if (!$err) {
        return;
    }
    $fatal = array(E_ERROR, E_PARSE, E_CORE_ERROR, E_COMPILE_ERROR, E_USER_ERROR);
    if (!in_array($err['type'], $fatal, true)) {
        return;
    }
    $oxidepress_fatal_sent = true;
    oxidepress_write_frame(array(
        'id' => $oxidepress_request_id,
        'ok' => false,
        'error' => $err['message'],
    ));
}
