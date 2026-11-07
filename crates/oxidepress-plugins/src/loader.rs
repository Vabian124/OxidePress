// Modified by Vabian124 on 2026-11-07 to discover dropped-in PHP plugins.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::registry::{PluginEntry, PluginMeta, PluginRegistry, PluginStatus, PluginType};

/// Plugin loader - scans directories for plugins and loads them.
pub struct PluginLoader {
    plugins_dir: PathBuf,
}

impl PluginLoader {
    pub fn new(plugins_dir: impl Into<PathBuf>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
        }
    }

    /// Scan the plugins directory and register discovered plugins.
    pub async fn scan_and_register(&self, registry: &PluginRegistry) -> Result<usize, String> {
        if !self.plugins_dir.exists() {
            info!(dir = ?self.plugins_dir, "plugins directory does not exist, creating");
            std::fs::create_dir_all(&self.plugins_dir)
                .map_err(|e| format!("Failed to create plugins dir: {e}"))?;
            return Ok(0);
        }

        let mut count = 0;
        let entries = std::fs::read_dir(&self.plugins_dir)
            .map_err(|e| format!("Failed to read plugins dir: {e}"))?;

        for entry in entries.flatten() {
            let path = entry.path();

            if let Some(plugin_entry) = self.try_load_plugin(&path).await {
                registry.register(plugin_entry).await;
                count += 1;
            }
        }

        count += register_php_plugins(&self.plugins_dir, registry).await;

        info!(count, "plugins scanned and registered");
        Ok(count)
    }

    /// Try to load a plugin from a path.
    async fn try_load_plugin(&self, path: &Path) -> Option<PluginEntry> {
        // Check for WASM plugins (.wasm files)
        if path.extension().map(|e| e == "wasm").unwrap_or(false) {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            info!(name = &name, path = ?path, "discovered WASM plugin");

            return Some(PluginEntry {
                meta: PluginMeta {
                    name,
                    version: "0.1.0".to_string(),
                    description: "WASM Plugin".to_string(),
                    author: "Unknown".to_string(),
                    plugin_type: PluginType::Wasm,
                    basename: String::new(),
                },
                status: PluginStatus::Inactive,
                file_path: path.to_string_lossy().to_string(),
            });
        }

        // Check for plugin directories with a plugin.json manifest
        if path.is_dir() {
            let manifest_path = path.join("plugin.json");
            if manifest_path.exists() {
                match std::fs::read_to_string(&manifest_path) {
                    Ok(content) => match serde_json::from_str::<PluginMeta>(&content) {
                        Ok(meta) => {
                            info!(name = &meta.name, "discovered plugin from manifest");
                            return Some(PluginEntry {
                                meta,
                                status: PluginStatus::Inactive,
                                file_path: path.to_string_lossy().to_string(),
                            });
                        }
                        Err(e) => {
                            warn!(path = ?manifest_path, error = %e, "failed to parse plugin manifest");
                        }
                    },
                    Err(e) => {
                        warn!(path = ?manifest_path, error = %e, "failed to read plugin manifest");
                    }
                }
            }
        }

        None
    }
}

/// Register PHP plugins found by a WordPress-style header scan.
async fn register_php_plugins(plugins_dir: &Path, registry: &PluginRegistry) -> usize {
    let found = match oxidepress_php::discover(plugins_dir) {
        Ok(found) => found,
        Err(error) => {
            warn!(error = %error, "PHP plugin discovery failed");
            return 0;
        }
    };

    let mut count = 0;
    for plugin in found {
        if manifest_owns(&plugin.file_path) {
            continue;
        }
        info!(
            name = %plugin.name,
            basename = %plugin.basename,
            "discovered PHP plugin"
        );
        registry
            .register(PluginEntry {
                meta: PluginMeta {
                    name: plugin.name,
                    version: plugin.version,
                    description: plugin.description,
                    author: plugin.author,
                    plugin_type: PluginType::Php,
                    basename: plugin.basename,
                },
                status: PluginStatus::Inactive,
                file_path: plugin.file_path.to_string_lossy().to_string(),
            })
            .await;
        count += 1;
    }
    count
}

fn manifest_owns(path: &Path) -> bool {
    path.parent()
        .is_some_and(|parent| parent.join("plugin.json").is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn discovers_php_plugin_header_and_keeps_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let plugin_dir = root.join("hello-dolly");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("hello.php"),
            "<?php\n/*\nPlugin Name: Hello Dolly\nVersion: 1.7.2\nDescription: Lyric\nAuthor: Matt\n*/\n",
        )
        .unwrap();
        std::fs::write(root.join("not-a-plugin.php"), "<?php\n// no header\n").unwrap();

        let manifest_dir = root.join("native-demo");
        std::fs::create_dir_all(&manifest_dir).unwrap();
        std::fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"name":"Native Demo","version":"1.0.0","description":"Manifest","author":"Oxide","plugin_type":"Native"}"#,
        )
        .unwrap();
        std::fs::write(
            manifest_dir.join("native-demo.php"),
            "<?php\n/*\nPlugin Name: Should Stay With Manifest\n*/\n",
        )
        .unwrap();

        let registry = PluginRegistry::new();
        let count = PluginLoader::new(root)
            .scan_and_register(&registry)
            .await
            .unwrap();
        assert_eq!(count, 2);

        registry.activate("hello-dolly/hello.php").await.unwrap();
        assert!(registry.is_active("Hello Dolly").await);

        let plugins = registry.list().await;
        let php = plugins
            .iter()
            .find(|entry| entry.meta.plugin_type == PluginType::Php)
            .unwrap();
        assert_eq!(php.meta.name, "Hello Dolly");
        assert_eq!(php.meta.basename, "hello-dolly/hello.php");
        assert!(plugins.iter().any(|entry| entry.meta.name == "Native Demo"));
        assert!(!plugins
            .iter()
            .any(|entry| entry.meta.name == "Should Stay With Manifest"));
    }
}
