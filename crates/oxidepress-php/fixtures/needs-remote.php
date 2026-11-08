<?php
/**
 * Plugin Name: Needs Remote
 * Description: Calls the worker HTTP bridge.
 * Version: 0.0.1
 * Author: OxidePress
 */

add_action('init', function () {
    $blocked = wp_remote_get('http://127.0.0.1/secret');
    update_option(
        'remote_blocked',
        is_wp_error($blocked) ? $blocked->get_error_message() : 'allowed'
    );

    $response = wp_remote_post('https://example.com/1.1/comment-check', array(
        'body' => 'blog=example',
        'headers' => array('Content-Type' => 'application/x-www-form-urlencoded'),
    ));
    update_option('remote_code', (string) wp_remote_retrieve_response_code($response));
    update_option('remote_body', wp_remote_retrieve_body($response));
});
