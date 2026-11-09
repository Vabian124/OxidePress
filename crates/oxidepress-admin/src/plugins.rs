// Modified by Vabian124 on 2026-11-09 to boot a plugin when an administrator activates it.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Serialize;

use crate::{AdminState, PluginChange};

#[derive(Debug, Serialize)]
pub struct PluginActionResponse {
    pub ok: bool,
    pub name: String,
    pub status: String,
    pub message: String,
}

const PLUGIN_ACTION_ROUTE: &str = "/admin/plugins/{*path}";

pub fn routes() -> Router<AdminState> {
    Router::new().route(PLUGIN_ACTION_ROUTE, post(plugin_action_route))
}

async fn plugin_action_route(
    State(state): State<AdminState>,
    Path(path): Path<String>,
) -> Result<Json<PluginActionResponse>, (StatusCode, String)> {
    let (name, action) = plugin_action(&path).map_err(|error| (StatusCode::NOT_FOUND, error))?;
    let (status, verb, change) = match action {
        PluginVerb::Activate => {
            state
                .plugin_registry
                .activate(&name)
                .await
                .map_err(|error| (StatusCode::NOT_FOUND, error))?;
            ("Active", "activated", PluginChange::Activated)
        }
        PluginVerb::Deactivate => {
            state
                .plugin_registry
                .deactivate(&name)
                .await
                .map_err(|error| (StatusCode::NOT_FOUND, error))?;
            ("Inactive", "deactivated", PluginChange::Deactivated)
        }
    };
    if let Some(hook) = &state.plugin_lifecycle {
        if let Some(entry) = plugin_entry(&state, &name).await {
            if let Err(error) = hook(entry, change) {
                let _ = state.plugin_registry.mark_error(&name, error.clone()).await;
                return Err((StatusCode::INTERNAL_SERVER_ERROR, error));
            }
        }
    }
    persist_active_plugins(&state).await;

    Ok(Json(PluginActionResponse {
        ok: true,
        name: name.clone(),
        status: status.to_string(),
        message: format!("Plugin '{name}' {verb} successfully."),
    }))
}

enum PluginVerb {
    Activate,
    Deactivate,
}

fn plugin_action(path: &str) -> Result<(String, PluginVerb), String> {
    let path = path.trim_matches('/');
    let Some((name, action)) = path.rsplit_once('/') else {
        return Err(format!("plugin action is missing from '{path}'"));
    };
    if name.is_empty() {
        return Err("plugin name is empty".to_string());
    }
    let action = match action {
        "activate" => PluginVerb::Activate,
        "deactivate" => PluginVerb::Deactivate,
        other => return Err(format!("unknown plugin action '{other}'")),
    };
    Ok((name.to_string(), action))
}

async fn plugin_entry(
    state: &AdminState,
    name: &str,
) -> Option<oxidepress_plugins::registry::PluginEntry> {
    state
        .plugin_registry
        .list()
        .await
        .into_iter()
        .find(|entry| {
            entry.meta.name == name
                || entry.meta.basename == name
                || stored_plugin_id(entry) == name
        })
}

fn stored_plugin_id(entry: &oxidepress_plugins::registry::PluginEntry) -> String {
    if entry.meta.basename.is_empty() {
        entry.meta.name.clone()
    } else {
        entry.meta.basename.clone()
    }
}

fn serialize_active_plugins(entries: &[oxidepress_plugins::registry::PluginEntry]) -> String {
    let mut entries: Vec<_> = entries.iter().collect();
    entries.sort_by_key(|entry| stored_plugin_id(entry));
    let pairs = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                oxidepress_core::PhpValue::Int(index as i64),
                oxidepress_core::PhpValue::String(stored_plugin_id(entry)),
            )
        })
        .collect();
    oxidepress_core::php_serialize(&oxidepress_core::PhpValue::Array(pairs))
}

async fn persist_active_plugins(state: &AdminState) {
    let active = state.plugin_registry.active_plugins().await;
    let serialized = serialize_active_plugins(&active);
    let options = oxidepress_db::options::OptionsManager::new(state.db.clone());
    if let Err(error) = options.update_option("active_plugins", &serialized).await {
        tracing::warn!(%error, "active plugins were not stored");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use oxidepress_plugins::registry::{PluginEntry, PluginMeta, PluginStatus, PluginType};
    use tower::ServiceExt;

    fn entry(name: &str, basename: &str) -> PluginEntry {
        PluginEntry {
            meta: PluginMeta {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: String::new(),
                author: String::new(),
                plugin_type: if basename.is_empty() {
                    PluginType::Native
                } else {
                    PluginType::Php
                },
                basename: basename.to_string(),
            },
            status: PluginStatus::Active,
            file_path: String::new(),
        }
    }

    #[test]
    fn basename_with_a_slash_is_the_stored_id() {
        let php = entry("Hello Dolly", "hello-dolly/hello.php");
        let native = entry("hello-oxide", "");
        assert_eq!(stored_plugin_id(&php), "hello-dolly/hello.php");
        assert_eq!(stored_plugin_id(&native), "hello-oxide");
        assert_eq!(
            serialize_active_plugins(&[php, native]),
            "a:2:{i:0;s:21:\"hello-dolly/hello.php\";i:1;s:11:\"hello-oxide\";}"
        );
    }

    #[test]
    fn action_path_keeps_the_slash_in_the_basename() {
        let (name, action) = plugin_action("hello-dolly/hello.php/activate").unwrap();
        assert_eq!(name, "hello-dolly/hello.php");
        assert!(matches!(action, PluginVerb::Activate));

        let (name, action) = plugin_action("hello-oxide/deactivate").unwrap();
        assert_eq!(name, "hello-oxide");
        assert!(matches!(action, PluginVerb::Deactivate));

        assert!(plugin_action("activate").is_err());
        assert!(plugin_action("hello-dolly/hello.php/reload").is_err());
    }

    #[tokio::test]
    async fn route_accepts_a_basename_that_contains_a_slash() {
        let app = Router::new().route(
            PLUGIN_ACTION_ROUTE,
            post(|Path(path): Path<String>| async move { plugin_action(&path).unwrap().0 }),
        );

        for uri in [
            "/admin/plugins/hello-dolly/hello.php/activate",
            "/admin/plugins/hello-dolly%2Fhello.php/activate",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            assert_eq!(body.as_ref(), b"hello-dolly/hello.php", "{uri}");
        }
    }
}
