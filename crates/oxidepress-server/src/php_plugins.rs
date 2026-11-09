// Modified by Vabian124 on 2026-11-09 to drop one plugin's hooks without a restart.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! The long-lived PHP worker and the plugins it has included.
//!
//! Plugins that are already active are included at boot. Their activation
//! callbacks are not run again. An administrator activating a PHP plugin
//! includes it, runs `register_activation_hook`, then runs that plugin's `init`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use oxidepress_core::hooks::HookRegistry;
use oxidepress_core::post_type::{PostTypeArgs, PostTypeLabels, PostTypeRegistry};
use oxidepress_core::shortcode::ShortcodeRegistry;
use oxidepress_php::{DiscoveredPlugin, LoadedPlugin, PhpHost, PhpRuntime};
use oxidepress_plugins::registry::{PluginEntry, PluginType};

/// Owns the PHP process and remembers which basenames it has included.
pub struct PhpWorkers {
    runtime: Mutex<Option<PhpRuntime>>,
    loaded: Mutex<HashMap<String, IncludedPlugin>>,
    plugins_dir: PathBuf,
    site_url: String,
    host: Arc<dyn PhpHost>,
}

impl PhpWorkers {
    pub fn new(
        plugins_dir: impl Into<PathBuf>,
        site_url: impl Into<String>,
        host: Arc<dyn PhpHost>,
    ) -> Self {
        Self {
            runtime: Mutex::new(None),
            loaded: Mutex::new(HashMap::new()),
            plugins_dir: plugins_dir.into(),
            site_url: site_url.into(),
            host,
        }
    }

    /// The running worker, when `php` started and at least one plugin was included
    /// or an administrator has since activated one.
    pub fn runtime(&self) -> Option<PhpRuntime> {
        self.runtime.lock().ok().and_then(|slot| slot.clone())
    }

    /// Include every active PHP plugin. Activation hooks stay unrun.
    pub async fn boot(
        &self,
        registry: &oxidepress_plugins::PluginRegistry,
        hooks: &HookRegistry,
        shortcodes: &ShortcodeRegistry,
    ) {
        let active: Vec<_> = registry
            .active_plugins()
            .await
            .into_iter()
            .filter(|entry| entry.meta.plugin_type == PluginType::Php)
            .collect();
        if active.is_empty() {
            return;
        }
        let Some(runtime) = self.ensure() else {
            for entry in &active {
                self.mark_failed(registry, entry, "PHP plugin worker did not start")
                    .await;
            }
            return;
        };
        for entry in active {
            if let Err(error) = self.include(&runtime, &entry, hooks, shortcodes, false) {
                tracing::warn!(
                    plugin = %entry.meta.basename,
                    %error,
                    "PHP plugin failed to load"
                );
                self.mark_failed(registry, &entry, &error).await;
            }
        }
    }

    /// Include a PHP plugin an administrator just activated, then run its activation hook.
    pub fn activate(
        &self,
        entry: &PluginEntry,
        hooks: &HookRegistry,
        shortcodes: &ShortcodeRegistry,
        post_types: &PostTypeRegistry,
    ) -> Result<(), String> {
        if entry.meta.plugin_type != PluginType::Php {
            return Ok(());
        }
        let runtime = self
            .ensure()
            .ok_or_else(|| "PHP plugin worker did not start".to_string())?;
        self.include(&runtime, entry, hooks, shortcodes, true)?;
        self.adopt_post_types(&runtime, post_types);
        Ok(())
    }

    pub fn adopt_post_types(&self, runtime: &PhpRuntime, post_types: &PostTypeRegistry) {
        let types = match runtime.registered_post_types() {
            Ok(types) => types,
            Err(error) => {
                tracing::warn!(%error, "PHP post types were not loaded");
                return;
            }
        };
        for kind in types {
            if post_types
                .get(&kind.name)
                .is_some_and(|existing| existing.builtin)
            {
                continue;
            }
            post_types.register(
                &kind.name,
                PostTypeArgs {
                    label: kind.label.clone(),
                    labels: PostTypeLabels {
                        name: kind.label,
                        singular_name: kind.singular,
                        ..Default::default()
                    },
                    public: kind.public,
                    show_ui: kind.show_ui,
                    show_in_menu: kind.show_in_menu,
                    show_in_rest: kind.show_ui,
                    ..Default::default()
                },
                false,
            );
        }
    }

    fn ensure(&self) -> Option<PhpRuntime> {
        let mut slot = self.runtime.lock().ok()?;
        if slot.is_none() {
            let php = std::env::var_os("OXIDEPRESS_PHP")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("php"));
            match PhpRuntime::spawn_php(
                php,
                Some(Path::new(&self.plugins_dir)),
                Some(&self.site_url),
                self.host.clone(),
            ) {
                Ok(runtime) => *slot = Some(runtime),
                Err(error) => {
                    tracing::warn!(%error, "PHP plugin worker did not start");
                    return None;
                }
            }
        }
        slot.clone()
    }

    fn include(
        &self,
        runtime: &PhpRuntime,
        entry: &PluginEntry,
        hooks: &HookRegistry,
        shortcodes: &ShortcodeRegistry,
        run_activation: bool,
    ) -> Result<(), String> {
        if entry.file_path.is_empty() {
            return Err("PHP plugin has no file".to_string());
        }
        let key = plugin_key(entry);
        let loaded = self
            .loaded
            .lock()
            .map_err(|_| "plugin lock poisoned".to_string())?;
        if let Some(existing) = loaded.get(&key) {
            if run_activation && !existing.live.load(Ordering::Relaxed) {
                existing.live.store(true, Ordering::Relaxed);
                let plugin = existing.plugin.clone();
                drop(loaded);
                return self.finish_activation(runtime, &plugin, &key);
            }
            return Ok(());
        }
        drop(loaded);
        let discovered = DiscoveredPlugin {
            name: entry.meta.name.clone(),
            version: entry.meta.version.clone(),
            description: entry.meta.description.clone(),
            author: entry.meta.author.clone(),
            file_path: PathBuf::from(&entry.file_path),
            basename: entry.meta.basename.clone(),
        };
        let plugin = runtime
            .load(&discovered)
            .map_err(|error| error.to_string())?;
        let live = Arc::new(AtomicBool::new(true));
        runtime.attach_while(hooks, shortcodes, &plugin, live.clone());
        self.loaded
            .lock()
            .map_err(|_| "plugin lock poisoned".to_string())?
            .insert(
                key.clone(),
                IncludedPlugin {
                    plugin: plugin.clone(),
                    live,
                },
            );
        if run_activation {
            self.finish_activation(runtime, &plugin, &key)?;
        }
        Ok(())
    }

    /// Run the deactivation hook, then stop this plugin's callbacks.
    ///
    /// The worker process stays up, so the other plugins keep running.
    pub fn deactivate(&self, entry: &PluginEntry) -> Result<(), String> {
        if entry.meta.plugin_type != PluginType::Php {
            return Ok(());
        }
        let Some(runtime) = self.runtime() else {
            return Ok(());
        };
        let key = plugin_key(entry);
        let included = self
            .loaded
            .lock()
            .map_err(|_| "plugin lock poisoned".to_string())?
            .get(&key)
            .map(|item| (item.plugin.clone(), item.live.clone()));
        let Some((plugin, live)) = included else {
            return Ok(());
        };
        runtime
            .run_deactivation(&plugin)
            .map_err(|error| error.to_string())?;
        live.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn finish_activation(
        &self,
        runtime: &PhpRuntime,
        plugin: &LoadedPlugin,
        key: &str,
    ) -> Result<(), String> {
        runtime
            .run_activation(plugin)
            .map_err(|error| error.to_string())?;
        if let Err(error) = runtime.run_tag(plugin, "init") {
            tracing::warn!(plugin = %key, %error, "plugin init did not finish");
        }
        Ok(())
    }

    async fn mark_failed(
        &self,
        registry: &oxidepress_plugins::PluginRegistry,
        entry: &PluginEntry,
        error: &str,
    ) {
        let name = plugin_key(entry);
        if let Err(mark) = registry.mark_error(&name, error.to_string()).await {
            tracing::warn!(error = %mark, "failed to record PHP plugin error");
        }
    }
}

fn plugin_key(entry: &PluginEntry) -> String {
    if entry.meta.basename.is_empty() {
        entry.meta.name.clone()
    } else {
        entry.meta.basename.clone()
    }
}

struct IncludedPlugin {
    plugin: LoadedPlugin,
    live: Arc<AtomicBool>,
}
