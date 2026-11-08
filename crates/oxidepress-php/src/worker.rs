// Modified by Vabian124 on 2026-11-08 to let the PHP worker make public HTTP requests.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! PHP process supervisor.
//!
//! One worker includes every active plugin. Rust registers one closure per
//! PHP callback so hook priority stays on [`oxidepress_core::hooks::HookRegistry`].
//! Deactivation restarts the process: that registry cannot remove a single callback.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
        for reg in &loaded.registrations {
            let runtime = self.clone();
            let callback_id = reg.callback_id;
            let tag = reg.tag.clone();
            match reg.kind {
                RegistrationKind::Action => {
                    hooks.add_action(
                        &reg.tag,
                        Arc::new(move |args| match runtime.invoke_action(callback_id, args) {
                            Ok(output) => runtime.push_output(&tag, &output),
                            Err(err) => runtime.note_error(err.to_string()),
                        }),
                        reg.priority,
                    );
                }
                RegistrationKind::Filter => {
                    hooks.add_filter(
                        &reg.tag,
                        Arc::new(move |value| {
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

    struct MemHost {
        options: Mutex<HashMap<String, String>>,
        http: Mutex<Vec<crate::http::HttpRequest>>,
    }

    impl MemHost {
        fn new() -> Self {
            Self {
                options: Mutex::new(HashMap::new()),
                http: Mutex::new(Vec::new()),
            }
        }

        fn set(&self, name: &str, value: &str) {
            self.options
                .lock()
                .unwrap()
                .insert(name.to_string(), value.to_string());
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
}
