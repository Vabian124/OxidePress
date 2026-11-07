// Modified by Vabian124 on 2026-11-07 to rebrand as OxidePress.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::Router;
use std::sync::Arc;

use oxidepress_admin::AdminState;

use crate::middleware::require_auth_jwt_or_session;
use crate::state::AppState;

/// Create the admin panel routes from the shared application state.
/// All admin CRUD endpoints require JWT or session authentication.
pub fn create_admin_routes(state: &Arc<AppState>) -> Router {
    let admin_state = AdminState {
        db: state.db.clone(),
        hooks: state.hooks.clone(),
        jwt: state.jwt.clone(),
        plugin_registry: state.plugin_registry.clone(),
    };

    oxidepress_admin::routes(admin_state).layer(axum::middleware::from_fn_with_state(
        state.clone(),
        require_auth_jwt_or_session,
    ))
}
