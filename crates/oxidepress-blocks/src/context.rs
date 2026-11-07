// Modified by Vabian124 on 2026-11-07 to give query, page, and navigation blocks their posts.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Values a dynamic block can read while a post or template is rendering.
//!
//! The server sets this for the duration of one render. Blocks rendered
//! without a context keep their previous placeholder text.

use std::cell::RefCell;

#[derive(Clone, Debug, Default)]
pub struct QueryPost {
    pub id: u64,
    pub title: String,
    pub excerpt: String,
    pub date: String,
    pub date_iso: String,
    pub author: String,
    pub url: String,
    pub featured_image_url: String,
    pub comment_count: i64,
}

#[derive(Clone, Debug, Default)]
pub struct NavLink {
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, Default)]
pub struct RenderContext {
    pub site_title: String,
    pub site_tagline: String,
    pub site_url: String,
    pub post_id: u64,
    pub post_title: String,
    pub post_excerpt: String,
    pub post_date: String,
    pub post_date_iso: String,
    pub post_author: String,
    pub post_url: String,
    pub featured_image_url: String,
    pub comment_count: i64,
    pub content_words: u32,
    pub query_posts: Vec<QueryPost>,
    pub query_total: usize,
    pub pages: Vec<NavLink>,
    pub previous_url: String,
    pub next_url: String,
}

std::thread_local! {
    static RENDER_CONTEXT: RefCell<RenderContext> = RefCell::new(RenderContext::default());
}

pub fn with_render_context<T>(ctx: &RenderContext, render: impl FnOnce() -> T) -> T {
    RENDER_CONTEXT.with(|slot| {
        let previous = slot.replace(ctx.clone());
        let result = render();
        slot.replace(previous);
        result
    })
}

pub(crate) fn current() -> RenderContext {
    RENDER_CONTEXT.with(|slot| slot.borrow().clone())
}

pub(crate) fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn shown(value: &str, fallback: &str) -> String {
    escape_html(if value.is_empty() { fallback } else { value })
}

/// Allow site-relative, fragment, and http(s)/mailto links. Anything else
/// becomes `#` so a block attribute cannot inject a script URL.
pub(crate) fn safe_href(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "#".to_string();
    }
    let lower = trimmed.to_ascii_lowercase();
    let allowed_scheme = lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:");
    if trimmed.contains(':') && !allowed_scheme {
        return "#".to_string();
    }
    escape_html(trimmed)
}
