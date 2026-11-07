// Modified by Vabian124 on 2026-11-07 to run WordPress plugins in a PHP worker.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Run dropped-in WordPress plugins in one long-lived PHP process.
//!
//! OxidePress stays the CMS. The worker includes a plugin once, returns its
//! hook and shortcode registrations in a single batch, and runs a callback
//! only when Rust fires a tag that plugin registered.

mod discover;
mod error;
mod protocol;
mod worker;

pub use discover::{discover, DiscoveredPlugin};
pub use error::PhpError;
pub use worker::{LoadedPlugin, PhpHost, PhpRuntime, Registration, RegistrationKind};
