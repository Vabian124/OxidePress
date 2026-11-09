// Modified by Vabian124 on 2026-11-09 to draw plugin settings pages and save registered options.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Plugin settings screens.
//!
//! `admin.php?page=` renders the page callback the PHP worker recorded.
//! `options.php` writes only options that plugin registered for the posted group.

use std::collections::HashMap;

use axum::extract::{Extension, Form, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use oxidepress_auth::session::Session;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

use super::wp_admin::{admin_context, render_admin};

#[derive(Deserialize)]
pub struct PluginPageQuery {
    page: Option<String>,
    updated: Option<String>,
}

/// Pages for the admin menu. An empty list means no PHP worker or no pages.
pub fn menu_items(state: &AppState) -> Vec<Value> {
    let Some(runtime) = state.php_runtime.runtime() else {
        return Vec::new();
    };
    match runtime.admin_pages() {
        Ok(pages) => pages
            .into_iter()
            .filter(|page| oxidepress_php::worker_token(&page.slug, 80))
            .take(40)
            .map(|page| {
                json!({
                    "slug": page.slug,
                    "menu_title": page.menu_title,
                    "page_title": page.page_title,
                })
            })
            .collect(),
        Err(err) => {
            tracing::warn!(error = %err, "php admin menus were not listed");
            Vec::new()
        }
    }
}

pub async fn plugin_page(
    State(state): State<std::sync::Arc<AppState>>,
    Extension(session): Extension<Session>,
    Query(query): Query<PluginPageQuery>,
) -> Html<String> {
    let mut ctx = admin_context(&state, &session).await;
    let slug = query.page.unwrap_or_default();
    let known = oxidepress_php::worker_token(&slug, 80);
    if known {
        ctx.insert("active_page", &format!("php-{slug}"));
    }
    ctx.insert("saved", &(query.updated.as_deref() == Some("1")));
    ctx.insert("save_error", &(query.updated.as_deref() == Some("0")));

    if !known {
        ctx.insert("page_title", "Plugin pages");
        ctx.insert("page_html", "");
        return render_admin(&state, "admin/plugin-page.html", &ctx);
    }

    let nonce = state.nonces.create_nonce("php_options", session.user_id);
    match state
        .php_runtime
        .runtime()
        .and_then(|runtime| runtime.render_admin_page(&slug, &nonce).ok())
    {
        Some(page) => {
            ctx.insert("page_title", &page.page_title);
            ctx.insert("page_html", &page.html);
        }
        None => {
            ctx.insert("page_title", "Plugin page");
            ctx.insert("page_html", "");
        }
    }
    render_admin(&state, "admin/plugin-page.html", &ctx)
}

pub async fn plugin_options_save(
    State(state): State<std::sync::Arc<AppState>>,
    Extension(session): Extension<Session>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let nonce = form.get("_wpnonce").map(String::as_str).unwrap_or("");
    if state
        .nonces
        .verify_nonce(nonce, "php_options", session.user_id)
        .is_none()
    {
        return (StatusCode::FORBIDDEN, "Invalid or expired security token").into_response();
    }

    let group = form.get("option_page").cloned().unwrap_or_default();
    let page = form.get("oxidepress_page").cloned().unwrap_or_default();
    let saved_any = state.php_runtime.runtime().and_then(|runtime| {
        runtime
            .save_admin_settings(&group, &form)
            .ok()
            .map(|saved| !saved.is_empty())
    });
    let updated = if saved_any == Some(true) { "1" } else { "0" };
    let target = if oxidepress_php::worker_token(&page, 80) {
        format!("/wp-admin/admin.php?page={page}&updated={updated}")
    } else {
        format!("/wp-admin/options-general.php?updated={updated}")
    };
    Redirect::to(&target).into_response()
}

#[cfg(test)]
mod tests {
    #[test]
    fn page_slugs_stay_inside_the_admin_token_rules() {
        assert!(oxidepress_php::worker_token("queue-settings", 80));
        assert!(!oxidepress_php::worker_token("../options", 80));
        assert!(!oxidepress_php::worker_token("queue settings", 80));
        assert!(!oxidepress_php::worker_token("", 80));
    }
}
