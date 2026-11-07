// Modified by Vabian124 on 2026-11-07 to report how real WordPress plugins load.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Load every discovered plugin in a directory and fire the usual boot hooks.
//!
//! ```text
//! cargo run -p oxidepress-php --bin oxidepress-php-probe -- <plugins-dir>
//! ```

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use oxidepress_core::hooks::HookRegistry;
use oxidepress_core::shortcode::ShortcodeRegistry;
use oxidepress_php::{discover, PhpHost, PhpRuntime, RegistrationKind};
use serde_json::json;

struct MemHost {
    options: Mutex<HashMap<String, String>>,
}

impl PhpHost for MemHost {
    fn get_option(&self, name: &str) -> Option<String> {
        self.options.lock().expect("options").get(name).cloned()
    }

    fn update_option(&self, name: &str, value: &str) -> bool {
        self.options
            .lock()
            .expect("options")
            .insert(name.to_string(), value.to_string());
        true
    }

    fn delete_option(&self, name: &str) -> bool {
        self.options.lock().expect("options").remove(name).is_some()
    }
}

fn main() -> ExitCode {
    let Some(dir) = env::args().nth(1) else {
        eprintln!("usage: oxidepress-php-probe <plugins-dir>");
        return ExitCode::from(2);
    };
    let dir = PathBuf::from(dir);
    let found = match discover(&dir) {
        Ok(found) => found,
        Err(err) => {
            eprintln!("discover failed: {err}");
            return ExitCode::from(1);
        }
    };
    let host = Arc::new(MemHost {
        options: Mutex::new(HashMap::from([
            ("blogname".to_string(), "OxidePress".to_string()),
            ("siteurl".to_string(), "http://localhost".to_string()),
            ("home".to_string(), "http://localhost".to_string()),
            ("admin_email".to_string(), "admin@example.com".to_string()),
            ("active_plugins".to_string(), "a:0:{}".to_string()),
        ])),
    });
    let php = env::var_os("OXIDEPRESS_PHP").unwrap_or_else(|| "php".into());
    let runtime = match PhpRuntime::spawn_php(php, Some(&dir), host) {
        Ok(runtime) => runtime,
        Err(err) => {
            eprintln!("php worker failed to start: {err}");
            return ExitCode::from(1);
        }
    };

    let tags = [
        "plugins_loaded",
        "init",
        "wp_loaded",
        "admin_init",
        "admin_menu",
        "admin_notices",
        "admin_head",
        "wp_enqueue_scripts",
        "wp_head",
        "wp_footer",
    ];

    println!("plugins {}", found.len());
    for plugin in &found {
        if runtime.restart().is_err() {
            eprintln!("restart failed before {}", plugin.basename);
            return ExitCode::from(1);
        }
        print!("{} | {} | ", plugin.basename, plugin.version);
        let loaded = match runtime.load(plugin) {
            Ok(loaded) => loaded,
            Err(err) => {
                println!("LOAD FAIL | {err}");
                continue;
            }
        };
        let actions = loaded
            .registrations
            .iter()
            .filter(|reg| reg.kind == RegistrationKind::Action)
            .count();
        let filters = loaded
            .registrations
            .iter()
            .filter(|reg| reg.kind == RegistrationKind::Filter)
            .count();
        let shortcodes = loaded
            .registrations
            .iter()
            .filter(|reg| reg.kind == RegistrationKind::Shortcode)
            .count();
        let hooks = HookRegistry::new();
        let shortcodes_reg = ShortcodeRegistry::new();
        runtime.attach(&hooks, &shortcodes_reg, &loaded);
        let mut ran = Vec::new();
        let mut stopped = None;
        for tag in tags {
            let before = runtime.last_error();
            hooks.do_action(tag, &json!({}));
            let after = runtime.last_error();
            if after.is_some() && after != before {
                stopped = after;
                break;
            }
            let output = runtime.take_output(tag);
            ran.push(if output.is_empty() {
                tag.to_string()
            } else {
                format!("{tag}({}b:{})", output.len(), preview(&output))
            });
        }
        let before = runtime.last_error();
        let filtered = hooks.apply_filters("the_content", json!("Hello"));
        let widgets = hooks.apply_filters("use_widgets_block_editor", json!(true));
        let content_note = if runtime.last_error() != before {
            format!(
                "the_content FAIL {}",
                runtime.last_error().unwrap_or_default()
            )
        } else {
            format!("the_content={filtered} widgets_block_editor={widgets}")
        };
        match stopped {
            Some(err) => println!(
                "LOAD OK actions={actions} filters={filters} shortcodes={shortcodes} ran={} STOP {err} | {content_note}",
                ran.join(",")
            ),
            None => println!(
                "LOAD OK actions={actions} filters={filters} shortcodes={shortcodes} ran={} | {content_note}",
                ran.join(",")
            ),
        }
    }
    ExitCode::SUCCESS
}

fn preview(output: &str) -> String {
    let flat = output.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars();
    let short: String = chars.by_ref().take(72).collect();
    if chars.next().is_some() {
        format!("{short}...")
    } else {
        short
    }
}
