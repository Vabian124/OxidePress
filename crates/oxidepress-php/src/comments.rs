// Modified by Vabian124 on 2026-11-11 to let plugins read and write comments.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Comments a PHP plugin may attach to a content post.
//!
//! A missing post is refused. The default status is hold (`0`). `1`, `spam`,
//! and `trash` are the other stored statuses. Author and body are plain text.

use serde_json::{json, Value};

/// One comment a plugin wants to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDraft {
    pub post_id: u64,
    pub author: String,
    pub email: String,
    pub content: String,
    pub status: String,
    pub parent: u64,
}

/// One stored comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentRecord {
    pub id: u64,
    pub post_id: u64,
    pub author: String,
    pub email: String,
    pub content: String,
    pub status: String,
    pub parent: u64,
}

/// A list of comments for one post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentQuery {
    pub post_id: u64,
    pub status: String,
    pub limit: u64,
}

pub fn comment_status_allowed(status: &str) -> bool {
    matches!(status, "0" | "1" | "spam" | "trash")
}

pub fn comment_draft_from_value(value: Option<&Value>) -> Result<CommentDraft, String> {
    let Some(value) = value else {
        return Err("comment is missing".to_string());
    };
    let post_id = value.get("post_id").and_then(Value::as_u64).unwrap_or(0);
    if post_id == 0 {
        return Err("post is missing".to_string());
    }
    let author = plain(&text(value, "author"), 200);
    if author.is_empty() {
        return Err("comment author is empty".to_string());
    }
    let email = email(&text(value, "email"))?;
    let mut content = text(value, "content");
    content.retain(|ch| ch != '\0');
    let content = content.trim().to_string();
    if content.is_empty() || content.len() > 10_000 {
        return Err("comment content is not allowed".to_string());
    }
    let status = text(value, "status");
    let status = if status.is_empty() {
        "0".to_string()
    } else {
        status
    };
    if !comment_status_allowed(&status) {
        return Err("comment status is not allowed".to_string());
    }
    let parent = value.get("parent").and_then(Value::as_u64).unwrap_or(0);
    Ok(CommentDraft {
        post_id,
        author,
        email,
        content,
        status,
        parent,
    })
}

pub fn comment_query_from_value(value: Option<&Value>) -> Result<CommentQuery, String> {
    let empty = Value::Null;
    let value = value.unwrap_or(&empty);
    let post_id = value.get("post_id").and_then(Value::as_u64).unwrap_or(0);
    if post_id == 0 {
        return Err("post is missing".to_string());
    }
    let status = match text(value, "status").as_str() {
        "" | "approve" | "1" => "1",
        "hold" | "0" => "0",
        "spam" => "spam",
        "trash" => "trash",
        "all" => "all",
        _ => return Err("comment status is not allowed".to_string()),
    };
    let limit = value
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .clamp(1, 20);
    Ok(CommentQuery {
        post_id,
        status: status.to_string(),
        limit,
    })
}

pub fn comment_json(comment: &CommentRecord) -> Value {
    json!({
        "id": comment.id,
        "post_id": comment.post_id,
        "author": comment.author,
        "email": comment.email,
        "content": comment.content,
        "status": comment.status,
        "parent": comment.parent,
    })
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn plain(raw: &str, max: usize) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_control() {
            continue;
        }
        out.push(ch);
        if out.len() >= max {
            break;
        }
    }
    out.trim().to_string()
}

fn email(raw: &str) -> Result<String, String> {
    let email = plain(raw, 100);
    if email.is_empty() {
        return Ok(email);
    }
    let at = email.bytes().filter(|byte| *byte == b'@').count();
    if at == 1 && !email.contains(' ') {
        Ok(email)
    } else {
        Err("comment email is not allowed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn comment_drafts_hold_by_default_and_refuse_a_bad_status() {
        let draft = comment_draft_from_value(Some(&json!({
            "post_id": 4,
            "author": "Ada",
            "email": "ada@example.com",
            "content": "Hello",
        })))
        .unwrap();
        assert_eq!(draft.status, "0");
        assert_eq!(draft.email, "ada@example.com");
        assert!(comment_draft_from_value(Some(&json!({
            "post_id": 4,
            "author": "Ada",
            "content": "Hello",
            "status": "inherit",
        })))
        .is_err());
        assert!(comment_draft_from_value(Some(&json!({
            "post_id": 4,
            "author": "Ada",
            "email": "not-an-email",
            "content": "Hello",
        })))
        .is_err());
        let query = comment_query_from_value(Some(&json!({
            "post_id": 4,
            "status": "approve",
        })))
        .unwrap();
        assert_eq!(query.status, "1");
    }
}
