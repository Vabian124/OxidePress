// Modified by Vabian124 on 2026-11-07 to list discovered plugin manifests.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! WordPress REST API — Plugins endpoint.
//!
//! `GET  /wp-json/wp/v2/plugins`         — list installed plugins
//! `GET  /wp-json/wp/v2/plugins/{slug}`  — get single plugin

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use oxidepress_plugins::registry::{PluginEntry, PluginStatus};

use crate::common::WpError;
use crate::ApiState;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/wp-json/wp/v2/plugins", get(list_plugins))
        .route("/wp-json/wp/v2/plugins/{slug}", get(get_plugin))
}

/// List installed plugins.
///
/// WordPress stores active plugins in `active_plugins` option.
async fn list_plugins(State(state): State<ApiState>) -> Json<Vec<Value>> {
    let mut plugins: Vec<Value> = state
        .plugins
        .list()
        .await
        .iter()
        .map(|entry| plugin_from_entry(entry, &state.site_url))
        .collect();
    plugins.sort_by(|left, right| left["plugin"].as_str().cmp(&right["plugin"].as_str()));
    Json(plugins)
}

/// Get a single plugin by slug.
async fn get_plugin(
    State(state): State<ApiState>,
    Path(slug): Path<String>,
) -> Result<Json<Value>, WpError> {
    let plugins = state.plugins.list().await;
    let found = plugins.iter().find(|entry| plugin_slug(entry) == slug);
    match found {
        Some(entry) => Ok(Json(plugin_from_entry(entry, &state.site_url))),
        None => Err(WpError::not_found("Plugin not found")),
    }
}

fn plugin_slug(entry: &PluginEntry) -> String {
    std::path::Path::new(&entry.file_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(entry.meta.name.as_str())
        .to_string()
}

fn plugin_from_entry(entry: &PluginEntry, site_url: &str) -> Value {
    plugin_json(
        &plugin_slug(entry),
        &entry.meta.name,
        &entry.meta.version,
        &entry.meta.description,
        entry.status == PluginStatus::Active,
        &entry.meta.author,
        site_url,
    )
}

fn plugin_json(
    slug: &str,
    name: &str,
    version: &str,
    description: &str,
    active: bool,
    author: &str,
    site_url: &str,
) -> Value {
    let base = site_url.trim_end_matches('/');
    let status = if active { "active" } else { "inactive" };
    json!({
        "plugin": format!("{}/{}.php", slug, slug),
        "status": status,
        "name": name,
        "plugin_uri": "",
        "author": author,
        "author_uri": base,
        "description": {"raw": description, "rendered": description},
        "version": version,
        "network_only": false,
        "requires_wp": "",
        "requires_php": "",
        "textdomain": slug,
        "_links": {
            "self": [{"href": format!("{}/wp-json/wp/v2/plugins/{}", base, slug)}],
            "collection": [{"href": format!("{}/wp-json/wp/v2/plugins", base)}],
            "curies": [{"name": "wp", "href": "https://api.w.org/{rel}", "templated": true}]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidepress_plugins::registry::{PluginMeta, PluginType};

    #[test]
    fn discovered_manifest_reports_its_status() {
        let entry = PluginEntry {
            meta: PluginMeta {
                name: "Hello Oxide".into(),
                version: "1.0.0".into(),
                description: "Welcome note".into(),
                author: "OxidePress".into(),
                plugin_type: PluginType::Native,
                basename: String::new(),
            },
            status: PluginStatus::Inactive,
            file_path: "demo-plugins/hello-oxide".into(),
        };
        let row = plugin_from_entry(&entry, "http://127.0.0.1:3000");
        assert_eq!(row["plugin"], "hello-oxide/hello-oxide.php");
        assert_eq!(row["status"], "inactive");
        assert_eq!(row["name"], "Hello Oxide");
        assert_eq!(row["version"], "1.0.0");
    }
}
