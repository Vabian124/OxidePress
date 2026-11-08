// Modified by Vabian124 on 2026-11-09 to render the custom post editor template.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use std::path::Path;

use oxidepress_themes::ThemeEngine;

/// Initialize the template/theme engine.
///
/// If an active theme exists under `themes_dir`, uses that theme's templates.
/// Otherwise falls back to the legacy `templates/` directory.
pub fn init_theme_engine(
    themes_dir: &str,
    templates_dir: &str,
    active_theme: &str,
) -> Result<ThemeEngine, String> {
    let themes_path = Path::new(themes_dir);

    // Try themes/{active_theme}/templates/ first
    if active_theme != "default" {
        let theme_templates = themes_path.join(active_theme).join("templates");
        if theme_templates.exists() {
            tracing::info!(theme = active_theme, "loading theme from themes directory");
            return ThemeEngine::from_templates_dir(&theme_templates).map_err(|e| e.to_string());
        }

        // Also try themes/{active_theme}/ directly (templates at root)
        let theme_dir = themes_path.join(active_theme);
        if theme_dir.exists() && theme_dir.join("base.html").exists() {
            tracing::info!(
                theme = active_theme,
                "loading theme from themes directory (flat layout)"
            );
            return ThemeEngine::from_templates_dir(&theme_dir).map_err(|e| e.to_string());
        }

        tracing::warn!(
            theme = active_theme,
            "theme not found in themes dir, falling back to templates/"
        );
    }

    // Fallback to legacy templates/ directory
    let path = Path::new(templates_dir);
    if !path.exists() {
        return Err(format!("Templates directory not found: {templates_dir}"));
    }

    ThemeEngine::from_templates_dir(path).map_err(|e| e.to_string())
}

/// Initialize the admin template engine (separate from the frontend theme).
pub fn init_admin_tera(admin_templates_dir: &str) -> Result<tera::Tera, String> {
    let path = Path::new(admin_templates_dir);

    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create admin templates dir: {e}"))?;
    }

    // Load from the parent "templates/" directory so that template names
    // retain the "admin/" prefix and {% extends "admin/base.html" %} resolves.
    let parent = path.parent().unwrap_or(Path::new("templates"));
    let glob = format!("{}/**/*.html", parent.display());
    tera::Tera::new(&glob).map_err(|e| e.to_string())
}

/// Resolve the static directory for a theme.
///
/// If the active theme has a `static/` directory under `themes/`, returns that path.
/// Otherwise returns the default `static/` directory.
pub fn resolve_theme_static_dir(
    themes_dir: &str,
    active_theme: &str,
    default_static: &str,
) -> String {
    if active_theme != "default" {
        let theme_static = Path::new(themes_dir).join(active_theme).join("static");
        if theme_static.exists() {
            return theme_static.to_string_lossy().to_string();
        }
    }
    default_static.to_string()
}

/// Resolve the theme.json path for the active theme.
///
/// Checks (in order): themes/{active}/theme.json, themes/{active}/static/theme.json,
/// then falls back to the default static dir.
pub fn resolve_theme_json_path(
    themes_dir: &str,
    active_theme: &str,
    default_static: &str,
) -> std::path::PathBuf {
    if active_theme != "default" {
        let base = Path::new(themes_dir).join(active_theme);
        // theme.json at theme root (standard for block themes)
        let root_json = base.join("theme.json");
        if root_json.exists() {
            return root_json;
        }
        // theme.json in static/ subdir
        let static_json = base.join("static").join("theme.json");
        if static_json.exists() {
            return static_json;
        }
    }
    Path::new(default_static).join("theme.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn custom_post_editor_lists_the_type_and_renders_the_form() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("templates")
            .join("admin");
        let mut tera = init_admin_tera(dir.to_str().expect("utf8 path")).expect("admin templates");
        tera.register_function(
            "__",
            |args: &HashMap<String, tera::Value>| -> tera::Result<tera::Value> {
                Ok(args
                    .get("key")
                    .cloned()
                    .unwrap_or_else(|| tera::Value::String(String::new())))
            },
        );
        let mut ctx = tera::Context::new();
        ctx.insert("type_label", "Events");
        ctx.insert("editing", &false);
        ctx.insert("wpnonce_save_post", "nonce");
        ctx.insert("post_id", &0u64);
        ctx.insert("post_type", "event");
        ctx.insert("post_title", "Launch");
        ctx.insert("post_content", "<p>body</p>");
        ctx.insert("post_excerpt", "");
        ctx.insert("post_slug", "launch");
        ctx.insert("post_status", "draft");
        ctx.insert("active_page", "type-event");
        ctx.insert(
            "custom_post_types",
            &serde_json::json!([{ "name": "event", "label": "Events" }]),
        );
        let html = tera
            .render("admin/custom-post-edit.html", &ctx)
            .expect("render custom post editor");
        assert!(html.contains("name=\"post_type\" value=\"event\""));
        assert!(html.contains("edit.php?post_type=event"));
        assert!(html.contains("class=\"menu-item active\""));
        assert!(html.contains("Launch"));
        assert!(
            html.contains("&lt;p&gt;body&lt;/p&gt;")
                || html.contains("&lt;p&gt;body&lt;&#x2F;p&gt;"),
            "content was not escaped"
        );
        assert!(html.contains("value=\"draft\" selected") || html.contains("selected"));
    }
}
