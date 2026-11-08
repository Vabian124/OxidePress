<?php
// Modified by Vabian124 on 2026-11-08 to print enqueued scripts and styles.
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
$oxidepress_admin_menus = array();
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
    return false;
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
    return false;
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
    $oxidepress_admin_menus[] = array(
        'parent' => $parent,
        'slug' => $slug,
        'hook' => $hook,
        'page_title' => oxidepress_text($page_title),
        'menu_title' => oxidepress_text($menu_title),
        'capability' => (string)$capability,
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
}

function add_settings_section($id, $title, $callback, $page)
{
}

function add_settings_field($id, $title, $callback, $page, $section = 'default', $args = array())
{
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
