// Modified by Vabian124 on 2026-11-09 to parse a WordPress capability name.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// WordPress user roles with associated capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Administrator,
    ShopManager,
    Editor,
    Author,
    Contributor,
    Subscriber,
}

/// WordPress capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    // Post capabilities
    EditPosts,
    EditOthersPosts,
    EditPublishedPosts,
    PublishPosts,
    DeletePosts,
    DeleteOthersPosts,
    DeletePublishedPosts,
    DeletePrivatePosts,
    EditPrivatePosts,
    ReadPrivatePosts,

    // Page capabilities
    EditPages,
    EditOthersPages,
    EditPublishedPages,
    PublishPages,
    DeletePages,
    DeleteOthersPages,
    DeletePublishedPages,
    DeletePrivatePages,
    EditPrivatePages,
    ReadPrivatePages,

    // User capabilities
    ListUsers,
    CreateUsers,
    EditUsers,
    DeleteUsers,
    PromoteUsers,

    // Theme capabilities
    EditThemeOptions,
    SwitchThemes,
    EditThemes,
    DeleteThemes,
    InstallThemes,

    // Plugin capabilities
    ActivatePlugins,
    EditPlugins,
    InstallPlugins,
    DeletePlugins,

    // Site capabilities
    ManageOptions,
    /// Products, orders, and the store catalog. Shop managers have this; they do not run the site.
    ManageWooCommerce,
    ManageLinks,
    ManageCategories,
    ModerateComments,
    UploadFiles,
    Import,
    Export,
    Unfiltered,
    Read,
}

impl Capability {
    /// WordPress capability slug, such as `manage_options`.
    ///
    /// `unfiltered_html` is the name plugins pass. The stored capability is `unfiltered`.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = if name == "unfiltered_html" {
            "unfiltered"
        } else {
            name
        };
        serde_json::from_value(serde_json::Value::String(name.to_string())).ok()
    }
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Administrator => "administrator",
            Self::ShopManager => "shop_manager",
            Self::Editor => "editor",
            Self::Author => "author",
            Self::Contributor => "contributor",
            Self::Subscriber => "subscriber",
        }
    }

    /// Parse a role from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "administrator" => Some(Self::Administrator),
            "shop_manager" => Some(Self::ShopManager),
            "editor" => Some(Self::Editor),
            "author" => Some(Self::Author),
            "contributor" => Some(Self::Contributor),
            "subscriber" => Some(Self::Subscriber),
            _ => None,
        }
    }

    /// Get the set of capabilities for this role.
    pub fn capabilities(&self) -> HashSet<Capability> {
        use Capability::*;
        match self {
            Role::Administrator => HashSet::from([
                EditPosts,
                EditOthersPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeleteOthersPosts,
                DeletePublishedPosts,
                DeletePrivatePosts,
                EditPrivatePosts,
                ReadPrivatePosts,
                EditPages,
                EditOthersPages,
                EditPublishedPages,
                PublishPages,
                DeletePages,
                DeleteOthersPages,
                DeletePublishedPages,
                DeletePrivatePages,
                EditPrivatePages,
                ReadPrivatePages,
                ListUsers,
                CreateUsers,
                EditUsers,
                DeleteUsers,
                PromoteUsers,
                EditThemeOptions,
                SwitchThemes,
                EditThemes,
                DeleteThemes,
                InstallThemes,
                ActivatePlugins,
                EditPlugins,
                InstallPlugins,
                DeletePlugins,
                ManageOptions,
                ManageWooCommerce,
                ManageLinks,
                ManageCategories,
                ModerateComments,
                UploadFiles,
                Import,
                Export,
                Unfiltered,
                Read,
            ]),
            Role::ShopManager => HashSet::from([
                EditPosts,
                EditOthersPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeleteOthersPosts,
                DeletePublishedPosts,
                EditPages,
                EditPublishedPages,
                PublishPages,
                DeletePages,
                ManageCategories,
                ModerateComments,
                UploadFiles,
                ManageWooCommerce,
                Read,
            ]),
            Role::Editor => HashSet::from([
                EditPosts,
                EditOthersPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeleteOthersPosts,
                DeletePublishedPosts,
                DeletePrivatePosts,
                EditPrivatePosts,
                ReadPrivatePosts,
                EditPages,
                EditOthersPages,
                EditPublishedPages,
                PublishPages,
                DeletePages,
                DeleteOthersPages,
                DeletePublishedPages,
                DeletePrivatePages,
                EditPrivatePages,
                ReadPrivatePages,
                ManageLinks,
                ManageCategories,
                ModerateComments,
                UploadFiles,
                Unfiltered,
                Read,
            ]),
            Role::Author => HashSet::from([
                EditPosts,
                EditPublishedPosts,
                PublishPosts,
                DeletePosts,
                DeletePublishedPosts,
                UploadFiles,
                Read,
            ]),
            Role::Contributor => HashSet::from([EditPosts, DeletePosts, Read]),
            Role::Subscriber => HashSet::from([Read]),
        }
    }

    /// Check if this role has a specific capability.
    pub fn can(&self, capability: &Capability) -> bool {
        self.capabilities().contains(capability)
    }

    /// WordPress `wp_user_level` for this role. Shop managers sit under administrators.
    pub fn user_level(&self) -> &'static str {
        match self {
            Self::Administrator => "10",
            Self::ShopManager => "8",
            Self::Editor => "7",
            Self::Author => "2",
            Self::Contributor => "1",
            Self::Subscriber => "0",
        }
    }
}

/// Check if a user (identified by role string) has a capability.
pub fn current_user_can(role_str: &str, capability: &Capability) -> bool {
    Role::from_str(role_str)
        .map(|r| r.can(capability))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_has_all_caps() {
        let admin = Role::Administrator;
        assert!(admin.can(&Capability::ManageOptions));
        assert!(admin.can(&Capability::EditPosts));
        assert!(admin.can(&Capability::DeleteUsers));
    }

    #[test]
    fn test_capability_names_match_wordpress() {
        assert_eq!(
            Capability::from_name("manage_options"),
            Some(Capability::ManageOptions)
        );
        assert_eq!(
            Capability::from_name("unfiltered_html"),
            Some(Capability::Unfiltered)
        );
        assert!(Capability::from_name("not_a_cap").is_none());
        assert!(Role::Administrator.can(&Capability::from_name("edit_posts").unwrap()));
        assert!(!Role::Subscriber.can(&Capability::from_name("edit_posts").unwrap()));
    }

    #[test]
    fn test_subscriber_limited() {
        let sub = Role::Subscriber;
        assert!(sub.can(&Capability::Read));
        assert!(!sub.can(&Capability::EditPosts));
        assert!(!sub.can(&Capability::ManageOptions));
    }

    #[test]
    fn test_current_user_can() {
        assert!(current_user_can(
            "administrator",
            &Capability::ManageOptions
        ));
        assert!(!current_user_can("subscriber", &Capability::EditPosts));
        assert!(!current_user_can("unknown_role", &Capability::Read));
    }

    // --- Role::from_str ---

    #[test]
    fn test_from_str_administrator() {
        assert_eq!(Role::from_str("administrator"), Some(Role::Administrator));
    }

    #[test]
    fn test_from_str_editor() {
        assert_eq!(Role::from_str("editor"), Some(Role::Editor));
    }

    #[test]
    fn test_from_str_author() {
        assert_eq!(Role::from_str("author"), Some(Role::Author));
    }

    #[test]
    fn test_from_str_contributor() {
        assert_eq!(Role::from_str("contributor"), Some(Role::Contributor));
    }

    #[test]
    fn test_from_str_subscriber() {
        assert_eq!(Role::from_str("subscriber"), Some(Role::Subscriber));
    }

    #[test]
    fn test_from_str_unknown_returns_none() {
        assert_eq!(Role::from_str("superadmin"), None);
        assert_eq!(Role::from_str(""), None);
        assert_eq!(Role::from_str("ADMINISTRATOR"), None);
    }

    // --- Role::as_str ---

    #[test]
    fn test_as_str_round_trips() {
        for role in [
            Role::Administrator,
            Role::ShopManager,
            Role::Editor,
            Role::Author,
            Role::Contributor,
            Role::Subscriber,
        ] {
            let s = role.as_str();
            assert_eq!(Role::from_str(s), Some(role));
        }
    }

    // --- Administrator capabilities ---

    #[test]
    fn test_shop_manager_runs_the_store_only() {
        let manager = Role::ShopManager;
        assert!(manager.can(&Capability::ManageWooCommerce));
        assert!(manager.can(&Capability::EditOthersPosts));
        assert!(manager.can(&Capability::UploadFiles));
        assert!(!manager.can(&Capability::ManageOptions));
        assert!(!manager.can(&Capability::ActivatePlugins));
        assert!(!manager.can(&Capability::ListUsers));
        assert!(Role::Administrator.can(&Capability::ManageWooCommerce));
        assert_eq!(manager.user_level(), "8");
        assert_eq!(Role::from_str("shop_manager"), Some(Role::ShopManager));
    }

    #[test]
    fn test_admin_can_manage_options() {
        assert!(Role::Administrator.can(&Capability::ManageOptions));
    }

    #[test]
    fn test_admin_can_list_users() {
        assert!(Role::Administrator.can(&Capability::ListUsers));
    }

    #[test]
    fn test_admin_can_activate_plugins() {
        assert!(Role::Administrator.can(&Capability::ActivatePlugins));
    }

    #[test]
    fn test_admin_can_upload_files() {
        assert!(Role::Administrator.can(&Capability::UploadFiles));
    }

    #[test]
    fn test_admin_can_moderate_comments() {
        assert!(Role::Administrator.can(&Capability::ModerateComments));
    }

    // --- Editor capabilities ---

    #[test]
    fn test_editor_can_edit_others_posts() {
        assert!(Role::Editor.can(&Capability::EditOthersPosts));
    }

    #[test]
    fn test_editor_cannot_manage_options() {
        assert!(!Role::Editor.can(&Capability::ManageOptions));
    }

    #[test]
    fn test_editor_cannot_activate_plugins() {
        assert!(!Role::Editor.can(&Capability::ActivatePlugins));
    }

    #[test]
    fn test_editor_can_publish_posts() {
        assert!(Role::Editor.can(&Capability::PublishPosts));
    }

    #[test]
    fn test_editor_can_moderate_comments() {
        assert!(Role::Editor.can(&Capability::ModerateComments));
    }

    // --- Author capabilities ---

    #[test]
    fn test_author_can_edit_posts() {
        assert!(Role::Author.can(&Capability::EditPosts));
    }

    #[test]
    fn test_author_cannot_edit_others_posts() {
        assert!(!Role::Author.can(&Capability::EditOthersPosts));
    }

    #[test]
    fn test_author_cannot_manage_options() {
        assert!(!Role::Author.can(&Capability::ManageOptions));
    }

    #[test]
    fn test_author_can_upload_files() {
        assert!(Role::Author.can(&Capability::UploadFiles));
    }

    // --- Contributor capabilities ---

    #[test]
    fn test_contributor_can_edit_posts() {
        assert!(Role::Contributor.can(&Capability::EditPosts));
    }

    #[test]
    fn test_contributor_cannot_publish_posts() {
        assert!(!Role::Contributor.can(&Capability::PublishPosts));
    }

    #[test]
    fn test_contributor_cannot_upload_files() {
        assert!(!Role::Contributor.can(&Capability::UploadFiles));
    }

    #[test]
    fn test_contributor_can_read() {
        assert!(Role::Contributor.can(&Capability::Read));
    }

    // --- Subscriber capabilities ---

    #[test]
    fn test_subscriber_can_read() {
        assert!(Role::Subscriber.can(&Capability::Read));
    }

    #[test]
    fn test_subscriber_cannot_edit_posts() {
        assert!(!Role::Subscriber.can(&Capability::EditPosts));
    }

    #[test]
    fn test_subscriber_cannot_upload() {
        assert!(!Role::Subscriber.can(&Capability::UploadFiles));
    }

    #[test]
    fn test_subscriber_cannot_list_users() {
        assert!(!Role::Subscriber.can(&Capability::ListUsers));
    }

    // --- current_user_can edge cases ---

    #[test]
    fn test_current_user_can_editor_manage_categories() {
        assert!(current_user_can("editor", &Capability::ManageCategories));
    }

    #[test]
    fn test_current_user_can_author_edit_posts() {
        assert!(current_user_can("author", &Capability::EditPosts));
    }

    #[test]
    fn test_current_user_can_contributor_no_publish() {
        assert!(!current_user_can("contributor", &Capability::PublishPosts));
    }

    #[test]
    fn test_current_user_can_unknown_role_no_read() {
        assert!(!current_user_can("2fa_pending", &Capability::Read));
    }
}
