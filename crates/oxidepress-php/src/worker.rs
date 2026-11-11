// Modified by Vabian124 on 2026-11-11 to store and assign terms from a plugin.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! PHP process supervisor.
//!
//! One worker includes every active plugin. Rust registers one closure per
//! PHP callback so hook priority stays on [`oxidepress_core::hooks::HookRegistry`].
//! Deactivating a plugin clears the flag on its callbacks. The other plugins stay attached.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::discover::DiscoveredPlugin;
use crate::protocol::{read_frame, write_frame};
use crate::PhpError;

/// Values the PHP worker can read and write while a plugin is running.
///
/// Methods run while the worker pipe is locked. They must not call back into
/// [`PhpRuntime`].
pub trait PhpHost: Send + Sync {
    fn get_option(&self, name: &str) -> Option<String>;
    fn update_option(&self, name: &str, value: &str) -> bool;
    fn delete_option(&self, name: &str) -> bool;

    /// Read one post. The default has no post store.
    fn get_post(&self, id: u64) -> Option<crate::posts::PostRecord> {
        let _ = id;
        None
    }

    /// List posts for `get_posts`. The default returns nothing.
    fn query_posts(&self, query: &crate::posts::PostQuery) -> Vec<crate::posts::PostRecord> {
        let _ = query;
        Vec::new()
    }

    /// Insert one post. The default has no database.
    fn insert_post(&self, draft: &crate::posts::PostDraft) -> Result<u64, String> {
        let _ = draft;
        Err("posts are not available".to_string())
    }

    /// Replace the editable fields of one post.
    fn update_post(&self, id: u64, draft: &crate::posts::PostDraft) -> Result<(), String> {
        let _ = (id, draft);
        Err("posts are not available".to_string())
    }

    /// Trash a post, or delete the row when `force` is set.
    fn delete_post(&self, id: u64, force: bool) -> Result<bool, String> {
        let _ = (id, force);
        Err("posts are not available".to_string())
    }

    /// Read post meta. An empty key returns every key for that post.
    fn get_post_meta(&self, post_id: u64, key: &str) -> Vec<crate::posts::PostMeta> {
        let _ = (post_id, key);
        Vec::new()
    }

    /// Replace one meta value, or insert it when the key is new.
    fn update_post_meta(
        &self,
        post_id: u64,
        key: &str,
        value: &str,
        prev: &str,
    ) -> Result<bool, String> {
        let _ = (post_id, key, value, prev);
        Err("post meta is not available".to_string())
    }

    /// Add another meta row. `unique` refuses a key that is already stored.
    fn add_post_meta(
        &self,
        post_id: u64,
        key: &str,
        value: &str,
        unique: bool,
    ) -> Result<bool, String> {
        let _ = (post_id, key, value, unique);
        Err("post meta is not available".to_string())
    }

    /// Delete meta rows for one key. A previous value limits the match.
    fn delete_post_meta(&self, post_id: u64, key: &str, prev: &str) -> Result<bool, String> {
        let _ = (post_id, key, prev);
        Err("post meta is not available".to_string())
    }

    /// Insert one term. The default has no term store.
    fn insert_term(
        &self,
        draft: &crate::taxonomies::TermDraft,
    ) -> Result<crate::taxonomies::TermRecord, String> {
        let _ = draft;
        Err("terms are not available".to_string())
    }

    /// List terms in one taxonomy.
    fn get_terms(&self, taxonomy: &str) -> Vec<crate::taxonomies::TermRecord> {
        let _ = taxonomy;
        Vec::new()
    }

    /// Replace or append the terms on one post.
    fn set_object_terms(
        &self,
        post_id: u64,
        taxonomy: &str,
        term_ids: &[u64],
        append: bool,
    ) -> Result<(), String> {
        let _ = (post_id, taxonomy, term_ids, append);
        Err("terms are not available".to_string())
    }

    /// Terms already assigned to one post.
    fn object_terms(&self, post_id: u64, taxonomy: &str) -> Vec<crate::taxonomies::TermRecord> {
        let _ = (post_id, taxonomy);
        Vec::new()
    }

    /// Delete one term and its assignments.
    fn delete_term(&self, term_id: u64, taxonomy: &str) -> Result<bool, String> {
        let _ = (term_id, taxonomy);
        Err("terms are not available".to_string())
    }

    /// True while the host is drawing an admin screen.
    fn is_admin(&self) -> bool {
        false
    }

    /// Whether the signed-in account has a WordPress capability.
    fn user_can(&self, capability: &str) -> bool {
        let _ = capability;
        false
    }

    /// Perform one public HTTP request. The default refuses private addresses.
    fn http_request(
        &self,
        request: &crate::http::HttpRequest,
    ) -> Result<crate::http::HttpResponse, String> {
        crate::http::perform(request)
    }
}

/// One hook, shortcode, or lifecycle callback returned by a plugin load.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub kind: RegistrationKind,
    pub tag: String,
    pub callback_id: u64,
    pub priority: i32,
}

/// Kind of registration the worker reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationKind {
    Action,
    Filter,
    Shortcode,
    Activation,
    Deactivation,
}

/// One plugin page registered with `add_menu_page` or `add_options_page`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminPage {
    pub slug: String,
    pub parent: String,
    pub page_title: String,
    pub menu_title: String,
}

/// HTML a plugin printed for one settings page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedAdminPage {
    pub page_title: String,
    pub menu_title: String,
    pub html: String,
}

/// A plugin file that has been included in the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedPlugin {
    pub plugin: DiscoveredPlugin,
    pub registrations: Vec<Registration>,
}

/// A running `php php/worker.php` process, or a test double that speaks the same frames.
#[derive(Clone)]
pub struct PhpRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    io: Mutex<Option<Session>>,
    child: Mutex<Option<Child>>,
    config: Option<WorkerConfig>,
    host: Arc<dyn PhpHost>,
    output: Mutex<HashMap<String, String>>,
    last_error: Mutex<Option<String>>,
}

struct Session {
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    next_id: u64,
}

#[derive(Clone)]
struct WorkerConfig {
    php: PathBuf,
    plugins_dir: Option<PathBuf>,
    site_url: String,
}

struct Started {
    session: Session,
    child: Child,
}

impl PhpRuntime {
    /// Spawn `php` from `OXIDEPRESS_PHP` or `PATH`.
    pub fn spawn(host: Arc<dyn PhpHost>) -> Result<Self, PhpError> {
        Self::spawn_php(php_program(), None, None, host)
    }

    /// Spawn a PHP binary and tell the worker which directory plugins live in.
    ///
    /// `site_url` becomes `OXIDEPRESS_SITE_URL`. `None` keeps `http://localhost`.
    pub fn spawn_php(
        php: impl AsRef<Path>,
        plugins_dir: Option<&Path>,
        site_url: Option<&str>,
        host: Arc<dyn PhpHost>,
    ) -> Result<Self, PhpError> {
        let config = WorkerConfig {
            php: php.as_ref().to_path_buf(),
            plugins_dir: plugins_dir.map(Path::to_path_buf),
            site_url: site_url.unwrap_or("http://localhost").to_string(),
        };
        let started = start_process(&config)?;
        Ok(Self::from_started(started, Some(config), host))
    }

    /// Include a plugin file and return the hooks it registered.
    ///
    /// This does not run activation callbacks. Call [`Self::run_activation`]
    /// for that, matching WordPress: plugins are included on every boot, and
    /// the activation hook runs only when an administrator activates them.
    pub fn load(&self, plugin: &DiscoveredPlugin) -> Result<LoadedPlugin, PhpError> {
        let path = plugin.file_path.to_str().ok_or_else(|| {
            PhpError::Protocol(format!(
                "plugin path is not utf-8: {}",
                plugin.file_path.display()
            ))
        })?;
        let frame = self.roundtrip(json!({
            "op": "load",
            "path": path,
        }))?;
        if frame.get("ok").and_then(Value::as_bool) != Some(true) {
            let message = frame
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("plugin failed")
                .to_string();
            return Err(PhpError::Plugin {
                path: path.to_string(),
                message,
            });
        }
        let registrations = parse_registrations(frame.get("registrations"))?;
        Ok(LoadedPlugin {
            plugin: plugin.clone(),
            registrations,
        })
    }

    /// Register the plugin's actions, filters, and shortcodes with OxidePress.
    ///
    /// Activation and deactivation callbacks are stored on `loaded` and run
    /// only through [`Self::run_activation`] and [`Self::run_deactivation`].
    pub fn attach(
        &self,
        hooks: &oxidepress_core::hooks::HookRegistry,
        shortcodes: &oxidepress_core::shortcode::ShortcodeRegistry,
        loaded: &LoadedPlugin,
    ) {
        self.attach_while(hooks, shortcodes, loaded, Arc::new(AtomicBool::new(true)));
    }

    /// Register callbacks that stop running when `live` is set false.
    ///
    /// The PHP process keeps the file. Rust stops calling that plugin.
    pub fn attach_while(
        &self,
        hooks: &oxidepress_core::hooks::HookRegistry,
        shortcodes: &oxidepress_core::shortcode::ShortcodeRegistry,
        loaded: &LoadedPlugin,
        live: Arc<AtomicBool>,
    ) {
        for reg in &loaded.registrations {
            let runtime = self.clone();
            let callback_id = reg.callback_id;
            let tag = reg.tag.clone();
            let live = live.clone();
            match reg.kind {
                RegistrationKind::Action => {
                    hooks.add_action(
                        &reg.tag,
                        Arc::new(move |args| {
                            if !live.load(Ordering::Relaxed) {
                                return;
                            }
                            match runtime.invoke_action(callback_id, args) {
                                Ok(output) => runtime.push_output(&tag, &output),
                                Err(err) => runtime.note_error(err.to_string()),
                            }
                        }),
                        reg.priority,
                    );
                }
                RegistrationKind::Filter => {
                    hooks.add_filter(
                        &reg.tag,
                        Arc::new(move |value| {
                            if !live.load(Ordering::Relaxed) {
                                return value;
                            }
                            match runtime.invoke_filter(callback_id, value.clone()) {
                                Ok((next, output)) => {
                                    runtime.push_output(&tag, &output);
                                    next
                                }
                                Err(err) => {
                                    runtime.note_error(err.to_string());
                                    value
                                }
                            }
                        }),
                        reg.priority,
                    );
                }
                RegistrationKind::Shortcode => {
                    shortcodes.add_shortcode(
                        &reg.tag,
                        Arc::new(move |attrs, content| {
                            if !live.load(Ordering::Relaxed) {
                                return String::new();
                            }
                            match runtime.invoke_shortcode(callback_id, &tag, attrs, content) {
                                Ok((rendered, output)) => {
                                    runtime.push_output(&tag, &output);
                                    rendered
                                }
                                Err(err) => {
                                    runtime.note_error(err.to_string());
                                    String::new()
                                }
                            }
                        }),
                    );
                }
                RegistrationKind::Activation | RegistrationKind::Deactivation => {}
            }
        }
    }

    /// Run `register_activation_hook` callbacks for one loaded plugin.
    pub fn run_activation(&self, loaded: &LoadedPlugin) -> Result<(), PhpError> {
        self.run_lifecycle(loaded, RegistrationKind::Activation, "activate")
    }

    /// Run `register_deactivation_hook` callbacks for one loaded plugin.
    pub fn run_deactivation(&self, loaded: &LoadedPlugin) -> Result<(), PhpError> {
        self.run_lifecycle(loaded, RegistrationKind::Deactivation, "deactivate")
    }

    /// Run this plugin's action callbacks for one tag.
    ///
    /// Used when a plugin is activated after startup, so its `init` callbacks
    /// run once without firing `init` again for plugins already loaded.
    pub fn run_tag(&self, loaded: &LoadedPlugin, tag: &str) -> Result<(), PhpError> {
        for reg in &loaded.registrations {
            if reg.kind == RegistrationKind::Action && reg.tag == tag {
                let output = self.invoke_action(reg.callback_id, &Value::Null)?;
                self.push_output(tag, &output);
            }
        }
        Ok(())
    }

    /// Menu pages plugins registered, after `admin_menu` has run once.
    pub fn admin_pages(&self) -> Result<Vec<AdminPage>, PhpError> {
        let frame = self.roundtrip(json!({ "op": "admin_menus" }))?;
        ensure_ok(&frame)?;
        let Some(pages) = frame.get("pages").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        let mut listed = Vec::new();
        for page in pages {
            let slug = page.get("slug").and_then(Value::as_str).unwrap_or("");
            if !worker_token(slug, 80) {
                continue;
            }
            listed.push(AdminPage {
                slug: slug.to_string(),
                parent: json_text(page.get("parent")),
                page_title: json_text(page.get("page_title")),
                menu_title: json_text(page.get("menu_title")),
            });
        }
        Ok(listed)
    }

    /// Render one plugin settings page. `nonce` is printed by `settings_fields`.
    pub fn render_admin_page(
        &self,
        slug: &str,
        nonce: &str,
    ) -> Result<RenderedAdminPage, PhpError> {
        if !worker_token(slug, 80) {
            return Err(PhpError::Worker("unknown page".to_string()));
        }
        let nonce = if worker_token(nonce, 128) { nonce } else { "" };
        let frame = self.roundtrip(json!({
            "op": "admin_page",
            "slug": slug,
            "nonce": nonce,
        }))?;
        ensure_ok(&frame)?;
        Ok(RenderedAdminPage {
            page_title: json_text(frame.get("page_title")),
            menu_title: json_text(frame.get("menu_title")),
            html: json_text(frame.get("html")),
        })
    }

    /// Store posted values whose names were `register_setting`'d for `group`.
    ///
    /// Protected option names, other groups, and unregistered keys are ignored.
    pub fn save_admin_settings(
        &self,
        group: &str,
        values: &HashMap<String, String>,
    ) -> Result<Vec<String>, PhpError> {
        if !worker_token(group, 80) {
            return Err(PhpError::Worker("unknown settings group".to_string()));
        }
        let mut clean = serde_json::Map::new();
        for (name, value) in values {
            if clean.len() >= 40 {
                break;
            }
            if !worker_token(name, 191) {
                continue;
            }
            let mut stored = value.replace('\0', "");
            if stored.len() > 10_000 {
                stored.truncate(10_000);
            }
            clean.insert(name.clone(), Value::String(stored));
        }
        let frame = self.roundtrip(json!({
            "op": "admin_save",
            "group": group,
            "values": clean,
        }))?;
        ensure_ok(&frame)?;
        let saved = frame
            .get("saved")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|name| worker_token(name, 191))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Ok(saved)
    }

    /// Post types plugins registered, after `init` has run once inside the worker.
    pub fn registered_post_types(&self) -> Result<Vec<crate::posts::RegisteredPostType>, PhpError> {
        let frame = self.roundtrip(json!({ "op": "post_types" }))?;
        ensure_ok(&frame)?;
        let Some(types) = frame.get("types").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(types
            .iter()
            .filter_map(crate::posts::type_from_value)
            .collect())
    }

    /// Taxonomies plugins registered, after `init` has run once inside the worker.
    pub fn registered_taxonomies(
        &self,
    ) -> Result<Vec<crate::taxonomies::RegisteredTaxonomy>, PhpError> {
        let frame = self.roundtrip(json!({ "op": "taxonomies" }))?;
        ensure_ok(&frame)?;
        let Some(items) = frame.get("taxonomies").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };
        Ok(items
            .iter()
            .filter_map(crate::taxonomies::taxonomy_from_value)
            .collect())
    }

    /// Take echoed output captured while `tag` ran, and clear it.
    pub fn take_output(&self, tag: &str) -> String {
        self.inner
            .output
            .lock()
            .map(|mut output| output.remove(tag).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Most recent callback failure. Hook callbacks keep the previous filter
    /// value instead of panicking, and record the reason here.
    pub fn last_error(&self) -> Option<String> {
        self.inner
            .last_error
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
    }

    /// Replace the PHP process with a new one.
    ///
    /// Callback ids from earlier [`Self::load`] calls do not survive. Attach
    /// the plugins to a new hook registry afterwards.
    pub fn restart(&self) -> Result<(), PhpError> {
        let config = self
            .inner
            .config
            .clone()
            .ok_or_else(|| PhpError::Protocol("restart requires a php process".to_string()))?;
        self.stop_process();
        let started = start_process(&config)?;
        let mut io = self.inner.io.lock().map_err(|_| PhpError::Poisoned)?;
        *io = Some(started.session);
        let mut child = self.inner.child.lock().map_err(|_| PhpError::Poisoned)?;
        *child = Some(started.child);
        Ok(())
    }

    fn from_started(
        started: Started,
        config: Option<WorkerConfig>,
        host: Arc<dyn PhpHost>,
    ) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                io: Mutex::new(Some(started.session)),
                child: Mutex::new(Some(started.child)),
                config,
                host,
                output: Mutex::new(HashMap::new()),
                last_error: Mutex::new(None),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_io(
        reader: Box<dyn Read + Send>,
        writer: Box<dyn Write + Send>,
        host: Arc<dyn PhpHost>,
    ) -> Self {
        Self {
            inner: Arc::new(RuntimeInner {
                io: Mutex::new(Some(Session {
                    reader,
                    writer,
                    next_id: 1,
                })),
                child: Mutex::new(None),
                config: None,
                host,
                output: Mutex::new(HashMap::new()),
                last_error: Mutex::new(None),
            }),
        }
    }

    fn run_lifecycle(
        &self,
        loaded: &LoadedPlugin,
        kind: RegistrationKind,
        tag: &str,
    ) -> Result<(), PhpError> {
        for reg in &loaded.registrations {
            if reg.kind == kind {
                let output = self.invoke_action(reg.callback_id, &Value::Null)?;
                self.push_output(tag, &output);
            }
        }
        Ok(())
    }

    fn invoke_action(&self, callback_id: u64, args: &Value) -> Result<String, PhpError> {
        let frame = self.roundtrip(json!({
            "op": "invoke",
            "kind": "action",
            "callback_id": callback_id,
            "args": args,
        }))?;
        ensure_ok(&frame)?;
        Ok(frame
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    fn invoke_filter(&self, callback_id: u64, value: Value) -> Result<(Value, String), PhpError> {
        let frame = self.roundtrip(json!({
            "op": "invoke",
            "kind": "filter",
            "callback_id": callback_id,
            "value": value,
        }))?;
        ensure_ok(&frame)?;
        let output = frame
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let next = frame.get("value").cloned().unwrap_or(Value::Null);
        Ok((next, output))
    }

    fn invoke_shortcode(
        &self,
        callback_id: u64,
        tag: &str,
        attrs: &std::collections::BTreeMap<String, String>,
        content: &str,
    ) -> Result<(String, String), PhpError> {
        let frame = self.roundtrip(json!({
            "op": "invoke",
            "kind": "shortcode",
            "callback_id": callback_id,
            "tag": tag,
            "atts": attrs,
            "content": content,
        }))?;
        ensure_ok(&frame)?;
        let rendered = json_scalar_string(frame.get("value"));
        let output = frame
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok((rendered, output))
    }

    fn roundtrip(&self, mut message: Value) -> Result<Value, PhpError> {
        let host = Arc::clone(&self.inner.host);
        let mut io_guard = self.inner.io.lock().map_err(|_| PhpError::Poisoned)?;
        let io = io_guard.as_mut().ok_or(PhpError::WorkerExited)?;
        let id = io.next_id;
        io.next_id = io.next_id.saturating_add(1);
        if let Some(object) = message.as_object_mut() {
            object.insert("id".to_string(), Value::from(id));
        }
        write_frame(io.writer.as_mut(), &message)?;
        loop {
            let frame = read_frame(io.reader.as_mut())?;
            if frame.get("call").and_then(Value::as_str).is_some() {
                let result = dispatch_host(host.as_ref(), &frame);
                write_frame(
                    io.writer.as_mut(),
                    &json!({
                        "id": id,
                        "result": result,
                    }),
                )?;
                continue;
            }
            return Ok(frame);
        }
    }

    fn push_output(&self, tag: &str, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Ok(mut output) = self.inner.output.lock() {
            output.entry(tag.to_string()).or_default().push_str(text);
        }
    }

    fn note_error(&self, message: String) {
        if let Ok(mut slot) = self.inner.last_error.lock() {
            *slot = Some(message);
        }
    }

    fn stop_process(&self) {
        if let Ok(mut io) = self.inner.io.lock() {
            io.take();
        }
        if let Ok(mut child) = self.inner.child.lock() {
            if let Some(mut child) = child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

impl Drop for RuntimeInner {
    fn drop(&mut self) {
        if let Ok(mut io) = self.io.lock() {
            io.take();
        }
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut child) = child.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn start_process(config: &WorkerConfig) -> Result<Started, PhpError> {
    let script = worker_script();
    if !script.is_file() {
        return Err(PhpError::Protocol(format!(
            "php worker script is missing at {}",
            script.display()
        )));
    }

    let mut cmd = Command::new(&config.php);
    cmd.arg("-d")
        .arg("display_errors=stderr")
        .arg("-d")
        .arg("log_errors=1")
        .arg("-d")
        .arg("output_buffering=0")
        .arg("-d")
        .arg("implicit_flush=1")
        .arg("-d")
        .arg("disable_functions=exec,shell_exec,system,passthru,proc_open,popen")
        .arg(&script);
    if let Some(dir) = &config.plugins_dir {
        cmd.env("OXIDEPRESS_PLUGINS_DIR", dir);
        if dir.is_dir() {
            cmd.current_dir(dir);
        }
    }
    cmd.env("OXIDEPRESS_SITE_URL", &config.site_url);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn()?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| PhpError::Protocol("php worker stdin is missing".to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PhpError::Protocol("php worker stdout is missing".to_string()))?;
    Ok(Started {
        session: Session {
            reader: Box::new(stdout),
            writer: Box::new(stdin),
            next_id: 1,
        },
        child,
    })
}

fn worker_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("php")
        .join("worker.php")
}

fn php_program() -> PathBuf {
    std::env::var_os("OXIDEPRESS_PHP")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("php"))
}

/// ASCII token used for plugin page slugs, setting groups, and option names.
pub fn worker_token(value: &str, max: usize) -> bool {
    (1..=max).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn json_text(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or("").to_string()
}

fn ensure_ok(frame: &Value) -> Result<(), PhpError> {
    match frame.get("ok").and_then(Value::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(PhpError::Worker(
            frame
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("php callback failed")
                .to_string(),
        )),
        None => Err(PhpError::Protocol("worker response missing ok".to_string())),
    }
}

fn parse_registrations(value: Option<&Value>) -> Result<Vec<Registration>, PhpError> {
    let items = value
        .and_then(Value::as_array)
        .ok_or_else(|| PhpError::Protocol("registrations is not an array".to_string()))?;
    let mut registrations = Vec::with_capacity(items.len());
    for item in items {
        let kind = match item.get("kind").and_then(Value::as_str) {
            Some("action") => RegistrationKind::Action,
            Some("filter") => RegistrationKind::Filter,
            Some("shortcode") => RegistrationKind::Shortcode,
            Some("activation") => RegistrationKind::Activation,
            Some("deactivation") => RegistrationKind::Deactivation,
            other => {
                return Err(PhpError::Protocol(format!(
                    "unknown registration kind {other:?}"
                )));
            }
        };
        let callback_id = item
            .get("callback_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| PhpError::Protocol("registration is missing callback_id".to_string()))?;
        let tag = item
            .get("tag")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let priority = item
            .get("priority")
            .and_then(Value::as_i64)
            .and_then(|number| i32::try_from(number).ok())
            .unwrap_or(10);
        registrations.push(Registration {
            kind,
            tag,
            callback_id,
            priority,
        });
    }
    Ok(registrations)
}

fn dispatch_host(host: &dyn PhpHost, frame: &Value) -> Value {
    let name = frame
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match frame.get("call").and_then(Value::as_str) {
        Some("get_option") => match host.get_option(&name) {
            Some(value) => Value::String(value),
            None => Value::Null,
        },
        Some("update_option") => {
            let value = json_scalar_string(frame.get("value"));
            Value::Bool(host.update_option(&name, &value))
        }
        Some("delete_option") => Value::Bool(host.delete_option(&name)),
        Some("http") => crate::http::dispatch(host, frame),
        Some("get_post") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            match host.get_post(id) {
                Some(post) => crate::posts::record_json(&post),
                None => Value::Null,
            }
        }
        Some("query_posts") => match crate::posts::query_from_value(frame.get("query")) {
            Ok(query) => Value::Array(
                host.query_posts(&query)
                    .iter()
                    .map(crate::posts::record_json)
                    .collect(),
            ),
            Err(_) => Value::Array(Vec::new()),
        },
        Some("insert_post") => match crate::posts::draft_from_value(frame.get("post")) {
            Ok(draft) => match host.insert_post(&draft) {
                Ok(id) => Value::from(id),
                Err(_) => Value::Null,
            },
            Err(_) => Value::Null,
        },
        Some("update_post") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            match crate::posts::draft_from_value(frame.get("post")) {
                Ok(draft) => Value::Bool(host.update_post(id, &draft).is_ok()),
                Err(_) => Value::Bool(false),
            }
        }
        Some("delete_post") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let force = frame.get("force").and_then(Value::as_bool).unwrap_or(false);
            Value::Bool(host.delete_post(id, force).unwrap_or(false))
        }
        Some("get_post_meta") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let key = frame.get("meta_key").and_then(Value::as_str).unwrap_or("");
            Value::Array(
                host.get_post_meta(id, key)
                    .into_iter()
                    .map(|row| json!({ "key": row.key, "value": row.value }))
                    .collect(),
            )
        }
        Some("update_post_meta") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let key = frame.get("meta_key").and_then(Value::as_str).unwrap_or("");
            let value = json_scalar_string(frame.get("value"));
            let prev = json_scalar_string(frame.get("prev"));
            Value::Bool(
                host.update_post_meta(id, key, &value, &prev)
                    .unwrap_or(false),
            )
        }
        Some("add_post_meta") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let key = frame.get("meta_key").and_then(Value::as_str).unwrap_or("");
            let value = json_scalar_string(frame.get("value"));
            let unique = frame
                .get("unique")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Value::Bool(host.add_post_meta(id, key, &value, unique).unwrap_or(false))
        }
        Some("delete_post_meta") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let key = frame.get("meta_key").and_then(Value::as_str).unwrap_or("");
            let prev = json_scalar_string(frame.get("prev"));
            Value::Bool(host.delete_post_meta(id, key, &prev).unwrap_or(false))
        }
        Some("insert_term") => match crate::taxonomies::term_draft_from_value(frame.get("term")) {
            Ok(draft) => match host.insert_term(&draft) {
                Ok(term) => crate::taxonomies::term_json(&term),
                Err(_) => Value::Null,
            },
            Err(_) => Value::Null,
        },
        Some("get_terms") => {
            let taxonomy = frame.get("taxonomy").and_then(Value::as_str).unwrap_or("");
            Value::Array(
                host.get_terms(taxonomy)
                    .iter()
                    .map(crate::taxonomies::term_json)
                    .collect(),
            )
        }
        Some("set_object_terms") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let taxonomy = frame.get("taxonomy").and_then(Value::as_str).unwrap_or("");
            let append = frame
                .get("append")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let ids = frame
                .get("terms")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_u64)
                        .take(20)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Value::Bool(host.set_object_terms(id, taxonomy, &ids, append).is_ok())
        }
        Some("object_terms") => {
            let id = frame.get("post_id").and_then(Value::as_u64).unwrap_or(0);
            let taxonomy = frame.get("taxonomy").and_then(Value::as_str).unwrap_or("");
            Value::Array(
                host.object_terms(id, taxonomy)
                    .iter()
                    .map(crate::taxonomies::term_json)
                    .collect(),
            )
        }
        Some("delete_term") => {
            let id = frame.get("term_id").and_then(Value::as_u64).unwrap_or(0);
            let taxonomy = frame.get("taxonomy").and_then(Value::as_str).unwrap_or("");
            Value::Bool(host.delete_term(id, taxonomy).unwrap_or(false))
        }
        Some("is_admin") => Value::Bool(host.is_admin()),
        Some("current_user_can") => {
            let capability = frame
                .get("capability")
                .and_then(Value::as_str)
                .unwrap_or("");
            Value::Bool(host.user_can(capability))
        }
        _ => Value::Null,
    }
}

fn json_scalar_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::{TcpListener, TcpStream};
    use std::process::{Command, Stdio};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use oxidepress_core::hooks::HookRegistry;
    use oxidepress_core::shortcode::ShortcodeRegistry;
    use serde_json::{json, Value};

    use super::*;
    use crate::discover::discover;
    use crate::protocol::{read_frame, write_frame};

    struct StoredMeta {
        post_id: u64,
        key: String,
        value: String,
    }

    struct MemHost {
        options: Mutex<HashMap<String, String>>,
        http: Mutex<Vec<crate::http::HttpRequest>>,
        posts: Mutex<Vec<crate::posts::PostRecord>>,
        meta: Mutex<Vec<StoredMeta>>,
        terms: Mutex<Vec<crate::taxonomies::TermRecord>>,
        rels: Mutex<Vec<(u64, u64)>>,
        next_post: Mutex<u64>,
        next_term: Mutex<u64>,
        admin: bool,
        caps: Vec<String>,
    }

    impl MemHost {
        fn new() -> Self {
            Self {
                options: Mutex::new(HashMap::new()),
                http: Mutex::new(Vec::new()),
                posts: Mutex::new(Vec::new()),
                meta: Mutex::new(Vec::new()),
                terms: Mutex::new(Vec::new()),
                rels: Mutex::new(Vec::new()),
                next_post: Mutex::new(1),
                next_term: Mutex::new(1),
                admin: false,
                caps: Vec::new(),
            }
        }

        fn with_visitor(admin: bool, caps: &[&str]) -> Self {
            let mut host = Self::new();
            host.admin = admin;
            host.caps = caps.iter().map(|cap| (*cap).to_string()).collect();
            host
        }

        fn set(&self, name: &str, value: &str) {
            self.options
                .lock()
                .unwrap()
                .insert(name.to_string(), value.to_string());
        }

        fn post_type_of(&self, id: u64) -> Result<String, String> {
            self.posts
                .lock()
                .unwrap()
                .iter()
                .find(|post| post.id == id)
                .map(|post| post.post_type.clone())
                .ok_or_else(|| "post was not found".to_string())
        }

        fn write_meta(
            &self,
            post_id: u64,
            key: &str,
            value: &str,
            prev: &str,
            always_insert: bool,
        ) -> Result<bool, String> {
            let post_type = self.post_type_of(post_id)?;
            if !crate::posts::content_type_allowed(&post_type)
                || !crate::posts::meta_key_allowed(key)
            {
                return Err("post meta is not allowed".to_string());
            }
            let value = crate::posts::clean_meta_value(value);
            let mut meta = self.meta.lock().unwrap();
            if !always_insert {
                if let Some(row) = meta.iter_mut().find(|row| {
                    row.post_id == post_id
                        && row.key == key
                        && (prev.is_empty() || row.value == prev)
                }) {
                    row.value = value;
                    return Ok(true);
                }
                if !prev.is_empty() {
                    return Ok(false);
                }
            }
            if meta.iter().filter(|row| row.post_id == post_id).count() >= 100 {
                return Err("too much post meta".to_string());
            }
            meta.push(StoredMeta {
                post_id,
                key: key.to_string(),
                value,
            });
            Ok(true)
        }
    }

    impl PhpHost for MemHost {
        fn get_option(&self, name: &str) -> Option<String> {
            self.options.lock().unwrap().get(name).cloned()
        }

        fn update_option(&self, name: &str, value: &str) -> bool {
            self.options
                .lock()
                .unwrap()
                .insert(name.to_string(), value.to_string());
            true
        }

        fn delete_option(&self, name: &str) -> bool {
            self.options.lock().unwrap().remove(name).is_some()
        }

        fn is_admin(&self) -> bool {
            self.admin
        }

        fn user_can(&self, capability: &str) -> bool {
            self.caps.iter().any(|cap| cap == capability)
        }

        fn get_post(&self, id: u64) -> Option<crate::posts::PostRecord> {
            self.posts
                .lock()
                .unwrap()
                .iter()
                .find(|post| post.id == id)
                .cloned()
        }

        fn query_posts(&self, query: &crate::posts::PostQuery) -> Vec<crate::posts::PostRecord> {
            self.posts
                .lock()
                .unwrap()
                .iter()
                .filter(|post| post.post_type == query.post_type && post.status == query.status)
                .filter(|post| query.name.is_empty() || post.name == query.name)
                .filter(|post| query.search.is_empty() || post.title.contains(&query.search))
                .take(query.limit as usize)
                .cloned()
                .collect()
        }

        fn insert_post(&self, draft: &crate::posts::PostDraft) -> Result<u64, String> {
            let mut next = self.next_post.lock().unwrap();
            let id = *next;
            *next += 1;
            self.posts.lock().unwrap().push(crate::posts::PostRecord {
                id,
                author: 1,
                date: "2026-11-09 00:00:00".to_string(),
                content: draft.content.clone(),
                title: draft.title.clone(),
                excerpt: draft.excerpt.clone(),
                status: draft.status.clone(),
                name: draft.name.clone(),
                post_type: draft.post_type.clone(),
                parent: draft.parent,
                guid: String::new(),
                menu_order: 0,
                comment_count: 0,
            });
            Ok(id)
        }

        fn update_post(&self, id: u64, draft: &crate::posts::PostDraft) -> Result<(), String> {
            let mut posts = self.posts.lock().unwrap();
            let Some(post) = posts.iter_mut().find(|post| post.id == id) else {
                return Err("missing".to_string());
            };
            if !crate::posts::content_type_allowed(&post.post_type) {
                return Err("post type is not allowed".to_string());
            }
            post.title = draft.title.clone();
            post.content = draft.content.clone();
            post.excerpt = draft.excerpt.clone();
            post.status = draft.status.clone();
            post.name = draft.name.clone();
            post.parent = draft.parent;
            Ok(())
        }

        fn delete_post(&self, id: u64, force: bool) -> Result<bool, String> {
            let mut posts = self.posts.lock().unwrap();
            let Some(index) = posts.iter().position(|post| post.id == id) else {
                return Ok(false);
            };
            if !crate::posts::content_type_allowed(&posts[index].post_type) {
                return Err("post type is not allowed".to_string());
            }
            if force {
                posts.remove(index);
            } else {
                posts[index].status = "trash".to_string();
            }
            Ok(true)
        }

        fn get_post_meta(&self, post_id: u64, key: &str) -> Vec<crate::posts::PostMeta> {
            if self.post_type_of(post_id).is_err() {
                return Vec::new();
            }
            self.meta
                .lock()
                .unwrap()
                .iter()
                .filter(|row| row.post_id == post_id && (key.is_empty() || row.key == key))
                .take(if key.is_empty() { 100 } else { 20 })
                .map(|row| crate::posts::PostMeta {
                    key: row.key.clone(),
                    value: row.value.clone(),
                })
                .collect()
        }

        fn update_post_meta(
            &self,
            post_id: u64,
            key: &str,
            value: &str,
            prev: &str,
        ) -> Result<bool, String> {
            self.write_meta(post_id, key, value, prev, false)
        }

        fn add_post_meta(
            &self,
            post_id: u64,
            key: &str,
            value: &str,
            unique: bool,
        ) -> Result<bool, String> {
            if unique
                && self
                    .get_post_meta(post_id, key)
                    .iter()
                    .any(|row| row.key == key)
            {
                return Ok(false);
            }
            self.write_meta(post_id, key, value, "", true)
        }

        fn delete_post_meta(&self, post_id: u64, key: &str, prev: &str) -> Result<bool, String> {
            let post_type = self.post_type_of(post_id)?;
            if !crate::posts::content_type_allowed(&post_type)
                || !crate::posts::meta_key_allowed(key)
            {
                return Err("post meta is not allowed".to_string());
            }
            let mut meta = self.meta.lock().unwrap();
            let before = meta.len();
            meta.retain(|row| {
                !(row.post_id == post_id
                    && row.key == key
                    && (prev.is_empty() || row.value == prev))
            });
            Ok(meta.len() != before)
        }

        fn insert_term(
            &self,
            draft: &crate::taxonomies::TermDraft,
        ) -> Result<crate::taxonomies::TermRecord, String> {
            if !crate::taxonomies::term_taxonomy_allowed(&draft.taxonomy) {
                return Err("taxonomy is not allowed".to_string());
            }
            let mut terms = self.terms.lock().unwrap();
            if terms.iter().any(|term| {
                term.taxonomy == draft.taxonomy && term.name.eq_ignore_ascii_case(&draft.name)
            }) {
                return Err("term exists".to_string());
            }
            let parent = if draft.parent > 0
                && terms
                    .iter()
                    .any(|term| term.term_id == draft.parent && term.taxonomy == draft.taxonomy)
            {
                draft.parent
            } else {
                0
            };
            let mut slug = draft.slug.clone();
            let mut suffix = 2u32;
            while terms
                .iter()
                .any(|term| term.taxonomy == draft.taxonomy && term.slug == slug)
            {
                slug = format!("{}-{suffix}", draft.slug);
                suffix += 1;
                if suffix > 20 {
                    break;
                }
            }
            let mut next = self.next_term.lock().unwrap();
            let id = *next;
            *next += 1;
            let term = crate::taxonomies::TermRecord {
                term_id: id,
                term_taxonomy_id: id,
                taxonomy: draft.taxonomy.clone(),
                name: draft.name.clone(),
                slug,
                description: draft.description.clone(),
                parent,
                count: 0,
            };
            terms.push(term.clone());
            Ok(term)
        }

        fn get_terms(&self, taxonomy: &str) -> Vec<crate::taxonomies::TermRecord> {
            if !crate::taxonomies::term_taxonomy_allowed(taxonomy) {
                return Vec::new();
            }
            let mut terms: Vec<_> = self
                .terms
                .lock()
                .unwrap()
                .iter()
                .filter(|term| term.taxonomy == taxonomy)
                .cloned()
                .collect();
            terms.sort_by(|left, right| left.name.cmp(&right.name));
            terms.truncate(40);
            terms
        }

        fn set_object_terms(
            &self,
            post_id: u64,
            taxonomy: &str,
            term_ids: &[u64],
            append: bool,
        ) -> Result<(), String> {
            let post_type = self.post_type_of(post_id)?;
            if !crate::posts::content_type_allowed(&post_type)
                || !crate::taxonomies::term_taxonomy_allowed(taxonomy)
            {
                return Err("terms are not allowed".to_string());
            }
            let mut terms = self.terms.lock().unwrap();
            let chosen: Vec<u64> = term_ids
                .iter()
                .copied()
                .filter(|id| {
                    terms
                        .iter()
                        .any(|term| term.term_id == *id && term.taxonomy == taxonomy)
                })
                .take(20)
                .collect();
            if chosen.len() != term_ids.len() {
                return Err("term was not found".to_string());
            }
            let mut rels = self.rels.lock().unwrap();
            let in_taxonomy = |taxonomy_id: u64| {
                terms
                    .iter()
                    .any(|term| term.term_taxonomy_id == taxonomy_id && term.taxonomy == taxonomy)
            };
            if !append {
                rels.retain(|(object_id, taxonomy_id)| {
                    *object_id != post_id || !in_taxonomy(*taxonomy_id)
                });
            }
            for id in &chosen {
                if !rels
                    .iter()
                    .any(|(object_id, taxonomy_id)| *object_id == post_id && taxonomy_id == id)
                {
                    rels.push((post_id, *id));
                }
            }
            for term in terms.iter_mut().filter(|term| term.taxonomy == taxonomy) {
                term.count = rels
                    .iter()
                    .filter(|(_, taxonomy_id)| *taxonomy_id == term.term_taxonomy_id)
                    .count() as i64;
            }
            Ok(())
        }

        fn object_terms(&self, post_id: u64, taxonomy: &str) -> Vec<crate::taxonomies::TermRecord> {
            let terms = self.terms.lock().unwrap();
            let rels = self.rels.lock().unwrap();
            terms
                .iter()
                .filter(|term| {
                    term.taxonomy == taxonomy
                        && rels.iter().any(|(object_id, taxonomy_id)| {
                            *object_id == post_id && *taxonomy_id == term.term_taxonomy_id
                        })
                })
                .cloned()
                .collect()
        }

        fn delete_term(&self, term_id: u64, taxonomy: &str) -> Result<bool, String> {
            if !crate::taxonomies::term_taxonomy_allowed(taxonomy) {
                return Err("taxonomy is not allowed".to_string());
            }
            let mut terms = self.terms.lock().unwrap();
            let Some(index) = terms
                .iter()
                .position(|term| term.term_id == term_id && term.taxonomy == taxonomy)
            else {
                return Ok(false);
            };
            let taxonomy_id = terms[index].term_taxonomy_id;
            terms.remove(index);
            let mut rels = self.rels.lock().unwrap();
            rels.retain(|(_, id)| *id != taxonomy_id);
            Ok(true)
        }

        fn http_request(
            &self,
            request: &crate::http::HttpRequest,
        ) -> Result<crate::http::HttpResponse, String> {
            self.http.lock().unwrap().push(request.clone());
            Ok(crate::http::HttpResponse {
                status: 200,
                reason: "OK".to_string(),
                body: "akismet-ok".to_string(),
            })
        }
    }

    fn fake_plugin() -> DiscoveredPlugin {
        DiscoveredPlugin {
            name: "Fake".to_string(),
            version: "1".to_string(),
            description: String::new(),
            author: String::new(),
            file_path: PathBuf::from("fake.php"),
            basename: "fake.php".to_string(),
        }
    }

    fn fake_worker(listener: TcpListener) {
        let (mut stream, _) = match listener.accept() {
            Ok(accepted) => accepted,
            Err(_) => return,
        };
        let _ = stream.set_nodelay(true);
        loop {
            let frame = match read_frame(&mut stream) {
                Ok(frame) => frame,
                Err(_) => return,
            };
            let id = frame.get("id").cloned().unwrap_or(Value::Null);
            let op = frame.get("op").and_then(Value::as_str).unwrap_or("");
            if op == "load" {
                if write_frame(
                    &mut stream,
                    &json!({
                        "id": id,
                        "ok": true,
                        "registrations": [
                            {"kind": "filter", "tag": "the_content", "callback_id": 1, "priority": 10},
                            {"kind": "shortcode", "tag": "hello", "callback_id": 2, "priority": 10},
                            {"kind": "action", "tag": "init", "callback_id": 3, "priority": 10}
                        ]
                    }),
                )
                .is_err()
                {
                    return;
                }
                continue;
            }
            if op == "invoke" {
                let kind = frame.get("kind").and_then(Value::as_str).unwrap_or("");
                if kind == "filter" {
                    if write_frame(
                        &mut stream,
                        &json!({"id": id, "call": "get_option", "name": "suffix"}),
                    )
                    .is_err()
                    {
                        return;
                    }
                    let reply = match read_frame(&mut stream) {
                        Ok(reply) => reply,
                        Err(_) => return,
                    };
                    let suffix = reply.get("result").and_then(Value::as_str).unwrap_or("");
                    let base = frame.get("value").and_then(Value::as_str).unwrap_or("");
                    if write_frame(
                        &mut stream,
                        &json!({
                            "id": id,
                            "ok": true,
                            "value": format!("{base}{suffix}"),
                            "output": ""
                        }),
                    )
                    .is_err()
                    {
                        return;
                    }
                } else if kind == "shortcode" {
                    if write_frame(
                        &mut stream,
                        &json!({"id": id, "ok": true, "value": "short", "output": ""}),
                    )
                    .is_err()
                    {
                        return;
                    }
                } else if write_frame(
                    &mut stream,
                    &json!({"id": id, "ok": true, "output": "booted"}),
                )
                .is_err()
                {
                    return;
                }
                continue;
            }
            if write_frame(
                &mut stream,
                &json!({"id": id, "ok": false, "error": "unknown op"}),
            )
            .is_err()
            {
                return;
            }
        }
    }

    fn connect_fake(host: Arc<dyn PhpHost>) -> (PhpRuntime, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || fake_worker(listener));
        let stream = TcpStream::connect(addr).unwrap();
        let _ = stream.set_nodelay(true);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let reader = stream.try_clone().unwrap();
        let _ = reader.set_read_timeout(Some(Duration::from_secs(5)));
        let runtime = PhpRuntime::from_io(Box::new(reader), Box::new(stream), host);
        (runtime, handle)
    }

    #[test]
    fn fake_worker_attaches_filter_shortcode_and_action() {
        let host = Arc::new(MemHost::new());
        host.set("suffix", " from-fake");
        let (runtime, handle) = connect_fake(host);
        let loaded = runtime.load(&fake_plugin()).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);

        assert_eq!(
            hooks.apply_filters("the_content", json!("hello")),
            json!("hello from-fake")
        );
        assert_eq!(shortcodes.do_shortcode("[hello]"), "short");
        hooks.do_action("init", &json!({}));
        assert_eq!(runtime.take_output("init"), "booted");
        assert!(runtime.last_error().is_none());

        drop(hooks);
        drop(shortcodes);
        drop(runtime);
        handle.join().unwrap();
    }

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    fn php_bin_works() -> bool {
        Command::new(php_program())
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[test]
    fn php_plugin_filters_shortcode_option_and_echo() {
        if !php_bin_works() {
            eprintln!("skipping php integration test: php is not on PATH");
            return;
        }

        let host = Arc::new(MemHost::new());
        host.set("notice_suffix", "!");
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&fixtures()), None, host.clone()).unwrap();
        let found = discover(&fixtures()).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "Sample Notice")
            .unwrap();
        let loaded = runtime.load(plugin).unwrap();
        assert!(loaded.registrations.iter().any(|reg| {
            reg.kind == RegistrationKind::Filter && reg.tag == "the_content" && reg.priority == 15
        }));

        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);

        assert_eq!(
            hooks.apply_filters("the_content", json!("hello")),
            json!("hello!")
        );
        assert_eq!(shortcodes.do_shortcode("[sample_notice]"), "notice");
        assert_eq!(
            shortcodes.do_shortcode("[sample_notice name=\"<b>\"]"),
            "&lt;b&gt;"
        );
        hooks.do_action("init", &json!({}));
        assert_eq!(runtime.take_output("init"), "ready");
        assert_eq!(host.get_option("notice_seen").as_deref(), Some("yes"));
        assert!(host.get_option("notice_activated").is_none());
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());

        runtime.run_activation(&loaded).unwrap();
        assert_eq!(host.get_option("notice_activated").as_deref(), Some("1"));
        assert_eq!(
            host.get_option("notice_basename").as_deref(),
            Some("sample-notice/sample-notice.php")
        );

        drop(hooks);
        drop(shortcodes);
        runtime.restart().unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);
        assert_eq!(
            hooks.apply_filters("the_content", json!("again")),
            json!("again!")
        );
    }

    #[test]
    fn php_missing_function_is_reported() {
        if !php_bin_works() {
            eprintln!("skipping php integration test: php is not on PATH");
            return;
        }

        let host = Arc::new(MemHost::new());
        let runtime = PhpRuntime::spawn_php(php_program(), Some(&fixtures()), None, host).unwrap();
        let found = discover(&fixtures()).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "Missing Call")
            .unwrap();
        let err = runtime.load(plugin).unwrap_err();
        assert!(err.to_string().contains("wp_mail"), "{err}");
    }

    #[test]
    fn php_remote_post_reaches_the_host_and_private_urls_do_not() {
        if !php_bin_works() {
            eprintln!("skipping php integration test: php is not on PATH");
            return;
        }

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&fixtures()), None, host.clone()).unwrap();
        let found = discover(&fixtures()).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "Needs Remote")
            .unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);
        hooks.do_action("init", &json!({}));
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());

        let blocked = host.get_option("remote_blocked").unwrap();
        assert!(blocked.contains("private"), "{blocked}");
        assert_eq!(host.get_option("remote_code").as_deref(), Some("200"));
        assert_eq!(
            host.get_option("remote_body").as_deref(),
            Some("akismet-ok")
        );
        let calls = host.http.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].method, "POST");
        assert_eq!(calls[0].url, "https://example.com/1.1/comment-check");
        assert_eq!(calls[0].body, "blog=example");
        assert!(calls[0]
            .headers
            .iter()
            .any(|(name, value)| name == "Content-Type"
                && value == "application/x-www-form-urlencoded"));
    }

    #[test]
    fn php_boot_surface_covers_real_plugin_helpers() {
        if !php_bin_works() {
            eprintln!("skipping php integration test: php is not on PATH");
            return;
        }

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&fixtures()), None, host.clone()).unwrap();
        let found = discover(&fixtures()).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "Boot Surface")
            .unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);

        assert_eq!(
            hooks.apply_filters("use_widgets_block_editor", json!(true)),
            json!(false)
        );
        hooks.do_action("init", &json!({}));
        hooks.do_action("admin_notices", &json!({}));

        assert_eq!(
            host.get_option("boot_field").as_deref(),
            Some("widget-boot_surface-title")
        );
        let url = host.get_option("boot_url").unwrap();
        assert!(
            url.contains("/wp-content/plugins/boot-surface/app.js"),
            "{url}"
        );
        let joined = host.get_option("boot_joined").unwrap();
        assert!(
            joined.replace('\\', "/").ends_with("/boot-surface/app.js"),
            "{joined}"
        );
        assert_eq!(host.get_option("boot_admin").as_deref(), Some("no"));
        assert_eq!(host.get_option("boot_locale").as_deref(), Some("en_US"));
        assert_eq!(host.get_option("boot_site").as_deref(), Some("fallback"));
        assert_eq!(
            host.get_option("boot_query").as_deref(),
            Some("http://localhost/notice?dolly=1")
        );
        assert_eq!(host.get_option("boot_has_notice").as_deref(), Some("yes"));
        assert_eq!(host.get_option("boot_type").as_deref(), Some("boot_form"));
        assert_eq!(host.get_option("boot_type_exists").as_deref(), Some("yes"));
        assert_eq!(
            host.get_option("boot_menu").as_deref(),
            Some("tools.php_page_boot-surface")
        );
        assert_eq!(runtime.take_output("admin_notices"), "Hello Dolly");
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
    }

    #[test]
    fn php_enqueued_assets_print_in_head_and_footer() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-assets-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("asset-queue");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("asset-queue.php"),
            r#"<?php
/**
 * Plugin Name: Asset Queue
 */
add_action('wp_enqueue_scripts', function () {
    wp_register_script('base', 'https://example.com/base.js', array(), '1');
    wp_enqueue_script('app', 'https://example.com/app.js', array('base'), '2', true);
    wp_enqueue_style('app', 'https://example.com/app.css', array(), '3');
});
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::new());
        let runtime = PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "Asset Queue")
            .unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);

        hooks.do_action("wp_head", &json!({}));
        hooks.do_action("wp_footer", &json!({}));

        let head = runtime.take_output("wp_head");
        let footer = runtime.take_output("wp_footer");
        let css = head.find("id=\"app-css\"").expect(&head);
        let base = head.find("id=\"base-js\"").expect(&head);
        assert!(css < base, "{head}");
        assert!(head.contains("https://example.com/app.css?ver=3"), "{head}");
        assert!(head.contains("https://example.com/base.js?ver=1"), "{head}");
        assert!(!head.contains("app.js"), "{head}");
        assert!(footer.contains("id=\"app-js\""), "{footer}");
        assert!(
            footer.contains("https://example.com/app.js?ver=2"),
            "{footer}"
        );
        assert!(!footer.contains("base.js"), "{footer}");
        assert!(!footer.contains("app.css"), "{footer}");
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn php_settings_page_renders_and_saves_only_registered_options() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-settings-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("settings-page");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("settings-page.php"),
            r#"<?php
/**
 * Plugin Name: Settings Page
 */
add_action('admin_menu', function () {
    if (!is_admin() || !current_user_can('manage_options')) {
        return;
    }
    add_options_page('Queue Settings', 'Queue', 'manage_options', 'queue-settings', 'queue_settings_page');
    register_setting('queue_group', 'queue_label', array(
        'type' => 'string',
        'sanitize_callback' => 'queue_clean',
    ));
    register_setting('queue_group', 'siteurl');
    add_settings_section('queue_main', 'Queue', function () {
        echo '<p>How the queue is labeled.</p>';
    }, 'queue-settings');
    add_settings_field('queue_label', 'Label', function () {
        echo '<input type="text" name="queue_label" value="' . esc_attr(get_option('queue_label', '')) . '" />';
    }, 'queue-settings', 'queue_main');
});
add_action('init', function () {
    update_option('settings_is_admin', is_admin() ? 'yes' : 'no');
});
function queue_settings_page() {
    echo '<form method="post" action="options.php">';
    settings_fields('queue_group');
    do_settings_sections('queue-settings');
    submit_button('Save Queue');
    echo '</form>';
}
function queue_clean($value) {
    return 'clean-' . $value;
}
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host.clone()).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "Settings Page")
            .unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);

        let pages = runtime.admin_pages().unwrap();
        assert_eq!(pages.len(), 1, "{pages:?}");
        assert_eq!(pages[0].slug, "queue-settings");
        assert_eq!(pages[0].menu_title, "Queue");

        let rendered = runtime
            .render_admin_page("queue-settings", "abc123nonce")
            .unwrap();
        assert_eq!(rendered.page_title, "Queue Settings");
        assert!(
            rendered.html.contains("How the queue is labeled."),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("name=\"queue_label\""),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("action=\"/wp-admin/options.php\""),
            "{}",
            rendered.html
        );
        assert!(
            rendered.html.contains("value=\"abc123nonce\""),
            "{}",
            rendered.html
        );
        assert!(
            !rendered.html.contains("action=\"options.php\""),
            "{}",
            rendered.html
        );

        let mut posted = HashMap::new();
        posted.insert("queue_label".to_string(), "Hello<script>".to_string());
        posted.insert("siteurl".to_string(), "http://evil.example".to_string());
        posted.insert("users_can_register".to_string(), "1".to_string());
        posted.insert("option_page".to_string(), "queue_group".to_string());
        let saved = runtime.save_admin_settings("queue_group", &posted).unwrap();
        assert_eq!(saved, vec!["queue_label".to_string()]);
        assert_eq!(
            host.get_option("queue_label").as_deref(),
            Some("clean-Hello")
        );
        assert!(host.get_option("siteurl").is_none());
        assert!(host.get_option("users_can_register").is_none());

        hooks.do_action("init", &json!({}));
        assert_eq!(host.get_option("settings_is_admin").as_deref(), Some("no"));
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn php_plugin_can_register_a_post_type_and_store_a_post() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-events-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("events");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("events.php"),
            r#"<?php
/**
 * Plugin Name: Events
 */
add_action('init', function () {
    register_post_type('event', array(
        'label' => 'Events',
        'labels' => array('singular_name' => 'Event'),
        'public' => true,
        'show_ui' => true,
    ));
    $id = wp_insert_post(array(
        'post_title' => 'Launch<script>',
        'post_content' => 'Day',
        'post_status' => 'publish',
        'post_type' => 'event',
    ));
    update_option('event_id', (string)$id);
    $post = get_post($id);
    update_option('event_title', $post ? $post->post_title : '');
    update_option('event_type', $post ? $post->post_type : '');
    update_option('event_link', (string)get_permalink($post));
    $found = get_posts(array(
        'post_type' => 'event',
        'post_status' => 'publish',
        'numberposts' => 5,
    ));
    update_option('event_count', (string)count($found));
    wp_update_post(array('ID' => $id, 'post_title' => 'Updated'));
    $again = get_post($id);
    update_option('event_updated', $again ? $again->post_title : '');
    $revision = wp_insert_post(array(
        'post_title' => 'Nope',
        'post_type' => 'revision',
        'post_status' => 'publish',
    ));
    update_option('event_revision', (string)$revision);
});
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host.clone()).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found.iter().find(|plugin| plugin.name == "Events").unwrap();
        runtime.load(plugin).unwrap();

        let types = runtime.registered_post_types().unwrap();
        assert_eq!(types.len(), 1, "{types:?}");
        assert_eq!(types[0].name, "event");
        assert_eq!(types[0].label, "Events");
        assert_eq!(types[0].singular, "Event");
        assert!(types[0].show_ui);
        assert_eq!(host.get_option("event_id").as_deref(), Some("1"));
        assert_eq!(host.get_option("event_title").as_deref(), Some("Launch"));
        assert_eq!(host.get_option("event_type").as_deref(), Some("event"));
        assert_eq!(
            host.get_option("event_link").as_deref(),
            Some("http://localhost/launch/")
        );
        assert_eq!(host.get_option("event_count").as_deref(), Some("1"));
        assert_eq!(host.get_option("event_updated").as_deref(), Some("Updated"));
        assert_eq!(host.get_option("event_revision").as_deref(), Some("0"));
        let stored = host.get_post(1).unwrap();
        assert_eq!(stored.post_type, "event");
        assert_eq!(stored.title, "Updated");
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn php_plugin_can_read_and_write_post_meta() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-meta-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("meta");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("meta.php"),
            r#"<?php
/**
 * Plugin Name: Meta
 */
add_action('init', function () {
    $id = wp_insert_post(array(
        'post_title' => 'Launch',
        'post_status' => 'publish',
        'post_type' => 'post',
    ));
    update_option('meta_saved', update_post_meta($id, 'city', 'Oslo') ? 'yes' : 'no');
    update_option('meta_city', (string)get_post_meta($id, 'city', true));
    update_option('meta_array', update_post_meta($id, 'city', array('no')) ? 'yes' : 'no');
    update_option('meta_prev', update_post_meta($id, 'city', 'Bergen', 'missing') ? 'yes' : 'no');
    update_option('meta_still', (string)get_post_meta($id, 'city', true));
    add_post_meta($id, 'note', 'one', true);
    add_post_meta($id, 'note', 'two', true);
    $notes = get_post_meta($id, 'note', false);
    update_option('meta_notes', is_array($notes) ? (string)count($notes) : '0');
    $all = get_post_meta($id);
    update_option('meta_keys', is_array($all) ? (string)count($all) : '0');
    update_option('meta_deleted', delete_post_meta($id, 'city') ? 'yes' : 'no');
    update_option('meta_after', (string)get_post_meta($id, 'city', true));
    update_option('meta_missing', update_post_meta(999, 'city', 'x') ? 'yes' : 'no');
});
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host.clone()).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found.iter().find(|plugin| plugin.name == "Meta").unwrap();
        runtime.load(plugin).unwrap();
        runtime
            .registered_post_types()
            .expect("init runs while listing types");

        assert_eq!(host.get_option("meta_saved").as_deref(), Some("yes"));
        assert_eq!(host.get_option("meta_city").as_deref(), Some("Oslo"));
        assert_eq!(host.get_option("meta_array").as_deref(), Some("no"));
        assert_eq!(host.get_option("meta_prev").as_deref(), Some("no"));
        assert_eq!(host.get_option("meta_still").as_deref(), Some("Oslo"));
        assert_eq!(host.get_option("meta_notes").as_deref(), Some("1"));
        assert_eq!(host.get_option("meta_keys").as_deref(), Some("2"));
        assert_eq!(host.get_option("meta_deleted").as_deref(), Some("yes"));
        assert_eq!(host.get_option("meta_after").as_deref(), Some(""));
        assert_eq!(host.get_option("meta_missing").as_deref(), Some("no"));
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn php_plugin_can_register_a_taxonomy() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-venues-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("venues");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("venues.php"),
            r#"<?php
/**
 * Plugin Name: Venues
 */
add_action('init', function () {
    $tax = register_taxonomy('venue', 'event', array(
        'label' => 'Venues',
        'labels' => array('singular_name' => 'Venue'),
        'public' => true,
        'show_ui' => true,
        'hierarchical' => true,
    ));
    update_option('tax_ok', $tax instanceof WP_Taxonomy ? 'yes' : 'no');
    update_option('tax_exists', taxonomy_exists('venue') ? 'yes' : 'no');
    update_option('tax_category', taxonomy_exists('category') ? 'yes' : 'no');
    $menu = register_taxonomy('nav_menu', 'post', array('label' => 'Nope'));
    update_option('tax_menu', $menu instanceof WP_Error ? 'refused' : 'accepted');
    $revision = register_taxonomy('place', 'revision', array('label' => 'Places'));
    update_option('tax_revision', $revision instanceof WP_Error ? 'refused' : 'accepted');
});
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host.clone()).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found.iter().find(|plugin| plugin.name == "Venues").unwrap();
        runtime.load(plugin).unwrap();
        let taxes = runtime.registered_taxonomies().unwrap();
        assert_eq!(taxes.len(), 1, "{taxes:?}");
        assert_eq!(taxes[0].name, "venue");
        assert_eq!(taxes[0].label, "Venues");
        assert_eq!(taxes[0].singular, "Venue");
        assert_eq!(taxes[0].object_types, vec!["event"]);
        assert!(taxes[0].show_ui);
        assert!(taxes[0].hierarchical);
        assert_eq!(host.get_option("tax_ok").as_deref(), Some("yes"));
        assert_eq!(host.get_option("tax_exists").as_deref(), Some("yes"));
        assert_eq!(host.get_option("tax_category").as_deref(), Some("yes"));
        assert_eq!(host.get_option("tax_menu").as_deref(), Some("refused"));
        assert_eq!(host.get_option("tax_revision").as_deref(), Some("refused"));
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn php_plugin_can_store_a_term_and_assign_it() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-terms-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("terms");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("terms.php"),
            r#"<?php
/**
 * Plugin Name: Terms
 */
add_action('init', function () {
    register_taxonomy('venue', 'event', array(
        'label' => 'Venues',
        'public' => true,
        'show_ui' => true,
    ));
    $id = wp_insert_post(array(
        'post_title' => 'Launch',
        'post_status' => 'publish',
        'post_type' => 'event',
    ));
    $created = wp_insert_term('Oslo', 'venue');
    update_option('term_id', is_array($created) ? (string)$created['term_id'] : '0');
    $again = wp_insert_term('Oslo', 'venue');
    update_option('term_again', $again instanceof WP_Error ? 'refused' : 'accepted');
    $menu = wp_insert_term('Main', 'nav_menu');
    update_option('term_menu', $menu instanceof WP_Error ? 'refused' : 'accepted');
    $set = wp_set_object_terms($id, array('Oslo', 'Bergen'), 'venue');
    update_option('term_set', is_array($set) ? (string)count($set) : '0');
    $got = wp_get_object_terms($id, 'venue');
    $names = array();
    if (is_array($got)) {
        foreach ($got as $term) {
            $names[] = $term->name;
        }
    }
    sort($names);
    update_option('term_names', implode(',', $names));
    wp_set_object_terms($id, array(), 'venue');
    $after = wp_get_object_terms($id, 'venue');
    update_option('term_cleared', is_array($after) ? (string)count($after) : 'bad');
});
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host.clone()).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found.iter().find(|plugin| plugin.name == "Terms").unwrap();
        runtime.load(plugin).unwrap();
        runtime.registered_taxonomies().unwrap();
        assert_eq!(host.get_option("term_id").as_deref(), Some("1"));
        assert_eq!(host.get_option("term_again").as_deref(), Some("refused"));
        assert_eq!(host.get_option("term_menu").as_deref(), Some("refused"));
        assert_eq!(host.get_option("term_set").as_deref(), Some("2"));
        assert_eq!(
            host.get_option("term_names").as_deref(),
            Some("Bergen,Oslo")
        );
        assert_eq!(host.get_option("term_cleared").as_deref(), Some("0"));
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    fn slash(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }

    fn write_abspath_probe(plugins: &Path) {
        let dir = plugins.join("abspath-probe");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("abspath-probe.php"),
            r#"<?php
/**
 * Plugin Name: ABSPATH Probe
 */
add_action('init', function () {
    update_option('probe_abspath', ABSPATH);
    update_option('probe_plugin_dir', WP_PLUGIN_DIR);
    update_option('probe_content_dir', WP_CONTENT_DIR);
    update_option('probe_uploads', wp_upload_dir()['basedir']);
});
"#,
        )
        .unwrap();
    }

    fn probe_paths(plugins: &Path) -> (String, String, String, String) {
        let host = Arc::new(MemHost::new());
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(plugins), None, host.clone()).unwrap();
        let found = discover(plugins).unwrap();
        let plugin = found
            .iter()
            .find(|plugin| plugin.name == "ABSPATH Probe")
            .unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);
        hooks.do_action("init", &json!({}));
        assert!(runtime.last_error().is_none(), "{:?}", runtime.last_error());
        (
            host.get_option("probe_abspath").unwrap(),
            host.get_option("probe_plugin_dir").unwrap(),
            host.get_option("probe_content_dir").unwrap(),
            host.get_option("probe_uploads").unwrap(),
        )
    }

    #[test]
    fn php_abspath_follows_the_install_layout() {
        if !php_bin_works() {
            eprintln!("skipping php integration test: php is not on PATH");
            return;
        }

        let root = std::env::temp_dir().join(format!("oxidepress-abspath-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let standard = root.join("standard").join("wp-content").join("plugins");
        write_abspath_probe(&standard);
        let (abspath, plugin_dir, content_dir, uploads) = probe_paths(&standard);
        assert_eq!(abspath, format!("{}/", slash(&root.join("standard"))));
        assert_eq!(plugin_dir, slash(&standard));
        assert_eq!(
            content_dir,
            slash(&root.join("standard").join("wp-content"))
        );
        assert_eq!(
            uploads,
            format!(
                "{}/uploads",
                slash(&root.join("standard").join("wp-content"))
            )
        );

        let loose_root = root.join("loose");
        let loose = loose_root.join("dropped");
        write_abspath_probe(&loose);
        let (abspath, plugin_dir, content_dir, uploads) = probe_paths(&loose);
        assert_eq!(abspath, format!("{}/", slash(&loose_root)));
        assert_ne!(abspath.trim_end_matches('/'), slash(&loose));
        assert_eq!(plugin_dir, slash(&loose));
        assert_eq!(content_dir, slash(&loose_root));
        assert_eq!(uploads, format!("{}/uploads", slash(&loose_root)));

        let loaded_root = root.join("with-load");
        std::fs::create_dir_all(&loaded_root).unwrap();
        std::fs::write(loaded_root.join("wp-load.php"), "<?php\n").unwrap();
        let custom = loaded_root.join("custom");
        write_abspath_probe(&custom);
        let (abspath, plugin_dir, content_dir, _) = probe_paths(&custom);
        assert_eq!(abspath, format!("{}/", slash(&loaded_root)));
        assert_eq!(plugin_dir, slash(&custom));
        assert_eq!(content_dir, slash(&loaded_root));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn php_current_user_can_follows_the_signed_in_visitor() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-caps-{}", std::process::id()));
        let plugins = root.join("plugins");
        let plugin_dir = plugins.join("caps");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("caps.php"),
            r#"<?php
/**
 * Plugin Name: Caps
 */
add_action('init', function () {
    update_option('cap_admin', is_admin() ? 'yes' : 'no');
    update_option('cap_edit', current_user_can('edit_posts') ? 'yes' : 'no');
    update_option('cap_manage', current_user_can('manage_options') ? 'yes' : 'no');
});
"#,
        )
        .unwrap();

        let host = Arc::new(MemHost::with_visitor(true, &["edit_posts"]));
        let runtime =
            PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host.clone()).unwrap();
        let found = discover(&plugins).unwrap();
        let plugin = found.iter().find(|plugin| plugin.name == "Caps").unwrap();
        let loaded = runtime.load(plugin).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes, &loaded);
        hooks.do_action("init", &json!({}));
        assert_eq!(host.get_option("cap_admin").as_deref(), Some("yes"));
        assert_eq!(host.get_option("cap_edit").as_deref(), Some("yes"));
        assert_eq!(host.get_option("cap_manage").as_deref(), Some("no"));
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn php_one_plugin_can_stop_while_the_other_keeps_filtering() {
        if !php_bin_works() {
            return;
        }
        let root = std::env::temp_dir().join(format!("oxidepress-stop-{}", std::process::id()));
        let plugins = root.join("plugins");
        for (dir, body) in [
            (
                "alpha",
                "function alpha_filter($content) { return $content . 'A'; }\nadd_filter('the_content', 'alpha_filter');\n",
            ),
            (
                "beta",
                "function beta_filter($content) { return $content . 'B'; }\nadd_filter('the_content', 'beta_filter');\n",
            ),
        ] {
            let plugin_dir = plugins.join(dir);
            std::fs::create_dir_all(&plugin_dir).unwrap();
            std::fs::write(
                plugin_dir.join(format!("{dir}.php")),
                format!("<?php\n/**\n * Plugin Name: {dir}\n */\n{body}"),
            )
            .unwrap();
        }

        let host = Arc::new(MemHost::new());
        let runtime = PhpRuntime::spawn_php(php_program(), Some(&plugins), None, host).unwrap();
        let found = discover(&plugins).unwrap();
        let alpha = found.iter().find(|plugin| plugin.name == "alpha").unwrap();
        let beta = found.iter().find(|plugin| plugin.name == "beta").unwrap();
        let alpha_loaded = runtime.load(alpha).unwrap();
        let beta_loaded = runtime.load(beta).unwrap();
        let hooks = HookRegistry::new();
        let shortcodes = ShortcodeRegistry::new();
        let alpha_live = Arc::new(std::sync::atomic::AtomicBool::new(true));
        runtime.attach_while(&hooks, &shortcodes, &alpha_loaded, alpha_live.clone());
        runtime.attach(&hooks, &shortcodes, &beta_loaded);
        assert_eq!(hooks.apply_filters("the_content", json!("x")), json!("xAB"));
        alpha_live.store(false, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(hooks.apply_filters("the_content", json!("x")), json!("xB"));
        drop(runtime);
        let _ = std::fs::remove_dir_all(root);
    }
}
