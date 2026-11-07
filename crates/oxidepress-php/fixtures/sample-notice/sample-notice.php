<?php
/**
 * Plugin Name: Sample Notice
 * Description: Shows a one-line notice.
 * Version: 1.2.0
 * Author: OxidePress
 * Author URI: https://example.com/oxidepress
 */

require_once __DIR__ . '/includes.php';

function sample_notice_init() {
    update_option('notice_seen', 'yes');
    echo 'ready';
}

function sample_notice_filter($content) {
    if (!is_string($content)) {
        $content = (string) $content;
    }
    $suffix = get_option('notice_suffix', sample_notice_default_suffix());
    return $content . $suffix;
}

function sample_notice_shortcode($atts = array(), $content = null) {
    $atts = shortcode_atts(array('name' => 'notice'), $atts, 'sample_notice');
    return esc_html($atts['name']);
}

function sample_notice_activate() {
    add_option('notice_activated', '1');
    add_option('notice_basename', plugin_basename(__FILE__));
}

add_action('init', 'sample_notice_init');
add_filter('the_content', 'sample_notice_filter', 15);
add_shortcode('sample_notice', 'sample_notice_shortcode');
register_activation_hook(__FILE__, 'sample_notice_activate');
