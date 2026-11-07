// Modified by Vabian124 on 2026-11-07 to use a random JWT secret when none is configured.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: String,
    pub site_url: String,
    pub templates_dir: String,
    pub themes_dir: String,
    pub uploads_dir: String,
    pub plugins_dir: String,
    pub static_dir: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("RUSTPRESS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("RUSTPRESS_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                eprintln!(
                    "WARNING: DATABASE_URL not set. Using default local credentials. \
                     Set DATABASE_URL env var for production."
                );
                "mysql://root:password@localhost:3306/wordpress".to_string()
            }),
            jwt_secret: jwt_secret_from_env_value(std::env::var("JWT_SECRET").ok().as_deref()),
            site_url: std::env::var("SITE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
            templates_dir: std::env::var("TEMPLATES_DIR")
                .unwrap_or_else(|_| "templates".to_string()),
            themes_dir: std::env::var("THEMES_DIR").unwrap_or_else(|_| "themes".to_string()),
            uploads_dir: std::env::var("UPLOADS_DIR")
                .unwrap_or_else(|_| "wp-content/uploads".to_string()),
            plugins_dir: std::env::var("PLUGINS_DIR")
                .unwrap_or_else(|_| "wp-content/plugins".to_string()),
            static_dir: std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string()),
        }
    }
}

/// Use the configured secret when it is at least 32 characters.
/// Otherwise generate a 256-bit secret and do not log it.
pub(crate) fn jwt_secret_from_env_value(value: Option<&str>) -> String {
    if let Some(secret) = value {
        if secret.len() >= 32 {
            return secret.to_string();
        }
        tracing::warn!(
            "JWT_SECRET is shorter than 32 characters. A per-process random secret will be used instead."
        );
    } else {
        tracing::warn!(
            "JWT_SECRET is not set. A per-process random secret will be used. Tokens will not survive restarts."
        );
    }
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[cfg(test)]
mod tests {
    use super::jwt_secret_from_env_value;

    #[test]
    fn test_missing_jwt_secret_is_random_and_long_enough() {
        let first = jwt_secret_from_env_value(None);
        let second = jwt_secret_from_env_value(None);
        assert!(first.len() >= 32);
        assert!(second.len() >= 32);
        assert_ne!(first, second);
    }

    #[test]
    fn test_short_jwt_secret_is_not_kept() {
        let secret = jwt_secret_from_env_value(Some("short-secret"));
        assert!(secret.len() >= 32);
        assert_ne!(secret, "short-secret");
    }

    #[test]
    fn test_configured_jwt_secret_is_kept() {
        let configured = "this-is-a-configured-secret-32ch";
        assert_eq!(jwt_secret_from_env_value(Some(configured)), configured);
    }
}
