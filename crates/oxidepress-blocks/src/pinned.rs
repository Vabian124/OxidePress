// Modified by Vabian124 on 2026-11-07 to link page lists, breadcrumbs, and post navigation.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Blocks present in the pinned WordPress `block.json` list and absent from
//! the older registry. Names that already exist are left untouched.

use std::sync::Arc;

use crate::context::{self, current};
use crate::parser::Block;
use crate::registry::{BlockCategory, BlockRegistry, BlockType, RenderCallback};

pub fn register_pinned_blocks(registry: &mut BlockRegistry) {
    register_dynamic(registry);
    for (name, title, category) in PINNED_STATIC {
        if registry.has_block_type(name) {
            continue;
        }
        registry.register_block_type(BlockType::new_static(
            name,
            title,
            category_from_slug(category),
            name.trim_start_matches("core/"),
        ));
    }
}

const PINNED_STATIC: &[(&str, &str, &str)] = &[
    ("core/accordion", "Accordion", "design"),
    ("core/accordion-heading", "Accordion Heading", "design"),
    ("core/accordion-item", "Accordion Item", "design"),
    ("core/accordion-panel", "Accordion Panel", "design"),
    ("core/block", "Pattern", "reusable"),
    ("core/comment-author-name", "Comment Author Name", "theme"),
    ("core/comment-content", "Comment Content", "theme"),
    ("core/comment-date", "Comment Date", "theme"),
    ("core/comment-edit-link", "Comment Edit Link", "theme"),
    ("core/comment-reply-link", "Comment Reply Link", "theme"),
    ("core/comments", "Comments", "theme"),
    ("core/comments-pagination", "Comments Pagination", "theme"),
    (
        "core/comments-pagination-next",
        "Comments Next Page",
        "theme",
    ),
    (
        "core/comments-pagination-numbers",
        "Comments Page Numbers",
        "theme",
    ),
    (
        "core/comments-pagination-previous",
        "Comments Previous Page",
        "theme",
    ),
    ("core/comments-title", "Comments Title", "theme"),
    ("core/comment-template", "Comment Template", "design"),
    ("core/details", "Details", "text"),
    ("core/footnotes", "Footnotes", "text"),
    ("core/html", "Custom HTML", "widgets"),
    ("core/icon", "Icon", "media"),
    ("core/legacy-widget", "Legacy Widget", "widgets"),
    ("core/list-item", "List Item", "text"),
    ("core/math", "Math", "text"),
    ("core/missing", "Unsupported", "text"),
    (
        "core/navigation-overlay-close",
        "Navigation Overlay Close",
        "design",
    ),
    ("core/page-list-item", "Page List Item", "widgets"),
    ("core/pattern", "Pattern Placeholder", "theme"),
    ("core/playlist", "Playlist", "media"),
    ("core/playlist-track", "Playlist track", "media"),
    ("core/post-author-biography", "Author Biography", "theme"),
    ("core/post-comments-form", "Comments Form", "theme"),
    ("core/post-template", "Post Template", "theme"),
    ("core/query-no-results", "No Results", "theme"),
    ("core/query-pagination", "Pagination", "theme"),
    ("core/query-pagination-numbers", "Page Numbers", "theme"),
    ("core/query-title", "Query Title", "theme"),
    ("core/social-links", "Social Icons", "widgets"),
    ("core/table", "Table", "text"),
    ("core/tab-list", "Tab List", "design"),
    ("core/tab-panel", "Tab Panel", "design"),
    ("core/tab-panels", "Tab Panels", "design"),
    ("core/tabs", "Tabs", "design"),
    ("core/term-count", "Term Count", "theme"),
    ("core/term-description", "Term Description", "theme"),
    ("core/term-name", "Term Name", "theme"),
    ("core/terms-query", "Terms Query", "theme"),
    ("core/term-template", "Term Template", "theme"),
    ("core/text-columns", "Text Columns (deprecated)", "design"),
    ("core/widget-group", "Widget Group", "widgets"),
];

fn category_from_slug(slug: &str) -> BlockCategory {
    match slug {
        "text" => BlockCategory::Text,
        "media" => BlockCategory::Media,
        "widgets" => BlockCategory::Widgets,
        "theme" => BlockCategory::Theme,
        "embed" => BlockCategory::Embed,
        _ => BlockCategory::Design,
    }
}

fn attr<'a>(block: &'a Block, key: &str) -> &'a str {
    block
        .attrs
        .get(key)
        .and_then(|value| value.as_str())
        .unwrap_or("")
}

fn register_dynamic(registry: &mut BlockRegistry) {
    let dynamic: &[(&str, &str, &str, RenderCallback)] = &[
        ("core/avatar", "Avatar", "theme", render_avatar()),
        (
            "core/breadcrumbs",
            "Breadcrumbs",
            "theme",
            render_breadcrumbs(),
        ),
        ("core/home-link", "Home Link", "design", render_home_link()),
        ("core/loginout", "Login/out", "theme", render_loginout()),
        ("core/more", "More", "design", render_more()),
        (
            "core/navigation-link",
            "Custom Link",
            "design",
            render_navigation_link(),
        ),
        (
            "core/navigation-submenu",
            "Submenu",
            "design",
            render_navigation_submenu(),
        ),
        ("core/nextpage", "Page Break", "design", render_nextpage()),
        ("core/page-list", "Page List", "widgets", render_page_list()),
        (
            "core/post-author-name",
            "Author Name",
            "theme",
            render_post_author_name(),
        ),
        (
            "core/post-comments-count",
            "Comments Count",
            "theme",
            render_comments_count(),
        ),
        (
            "core/post-comments-link",
            "Comments Link",
            "theme",
            render_comments_link(),
        ),
        (
            "core/post-navigation-link",
            "Post Navigation Link",
            "theme",
            render_post_navigation_link(),
        ),
        (
            "core/post-time-to-read",
            "Time to Read",
            "theme",
            render_time_to_read(),
        ),
        (
            "core/query-pagination-next",
            "Next Page",
            "theme",
            render_page_link("Next Page"),
        ),
        (
            "core/query-pagination-previous",
            "Previous Page",
            "theme",
            render_page_link("Previous Page"),
        ),
        (
            "core/query-total",
            "Query Total",
            "theme",
            render_query_total(),
        ),
        ("core/read-more", "Read More", "theme", render_read_more()),
        (
            "core/site-tagline",
            "Site Tagline",
            "theme",
            render_site_tagline(),
        ),
        (
            "core/social-link",
            "Social Icon",
            "widgets",
            render_social_link(),
        ),
    ];
    for (name, title, category, callback) in dynamic {
        registry.register_block_type(BlockType::new_dynamic(
            name,
            title,
            category_from_slug(category),
            name.trim_start_matches("core/"),
            Arc::clone(callback),
        ));
    }
}

fn render_navigation_link() -> RenderCallback {
    Arc::new(|block: &Block| {
        let label = context::shown(attr(block, "label"), "Link");
        let href = context::safe_href(attr(block, "url"));
        format!(
            "<li class=\"wp-block-navigation-item wp-block-navigation-link\"><a class=\"wp-block-navigation-item__content\" href=\"{href}\">{label}</a></li>"
        )
    })
}

fn render_navigation_submenu() -> RenderCallback {
    Arc::new(|block: &Block| {
        let label = context::shown(attr(block, "label"), "Submenu");
        let href = context::safe_href(attr(block, "url"));
        let inner = crate::core_blocks::render_inner_blocks_simple(&block.inner_blocks);
        format!(
            "<li class=\"wp-block-navigation-item has-child wp-block-navigation-submenu\"><a class=\"wp-block-navigation-item__content\" href=\"{href}\">{label}</a><ul class=\"wp-block-navigation__submenu-container\">{inner}</ul></li>"
        )
    })
}

fn render_social_link() -> RenderCallback {
    Arc::new(|block: &Block| {
        let service = attr(block, "service");
        let service_class = context::escape_html(service);
        let label = context::shown(
            attr(block, "label"),
            if service.is_empty() {
                "Social"
            } else {
                service
            },
        );
        let href = context::safe_href(attr(block, "url"));
        format!(
            "<li class=\"wp-social-link wp-social-link-{service_class} wp-block-social-link\"><a href=\"{href}\" class=\"wp-block-social-link-anchor\">{label}</a></li>"
        )
    })
}

fn render_site_tagline() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = class_name("wp-block-site-tagline", block);
        let tagline = context::shown(&current().site_tagline, "Just another OxidePress site");
        format!("<p class=\"{class}\">{tagline}</p>")
    })
}

fn render_home_link() -> RenderCallback {
    Arc::new(|block: &Block| {
        let ctx = current();
        let label = context::shown(
            attr(block, "label"),
            if ctx.site_title.is_empty() {
                "Home"
            } else {
                &ctx.site_title
            },
        );
        let href = context::safe_href(if ctx.site_url.is_empty() {
            "/"
        } else {
            &ctx.site_url
        });
        format!(
            "<div class=\"wp-block-home-link\"><a href=\"{href}\" rel=\"home\">{label}</a></div>"
        )
    })
}

fn render_read_more() -> RenderCallback {
    Arc::new(|block: &Block| {
        let ctx = current();
        let content = block.inner_html.trim();
        let label = if content.is_empty() {
            context::shown(attr(block, "content"), "Read more")
        } else {
            context::escape_html(content)
        };
        let href = context::safe_href(if ctx.post_url.is_empty() {
            "#"
        } else {
            &ctx.post_url
        });
        format!("<a class=\"wp-block-read-more\" href=\"{href}\">{label}</a>")
    })
}

fn render_loginout() -> RenderCallback {
    Arc::new(|_block: &Block| {
        "<div class=\"wp-block-loginout\"><a href=\"/wp-login.php\">Log in</a></div>".to_string()
    })
}

fn render_more() -> RenderCallback {
    Arc::new(|_block: &Block| {
        let id = current().post_id;
        format!("<span id=\"more-{id}\" class=\"wp-block-more\"></span>")
    })
}

fn render_nextpage() -> RenderCallback {
    Arc::new(|_block: &Block| "<span class=\"wp-block-nextpage\"></span>".to_string())
}

fn render_page_list() -> RenderCallback {
    Arc::new(|_block: &Block| {
        let ctx = current();
        let home = context::safe_href(if ctx.site_url.is_empty() {
            "/"
        } else {
            &ctx.site_url
        });
        let mut items =
            format!("<li class=\"wp-block-pages-list__item\"><a href=\"{home}\">Home</a></li>");
        for page in &ctx.pages {
            let href = context::safe_href(&page.url);
            let label = context::escape_html(&page.label);
            items.push_str(&format!(
                "<li class=\"wp-block-pages-list__item\"><a href=\"{href}\">{label}</a></li>"
            ));
        }
        format!("<ul class=\"wp-block-page-list\">{items}</ul>")
    })
}

fn render_post_author_name() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = class_name("wp-block-post-author-name", block);
        let author = context::shown(&current().post_author, "Author");
        format!("<div class=\"{class}\">{author}</div>")
    })
}

fn render_comments_count() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = class_name("wp-block-post-comments-count", block);
        let count = current().comment_count;
        format!("<div class=\"{class}\">{count} comments</div>")
    })
}

fn render_comments_link() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = class_name("wp-block-post-comments-link", block);
        let ctx = current();
        let href = context::safe_href(&format!("{}#comments", ctx.post_url.trim_end_matches('/')));
        let count = ctx.comment_count;
        format!("<div class=\"{class}\"><a href=\"{href}\">{count} comments</a></div>")
    })
}

fn render_post_navigation_link() -> RenderCallback {
    Arc::new(|block: &Block| {
        let kind = attr(block, "type");
        let ctx = current();
        let (url, fallback) = if kind == "previous" {
            (ctx.previous_url.as_str(), "Previous")
        } else {
            (ctx.next_url.as_str(), "Next")
        };
        if url.is_empty() {
            return String::new();
        }
        let label = context::shown(attr(block, "label"), fallback);
        let href = context::safe_href(url);
        format!("<div class=\"wp-block-post-navigation-link\"><a href=\"{href}\">{label}</a></div>")
    })
}

fn render_time_to_read() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = class_name("wp-block-post-time-to-read", block);
        let words = current().content_words;
        let minutes = if words == 0 { 1 } else { (words / 200).max(1) };
        format!("<div class=\"{class}\">{minutes} minute read</div>")
    })
}

fn render_page_link(fallback: &'static str) -> RenderCallback {
    Arc::new(move |block: &Block| {
        let label = if block.inner_html.trim().is_empty() {
            fallback.to_string()
        } else {
            context::escape_html(block.inner_html.trim())
        };
        let slug = block.name.trim_start_matches("core/");
        format!("<a class=\"wp-block-{slug}\" href=\"#\">{label}</a>")
    })
}

fn render_query_total() -> RenderCallback {
    Arc::new(|_block: &Block| {
        let ctx = current();
        let count = if ctx.query_total > 0 {
            ctx.query_total
        } else {
            ctx.query_posts.len()
        };
        format!("<div class=\"wp-block-query-total\">{count}</div>")
    })
}

fn render_breadcrumbs() -> RenderCallback {
    Arc::new(|_block: &Block| {
        let ctx = current();
        let home = context::safe_href(if ctx.site_url.is_empty() {
            "/"
        } else {
            &ctx.site_url
        });
        let mut html = format!(
            "<nav class=\"wp-block-breadcrumbs\" aria-label=\"Breadcrumbs\"><a href=\"{home}\">Home</a>"
        );
        if !ctx.post_title.is_empty() {
            let href = context::safe_href(if ctx.post_url.is_empty() {
                "#"
            } else {
                &ctx.post_url
            });
            let title = context::escape_html(&ctx.post_title);
            html.push_str(&format!(
                " <span aria-hidden=\"true\">/</span> <a href=\"{href}\">{title}</a>"
            ));
        }
        html.push_str("</nav>");
        html
    })
}

fn render_avatar() -> RenderCallback {
    Arc::new(|block: &Block| {
        let class = class_name("wp-block-avatar", block);
        let alt = context::shown(&current().post_author, "");
        format!(
            "<div class=\"{class}\"><img class=\"avatar avatar-96 photo\" alt=\"{alt}\" src=\"\" width=\"96\" height=\"96\" /></div>"
        )
    })
}

fn class_name(base: &str, block: &Block) -> String {
    match attr(block, "className") {
        "" => base.to_string(),
        extra => format!("{base} {}", context::escape_html(extra)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{with_render_context, RenderContext};
    use crate::parser::parse_blocks;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct GapReport {
        wordpress_blocks: Vec<GapBlock>,
    }

    #[derive(Debug, Deserialize)]
    struct GapBlock {
        name: String,
    }

    #[test]
    fn pinned_registry_covers_wordpress_71_blocks() {
        let report: GapReport =
            serde_json::from_str(include_str!("../../../gaps/blocks.json")).expect("blocks gap");
        let mut registry = BlockRegistry::new();
        crate::core_blocks::register_core_blocks(&mut registry);
        for block in &report.wordpress_blocks {
            assert!(
                registry.has_block_type(&block.name),
                "missing {}",
                block.name
            );
        }
    }

    #[test]
    fn navigation_link_renders_label_and_rejects_script_urls() {
        let renderer = crate::create_default_renderer();
        let blocks = parse_blocks(
            r#"<!-- wp:navigation-link {"label":"About","url":"/about"} /--><!-- wp:navigation-link {"label":"Bad","url":"javascript:alert(1)"} /-->"#,
        );
        let html = renderer.render_blocks(&blocks);
        assert!(html.contains("About"));
        assert!(html.contains("href=\"/about\""));
        assert!(html.contains("href=\"#\""));
        assert!(!html.contains("javascript:"));
    }

    #[test]
    fn post_title_and_tagline_use_the_render_context() {
        let renderer = crate::create_default_renderer();
        let blocks = parse_blocks("<!-- wp:post-title /--><!-- wp:site-tagline /-->");
        let ctx = RenderContext {
            post_title: "Hello Oxide".to_string(),
            site_tagline: "A Rust site".to_string(),
            ..RenderContext::default()
        };
        let html = with_render_context(&ctx, || renderer.render_blocks(&blocks));
        assert!(html.contains("Hello Oxide"));
        assert!(html.contains("A Rust site"));
        assert!(!html.contains(">Post Title<"));
    }

    #[test]
    fn accordion_keeps_its_wrapper_and_children() {
        let renderer = crate::create_default_renderer();
        let blocks = parse_blocks(
            "<!-- wp:accordion --><!-- wp:accordion-item --><!-- wp:paragraph --><p>Inside</p><!-- /wp:paragraph --><!-- /wp:accordion-item --><!-- /wp:accordion -->",
        );
        let html = renderer.render_blocks(&blocks);
        assert!(html.contains("wp-block-accordion"));
        assert!(html.contains("Inside"));
    }
}
