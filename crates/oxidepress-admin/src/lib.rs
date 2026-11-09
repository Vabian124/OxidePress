// Modified by Vabian124 on 2026-11-09 to run a hook when a plugin is activated.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
pub mod comments;
pub mod media;
pub mod plugins;
pub mod posts;
pub mod taxonomies;
pub mod users;

use axum::Router;
use sea_orm::DatabaseConnection;

use oxidepress_auth::JwtManager;
use oxidepress_core::hooks::HookRegistry;
use oxidepress_plugins::registry::PluginEntry;
use oxidepress_plugins::PluginRegistry;

/// Whether an administrator turned a plugin on or off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginChange {
    Activated,
    Deactivated,
}

/// Runs after the registry records a plugin change. `Err` marks that plugin as failed.
pub type PluginLifecycleHook =
    std::sync::Arc<dyn Fn(PluginEntry, PluginChange) -> Result<(), String> + Send + Sync>;

/// Shared state for admin routes.
#[derive(Clone)]
pub struct AdminState {
    pub db: DatabaseConnection,
    pub hooks: HookRegistry,
    pub jwt: JwtManager,
    pub plugin_registry: PluginRegistry,
    /// Optional. The server uses this to boot a PHP plugin when it is activated.
    pub plugin_lifecycle: Option<PluginLifecycleHook>,
}

/// Create the admin API router with all CRUD endpoints.
pub fn routes(state: AdminState) -> Router {
    Router::new()
        .merge(posts::routes())
        .merge(users::routes())
        .merge(media::routes())
        .merge(comments::routes())
        .merge(taxonomies::routes())
        .merge(plugins::routes())
        .with_state(state)
}
