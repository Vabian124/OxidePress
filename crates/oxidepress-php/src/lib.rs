// Modified by Vabian124 on 2026-11-11 to let plugins register a taxonomy.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Run dropped-in WordPress plugins in one long-lived PHP process.
//!
//! OxidePress stays the CMS. The worker includes a plugin once, returns its
//! hook and shortcode registrations in a single batch, and runs a callback
//! only when Rust fires a tag that plugin registered.

mod discover;
mod error;
mod http;
mod posts;
mod protocol;
mod taxonomies;
mod worker;

pub use discover::{discover, DiscoveredPlugin};
pub use error::PhpError;
pub use posts::{
    clean_meta_value, content_type_allowed, meta_key_allowed, PostDraft, PostMeta, PostQuery,
    PostRecord, RegisteredPostType,
};
pub use taxonomies::{taxonomy_name_allowed, RegisteredTaxonomy};
pub use worker::{
    worker_token, AdminPage, LoadedPlugin, PhpHost, PhpRuntime, Registration, RegistrationKind,
    RenderedAdminPage,
};
