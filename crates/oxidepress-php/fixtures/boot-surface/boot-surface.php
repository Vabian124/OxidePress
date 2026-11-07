<?php
/**
 * Plugin Name: Boot Surface
 * Description: Calls the same helpers the first real plugins need.
 * Version: 1.0.0
 * Author: OxidePress
 */

class Boot_Surface_Widget extends WP_Widget {
    public function __construct() {
        parent::__construct('boot_surface', 'Boot Surface');
    }
}

add_filter('use_widgets_block_editor', '__return_false');

add_action('init', 'boot_surface_init');
add_action('admin_notices', 'boot_surface_notice');

function boot_surface_init() {
    $widget = new Boot_Surface_Widget();
    update_option('boot_field', $widget->get_field_id('title'));
    update_option('boot_url', plugins_url('app.js', __FILE__));
    update_option('boot_joined', path_join(untrailingslashit(plugin_dir_path(__FILE__)), 'app.js'));
    update_option('boot_admin', is_admin() ? 'yes' : 'no');
    update_option('boot_locale', get_user_locale());
    update_option('boot_site', get_site_option('missing_site_option', 'fallback'));
    update_option('boot_query', add_query_arg('dolly', '1', home_url('notice')));
    update_option('boot_has_notice', has_action('admin_notices') ? 'yes' : 'no');
}

function boot_surface_notice() {
    echo esc_html_x(wptexturize('Hello Dolly'), 'lyric', 'boot-surface');
}
