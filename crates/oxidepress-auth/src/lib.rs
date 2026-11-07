// Modified by Vabian124 on 2026-11-07 to rebrand as OxidePress and export the login audit helpers.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
pub mod audit;
pub mod jwt;
pub mod jwt_blacklist;
pub mod middleware;
pub mod password;
pub mod rate_limit;
pub mod roles;
pub mod session;
pub mod totp;

pub use jwt::JwtManager;
pub use jwt_blacklist::{blacklist_token, is_blacklisted};
pub use middleware::AuthLayer;
pub use password::{PasswordHasher, PasswordPolicy};
pub use rate_limit::LoginAttemptTracker;
pub use roles::{Capability, Role};
pub use session::SessionManager;
pub use totp::{generate_qr_uri, generate_secret, verify_code as verify_totp};
