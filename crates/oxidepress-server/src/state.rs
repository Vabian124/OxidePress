// Modified by Vabian124 on 2026-11-09 to keep the PHP worker replaceable after startup.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use std::collections::HashMap;
use std::sync::{Arc, RwLock as SyncRwLock};

use sea_orm::DatabaseConnection;
use tokio::sync::RwLock;

use oxidepress_auth::{JwtManager, LoginAttemptTracker, SessionManager};
use oxidepress_blocks::BlockRenderer;
use oxidepress_cache::{ObjectCache, PageCache, RedisCache, TransientCache};
use oxidepress_commerce::{CartManager, OrderManager, ProductCatalog};
use oxidepress_core::hooks::HookRegistry;
use oxidepress_core::nonce::NonceManager;
use oxidepress_core::post_type::PostTypeRegistry;
use oxidepress_core::rewrite::RewriteRules;
use oxidepress_core::shortcode::ShortcodeRegistry;
use oxidepress_cron::CronManager;
use oxidepress_db::options::OptionsManager;
use oxidepress_fields::{FieldGroupRegistry, FieldStorage};
use oxidepress_forms::SubmissionStore;
use oxidepress_plugins::{PluginRegistry, WasmHost};
use oxidepress_security::{AuditLog, LoginProtection, RateLimiter, WafEngine};
use oxidepress_themes::ThemeEngine;

use crate::i18n::Translations;

/// Application state shared across all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub hooks: HookRegistry,
    pub options: OptionsManager,
    pub jwt: JwtManager,
    pub sessions: SessionManager,
    pub object_cache: ObjectCache,
    pub page_cache: PageCache,
    pub transients: TransientCache,
    pub plugin_registry: PluginRegistry,
    pub site_url: String,
    pub theme_engine: Arc<RwLock<ThemeEngine>>,
    pub admin_tera: Arc<tera::Tera>,
    pub translations: Translations,
    pub nonces: Arc<NonceManager>,
    pub rewrite_rules: Arc<RwLock<RewriteRules>>,
    pub shortcodes: ShortcodeRegistry,
    pub cron: Arc<CronManager>,
    // Integrated subsystems
    pub waf: Arc<RwLock<WafEngine>>,
    pub rate_limiter: Arc<RwLock<RateLimiter>>,
    pub login_protection: Arc<RwLock<LoginProtection>>,
    pub block_renderer: Arc<BlockRenderer>,
    pub redis_cache: Arc<RedisCache>,
    pub field_registry: Arc<RwLock<FieldGroupRegistry>>,
    pub field_storage: Arc<RwLock<FieldStorage>>,
    pub form_submissions: Arc<SubmissionStore>,
    /// Contact form id to (title, template), read by the contact-form-7 shortcode.
    pub contact_forms: Arc<SyncRwLock<HashMap<u64, (String, String)>>>,
    pub product_catalog: Arc<RwLock<ProductCatalog>>,
    pub cart_manager: Arc<RwLock<CartManager>>,
    pub order_manager: Arc<RwLock<OrderManager>>,
    // Login attempt rate limiter (moka-based, per-IP)
    pub login_tracker: LoginAttemptTracker,
    // Security audit log
    pub audit_log: Arc<AuditLog>,
    // WASM plugin host
    pub wasm_host: Arc<RwLock<WasmHost>>,
    /// Long-lived PHP worker for dropped-in plugins. The process is absent until
    /// an active PHP plugin is included or an administrator activates one.
    pub php_runtime: Arc<crate::php_plugins::PhpWorkers>,
    /// Built-in post types plus any a PHP plugin registered on `init`.
    pub post_types: PostTypeRegistry,
    // Multisite support
    pub multisite_resolver: Option<Arc<oxidepress_multisite::SiteResolver>>,
    pub multisite_network: Option<Arc<RwLock<oxidepress_multisite::NetworkManager>>>,
}
