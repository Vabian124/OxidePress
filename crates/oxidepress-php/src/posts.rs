// Modified by Vabian124 on 2026-11-09 to let plugins read and write posts.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Post rows a PHP plugin may read or write.
//!
//! Internal WordPress types such as revisions and templates are refused.
//! Titles and excerpts are plain text. Content is capped before the host stores it.

use serde_json::{json, Value};

use crate::worker_token;

const DENIED_TYPES: &[&str] = &[
    "revision",
    "nav_menu_item",
    "custom_css",
    "customize_changeset",
    "oembed_cache",
    "user_request",
    "wp_block",
    "wp_template",
    "wp_template_part",
    "wp_global_styles",
    "wp_navigation",
    "wp_font_family",
    "wp_font_face",
];

const STATUSES: &[&str] = &["publish", "draft", "pending", "private", "future", "trash"];

/// One `wp_posts` row returned to PHP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostRecord {
    pub id: u64,
    pub author: u64,
    pub date: String,
    pub content: String,
    pub title: String,
    pub excerpt: String,
    pub status: String,
    pub name: String,
    pub post_type: String,
    pub parent: u64,
    pub guid: String,
    pub menu_order: i32,
    pub comment_count: i64,
}

/// Fields a plugin may set on insert or update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostDraft {
    pub title: String,
    pub content: String,
    pub excerpt: String,
    pub status: String,
    pub name: String,
    pub post_type: String,
    pub parent: u64,
}

/// A bounded `get_posts` query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostQuery {
    pub post_type: String,
    pub status: String,
    pub name: String,
    pub search: String,
    pub limit: u64,
}

/// A post type reported by `register_post_type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredPostType {
    pub name: String,
    pub label: String,
    pub singular: String,
    pub public: bool,
    pub show_ui: bool,
    pub show_in_menu: bool,
}

/// Content types a plugin may create. `post` and `page` pass. Revisions do not.
pub fn content_type_allowed(name: &str) -> bool {
    worker_token(name, 20) && !DENIED_TYPES.contains(&name)
}

pub fn record_json(post: &PostRecord) -> Value {
    json!({
        "id": post.id,
        "author": post.author,
        "date": post.date,
        "content": post.content,
        "title": post.title,
        "excerpt": post.excerpt,
        "status": post.status,
        "name": post.name,
        "type": post.post_type,
        "parent": post.parent,
        "guid": post.guid,
        "menu_order": post.menu_order,
        "comment_count": post.comment_count,
    })
}

pub fn draft_from_value(value: Option<&Value>) -> Result<PostDraft, String> {
    let Some(value) = value else {
        return Err("post is missing".to_string());
    };
    let post_type = text(value, "type");
    let post_type = if post_type.is_empty() {
        "post".to_string()
    } else {
        post_type
    };
    if !content_type_allowed(&post_type) {
        return Err("post type is not allowed".to_string());
    }
    let status = text(value, "status");
    let status = if status.is_empty() {
        "draft".to_string()
    } else {
        status
    };
    if !STATUSES.contains(&status.as_str()) {
        return Err("post status is not allowed".to_string());
    }
    let title = plain_text(&text(value, "title"), 500);
    let excerpt = plain_text(&text(value, "excerpt"), 2_000);
    let mut content = text(value, "content");
    if content.len() > 200_000 {
        content.truncate(200_000);
    }
    content.retain(|ch| ch != '\0');
    let name = slug(&text(value, "name"), &title);
    let parent = value.get("parent").and_then(Value::as_u64).unwrap_or(0);
    Ok(PostDraft {
        title,
        content,
        excerpt,
        status,
        name,
        post_type,
        parent,
    })
}

pub fn query_from_value(value: Option<&Value>) -> Result<PostQuery, String> {
    let empty = Value::Null;
    let value = value.unwrap_or(&empty);
    let post_type = text(value, "type");
    let post_type = if post_type.is_empty() {
        "post".to_string()
    } else {
        post_type
    };
    if !content_type_allowed(&post_type) {
        return Err("post type is not allowed".to_string());
    }
    let status = text(value, "status");
    let status = if status.is_empty() {
        "publish".to_string()
    } else {
        status
    };
    if !STATUSES.contains(&status.as_str()) {
        return Err("post status is not allowed".to_string());
    }
    let mut search = plain_text(&text(value, "search"), 80);
    search.retain(|ch| ch != '%' && ch != '_');
    let limit = value
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 20);
    Ok(PostQuery {
        post_type,
        status,
        name: slug(&text(value, "name"), ""),
        search,
        limit,
    })
}

pub fn type_from_value(value: &Value) -> Option<RegisteredPostType> {
    let name = text(value, "name");
    if !content_type_allowed(&name) {
        return None;
    }
    let label = plain_text(&text(value, "label"), 80);
    let label = if label.is_empty() {
        name.clone()
    } else {
        label
    };
    let singular = plain_text(&text(value, "singular"), 80);
    let singular = if singular.is_empty() {
        label.clone()
    } else {
        singular
    };
    Some(RegisteredPostType {
        name,
        label,
        singular,
        public: value
            .get("public")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        show_ui: value
            .get("show_ui")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        show_in_menu: value
            .get("show_in_menu")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .replace('\0', "")
}

fn plain_text(value: &str, max: usize) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for ch in value.chars() {
        if ch == '\0' {
            continue;
        }
        if ch == '<' {
            skipping = true;
            continue;
        }
        if ch == '>' && skipping {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push(ch);
        }
        if out.len() >= max {
            break;
        }
    }
    out.trim().to_string()
}

fn slug(explicit: &str, title: &str) -> String {
    let source = if explicit.is_empty() { title } else { explicit };
    let mut out = String::new();
    let mut dash = false;
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
        if out.len() >= 200 {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn revisions_and_templates_are_refused() {
        assert!(content_type_allowed("post"));
        assert!(content_type_allowed("event"));
        assert!(!content_type_allowed("revision"));
        assert!(!content_type_allowed("wp_template"));
        assert!(!content_type_allowed("nav_menu_item"));
        let revision = json!({"title": "Nope", "type": "revision", "status": "publish"});
        assert!(draft_from_value(Some(&revision)).is_err());
    }

    #[test]
    fn titles_lose_tags_and_slugs_come_from_the_title() {
        let draft = draft_from_value(Some(&json!({
            "title": "Launch<script>",
            "content": "Day",
            "type": "event",
            "status": "publish",
        })))
        .unwrap();
        assert_eq!(draft.title, "Launch");
        assert_eq!(draft.name, "launch");
        assert_eq!(draft.post_type, "event");
    }
}
