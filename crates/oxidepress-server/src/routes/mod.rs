// Modified by Vabian124 on 2026-11-09 to draw plugin settings pages and save registered options.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::Router;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

mod auth;
pub mod commerce;
pub mod forms;
pub(crate) mod frontend;
pub mod health;
pub mod php_pages;
pub mod plugin_admin;
mod posts;
pub mod seo;
mod users;
pub mod wasm_plugins;
pub mod wc_api;
pub mod wp_admin;
#[allow(dead_code)]
pub mod xmlrpc;
pub(crate) mod yoast_store;

use crate::state::AppState;

pub fn create_router(state: Arc<AppState>) -> Router {
    // Admin dashboard HTML routes (must be before frontend catch-all)
    let admin_html = wp_admin::routes(state.clone());

    // XML-RPC endpoint (must be before frontend catch-all)
    let xmlrpc_router = Router::new()
        .merge(xmlrpc::routes())
        .with_state(state.clone());

    // Core router with API, health, auth
    let api_router = Router::new()
        .merge(health::routes())
        .merge(posts::routes())
        .merge(users::routes())
        .merge(auth::routes())
        .with_state(state.clone());

    // Commerce routes (/shop/*, /cart/*, /checkout/*)
    let commerce_router = commerce::routes(state.clone());

    // Form submission routes (/forms/*)
    let forms_router = forms::routes(state.clone());

    // SEO routes (sitemap.xml, robots.txt)
    let seo_router = seo::routes(state.clone());

    // Plugin admin pages (SEO, ACF, CF7, Security)
    let plugin_admin_router = plugin_admin::routes(state.clone());

    // WooCommerce REST API v3 routes
    let wc_router = wc_api::routes(state.clone());

    // WASM plugin API routes
    let wasm_router = Router::new()
        .merge(wasm_plugins::routes())
        .with_state(state.clone());

    // Frontend routes (includes /{slug} catch-all, must come last)
    let frontend_router = Router::new().merge(frontend::routes()).with_state(state);

    api_router
        .merge(admin_html)
        .merge(plugin_admin_router)
        .merge(xmlrpc_router)
        .merge(commerce_router)
        .merge(forms_router)
        .merge(seo_router)
        .merge(wc_router)
        .merge(wasm_router)
        .merge(frontend_router)
        .layer(TraceLayer::new_for_http())
}
