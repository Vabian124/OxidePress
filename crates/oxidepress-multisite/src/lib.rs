// Modified by Vabian124 on 2026-11-07 to rebrand as OxidePress.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! WordPress multisite support for OxidePress.
//!
//! This crate provides multisite (network) functionality compatible with
//! WordPress's multisite feature, including:
//!
//! - Network and site management
//! - Subdirectory and subdomain routing
//! - Per-site table prefix management
//! - Blog switching (switch_to_blog / restore_current_blog)

pub mod network;
pub mod routing;
pub mod switch;
pub mod tables;

pub use network::{Network, NetworkManager, Site, SiteStatus};
pub use routing::{DomainMapping, MultisiteMode, SiteResolver};
pub use switch::{BlogContext, SwitchManager};
pub use tables::{global_tables, is_main_site, per_site_tables, table_name};
