// Modified by Vabian124 on 2026-11-07 to store which plugins are active.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::Serialize;

use crate::AdminState;

#[derive(Debug, Serialize)]
pub struct PluginActionResponse {
    pub ok: bool,
    pub name: String,
    pub status: String,
    pub message: String,
}

pub fn routes() -> Router<AdminState> {
    Router::new()
        .route("/admin/plugins/{name}/activate", post(activate_plugin))
        .route("/admin/plugins/{name}/deactivate", post(deactivate_plugin))
}

async fn activate_plugin(
    State(state): State<AdminState>,
    Path(name): Path<String>,
) -> Result<Json<PluginActionResponse>, (StatusCode, String)> {
    state
        .plugin_registry
        .activate(&name)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    persist_active_plugins(&state).await;

    Ok(Json(PluginActionResponse {
        ok: true,
        name: name.clone(),
        status: "Active".to_string(),
        message: format!("Plugin '{name}' activated successfully."),
    }))
}

async fn deactivate_plugin(
    State(state): State<AdminState>,
    Path(name): Path<String>,
) -> Result<Json<PluginActionResponse>, (StatusCode, String)> {
    state
        .plugin_registry
        .deactivate(&name)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e))?;
    persist_active_plugins(&state).await;

    Ok(Json(PluginActionResponse {
        ok: true,
        name: name.clone(),
        status: "Inactive".to_string(),
        message: format!("Plugin '{name}' deactivated successfully."),
    }))
}

async fn persist_active_plugins(state: &AdminState) {
    let active = state.plugin_registry.active_plugins().await;
    let pairs = active
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            (
                oxidepress_core::PhpValue::Int(index as i64),
                oxidepress_core::PhpValue::String(entry.meta.name.clone()),
            )
        })
        .collect();
    let serialized = oxidepress_core::php_serialize(&oxidepress_core::PhpValue::Array(pairs));
    let options = oxidepress_db::options::OptionsManager::new(state.db.clone());
    if let Err(error) = options.update_option("active_plugins", &serialized).await {
        tracing::warn!(%error, "active plugins were not stored");
    }
}
