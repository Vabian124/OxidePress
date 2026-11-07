// Modified by Vabian124 on 2026-11-07 to parse plugin PHP into a Rust skeleton.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Parse WordPress plugin registrations and emit a Rust skeleton.
//!
//! The parser reads a plugin header and literal `add_action`, `add_filter`,
//! and `add_shortcode` calls. PHP function bodies are kept as source text and
//! are not executed.

mod emit;
mod parse;

pub use emit::emit_skeleton;
pub use parse::{
    parse_plugin, PluginHeader, PluginIr, Registration, RegistrationKind, Unsupported,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/sample-notice.php");
        std::fs::read_to_string(path).expect("fixture plugin")
    }

    #[test]
    fn fixture_header_and_registrations() {
        let ir = parse_plugin(&fixture());

        assert_eq!(ir.header.name, "Sample Notice");
        assert_eq!(ir.header.version, "1.2.0");
        assert_eq!(ir.header.description, "Shows a one-line notice.");
        assert_eq!(ir.header.author, "OxidePress");

        assert_eq!(ir.registrations.len(), 3);

        let action = &ir.registrations[0];
        assert_eq!(action.kind, RegistrationKind::Action);
        assert_eq!(action.tag, "init");
        assert_eq!(action.callback, "sample_notice_init");
        assert_eq!(action.priority, 10);
        let body = action.body.as_deref().expect("action body");
        assert!(body.contains("echo"), "{body}");

        let filter = &ir.registrations[1];
        assert_eq!(filter.kind, RegistrationKind::Filter);
        assert_eq!(filter.tag, "the_content");
        assert_eq!(filter.callback, "sample_notice_filter");
        assert_eq!(filter.priority, 15);

        let shortcode = &ir.registrations[2];
        assert_eq!(shortcode.kind, RegistrationKind::Shortcode);
        assert_eq!(shortcode.tag, "sample_notice");
        assert_eq!(shortcode.callback, "sample_notice_shortcode");
        assert_eq!(shortcode.priority, 10);
    }

    #[test]
    fn unsupported_calls_are_reported_and_skipped() {
        let ir = parse_plugin(&fixture());
        let sources: Vec<&str> = ir
            .unsupported
            .iter()
            .map(|item| item.source.as_str())
            .collect();

        assert!(
            sources.iter().any(|source| source.contains("include")),
            "{sources:?}"
        );
        assert!(
            sources.iter().any(|source| source.contains("$dynamic")),
            "{sources:?}"
        );
        assert!(ir
            .registrations
            .iter()
            .all(|registration| registration.tag != "$dynamic"));
    }

    #[test]
    fn skeleton_registers_hooks_without_php_bodies() {
        let ir = parse_plugin(&fixture());
        let rust = emit_skeleton(&ir);

        assert!(rust.contains("hooks.add_action("));
        assert!(rust.contains("\"init\""));
        assert!(rust.contains("sample_notice_init"));
        assert!(rust.contains("hooks.add_filter("));
        assert!(rust.contains("\"the_content\""));
        assert!(rust.contains("15"));
        assert!(rust.contains("shortcodes.add_shortcode("));
        assert!(rust.contains("\"sample_notice\""));
        assert!(rust.contains("// Original PHP function: sample_notice_init"));
        assert!(!rust.contains("echo"));
        assert!(!rust.contains("include"));
    }
}
