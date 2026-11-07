// Modified by Vabian124 on 2026-11-07 to run WordPress plugins in a PHP worker.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! WordPress-style plugin discovery.
//!
//! A plugin is a top-level `*.php` file, or a `*.php` file directly inside a
//! plugin directory, whose first 8 KiB contain a `Plugin Name:` header.
//! Subdirectories are not scanned.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::PhpError;

const HEADER_BYTES: usize = 8192;

/// A PHP file with a WordPress plugin header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub file_path: PathBuf,
    /// Path relative to the plugins directory, using `/` separators.
    ///
    /// `hello.php` or `akismet/akismet.php`.
    pub basename: String,
}

/// Scan `plugins_dir` the way WordPress scans `wp-content/plugins`.
pub fn discover(plugins_dir: &Path) -> Result<Vec<DiscoveredPlugin>, PhpError> {
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }
    if !plugins_dir.is_dir() {
        return Err(PhpError::Protocol(format!(
            "{} is not a directory",
            plugins_dir.display()
        )));
    }

    let mut found = Vec::new();
    for entry in std::fs::read_dir(plugins_dir)? {
        let path = entry?.path();
        if path.is_dir() {
            for child in std::fs::read_dir(&path)? {
                let child_path = child?.path();
                if let Some(plugin) = read_plugin(plugins_dir, &child_path) {
                    found.push(plugin);
                }
            }
        } else if let Some(plugin) = read_plugin(plugins_dir, &path) {
            found.push(plugin);
        }
    }
    found.sort_by(|left, right| left.basename.cmp(&right.basename));
    Ok(found)
}

fn read_plugin(plugins_dir: &Path, file: &Path) -> Option<DiscoveredPlugin> {
    if !is_php(file) || !file.is_file() {
        return None;
    }
    let text = header_text(file)?;
    let name = header_value(&text, "Plugin Name:")?;
    Some(DiscoveredPlugin {
        name,
        version: header_value(&text, "Version:").unwrap_or_default(),
        description: header_value(&text, "Description:").unwrap_or_default(),
        author: header_value(&text, "Author:").unwrap_or_default(),
        basename: plugin_basename(plugins_dir, file),
        file_path: file.to_path_buf(),
    })
}

fn is_php(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("php"))
}

fn plugin_basename(plugins_dir: &Path, file: &Path) -> String {
    file.strip_prefix(plugins_dir)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn header_text(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; HEADER_BYTES];
    let read = file.read(&mut buf).ok()?;
    buf.truncate(read);
    if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        buf.drain(..3);
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn header_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let stripped = line.trim_start_matches([' ', '\t', '\u{feff}', '/', '*', '#']);
        let Some(value) = stripped.strip_prefix(key) else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
    }

    #[test]
    fn discovers_top_level_file_and_plugin_directory_only() {
        let found = discover(&fixtures()).unwrap();
        let names: Vec<&str> = found.iter().map(|plugin| plugin.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Boot Surface",
                "Hello Dropin",
                "Needs Remote",
                "Sample Notice"
            ]
        );

        let hello = found
            .iter()
            .find(|plugin| plugin.name == "Hello Dropin")
            .unwrap();
        assert_eq!(hello.author, "OxidePress");
        assert_eq!(hello.version, "0.1.0");
        assert_eq!(hello.basename, "hello.php");
        assert!(hello.description.contains("Single-file"));

        let sample = found
            .iter()
            .find(|plugin| plugin.name == "Sample Notice")
            .unwrap();
        assert_eq!(sample.version, "1.2.0");
        assert_eq!(sample.basename, "sample-notice/sample-notice.php");
        assert_eq!(sample.author, "OxidePress");
        assert!(sample.file_path.ends_with("sample-notice.php"));
    }

    #[test]
    fn missing_directory_is_empty() {
        let missing = fixtures().join("does-not-exist");
        assert!(discover(&missing).unwrap().is_empty());
    }

    #[test]
    fn author_uri_is_not_the_author() {
        let text = " * Author URI: https://example.com\n * Author: OxidePress\n";
        assert_eq!(header_value(text, "Author:").as_deref(), Some("OxidePress"));
        assert_eq!(
            header_value(text, "Author URI:").as_deref(),
            Some("https://example.com")
        );
    }
}
