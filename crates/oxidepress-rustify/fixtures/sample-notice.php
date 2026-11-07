<?php
/**
 * Plugin Name: Sample Notice
 * Description: Shows a one-line notice.
 * Version: 1.2.0
 * Author: OxidePress
 */

function sample_notice_init() {
    echo 'ready';
}

function sample_notice_filter($content) {
    return $content . ' noted';
}

function sample_notice_shortcode() {
    return 'notice';
}

add_action('init', 'sample_notice_init');
add_filter('the_content', 'sample_notice_filter', 15);
add_shortcode('sample_notice', 'sample_notice_shortcode');
include 'extra.php';
add_action($dynamic, 'sample_notice_init');
