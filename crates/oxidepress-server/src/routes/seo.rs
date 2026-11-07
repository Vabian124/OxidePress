// Modified by Vabian124 on 2026-11-07 to serve the official Yoast sitemap index.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use std::sync::Arc;

use oxidepress_db::entities::{wp_postmeta, wp_posts};
use oxidepress_seo::sitemap::{ChangeFreq, SitemapGenerator, SitemapUrl};
use oxidepress_seo::yoast_present::{sitemap_index, urlset, YoastSitemapLink};
use oxidepress_themes::tags::PostTemplateData;
use std::collections::HashSet;

use crate::state::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sitemap.xml", get(sitemap_xml))
        .route("/sitemap_index.xml", get(yoast_sitemap_index))
        .route("/post-sitemap.xml", get(yoast_post_sitemap))
        .route("/page-sitemap.xml", get(yoast_page_sitemap))
        .route("/robots.txt", get(robots_txt))
        .with_state(state)
}

async fn sitemap_xml(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .filter(
            wp_posts::Column::PostType
                .eq("post")
                .or(wp_posts::Column::PostType.eq("page")),
        )
        .order_by_desc(wp_posts::Column::PostModified)
        .all(&state.db)
        .await
        .unwrap_or_default();

    let mut generator = SitemapGenerator::new();

    // Add homepage
    generator.add_url(SitemapUrl {
        loc: state.site_url.clone(),
        lastmod: Some(chrono::Utc::now().format("%Y-%m-%d").to_string()),
        changefreq: Some(ChangeFreq::Daily),
        priority: Some(1.0),
    });

    let include_posts =
        super::plugin_admin::option_is_on(&state, "_yoast_wpseo_sitemap-posts").await;
    let include_pages =
        super::plugin_admin::option_is_on(&state, "_yoast_wpseo_sitemap-pages").await;

    for post in posts {
        if post.post_type == "post" && !include_posts {
            continue;
        }
        if post.post_type == "page" && !include_pages {
            continue;
        }
        let permalink = format!("{}/{}", state.site_url, post.post_name);
        let lastmod = post.post_modified_gmt.format("%Y-%m-%d").to_string();
        let (changefreq, priority) = if post.post_type == "page" {
            (ChangeFreq::Monthly, 0.8)
        } else {
            (ChangeFreq::Weekly, 0.6)
        };
        generator.add_url(SitemapUrl {
            loc: permalink,
            lastmod: Some(lastmod),
            changefreq: Some(changefreq),
            priority: Some(priority),
        });
    }

    let xml = generator.generate_xml();

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(xml)
        .unwrap()
        .into_response()
}

async fn robots_txt(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let yoast = super::yoast_store::active_yoast(&state).await;
    let root = state.site_url.trim_end_matches('/');
    let mut content =
        "User-agent: *\nDisallow: /wp-admin/\nAllow: /wp-admin/admin-ajax.php\n".to_string();
    if yoast.general.enable_xml_sitemap {
        content.push_str(&format!("\nSitemap: {root}/sitemap_index.xml\n"));
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(content)
        .unwrap()
        .into_response()
}

async fn yoast_sitemap_index(State(state): State<Arc<AppState>>) -> Response {
    let yoast = super::yoast_store::active_yoast(&state).await;
    if !yoast.general.enable_xml_sitemap {
        return StatusCode::NOT_FOUND.into_response();
    }
    let root = state.site_url.trim_end_matches('/');
    let mut links = Vec::new();
    if super::plugin_admin::option_is_on(&state, "_yoast_wpseo_sitemap-posts").await
        && !yoast.titles.noindex_post
    {
        links.push(YoastSitemapLink {
            loc: format!("{root}/post-sitemap.xml"),
            lastmod: None,
        });
    }
    if super::plugin_admin::option_is_on(&state, "_yoast_wpseo_sitemap-pages").await
        && !yoast.titles.noindex_page
    {
        links.push(YoastSitemapLink {
            loc: format!("{root}/page-sitemap.xml"),
            lastmod: None,
        });
    }
    xml_response(sitemap_index(&links))
}

async fn yoast_post_sitemap(State(state): State<Arc<AppState>>) -> Response {
    yoast_type_sitemap(state, "post", "_yoast_wpseo_sitemap-posts", false).await
}

async fn yoast_page_sitemap(State(state): State<Arc<AppState>>) -> Response {
    yoast_type_sitemap(state, "page", "_yoast_wpseo_sitemap-pages", true).await
}

async fn yoast_type_sitemap(
    state: Arc<AppState>,
    post_type: &str,
    checkbox: &str,
    include_home: bool,
) -> Response {
    let yoast = super::yoast_store::active_yoast(&state).await;
    let type_noindex = if post_type == "page" {
        yoast.titles.noindex_page
    } else {
        yoast.titles.noindex_post
    };
    if !yoast.general.enable_xml_sitemap
        || type_noindex
        || !super::plugin_admin::option_is_on(&state, checkbox).await
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let hidden = meta_ids(&state, "_yoast_wpseo_meta-robots-noindex", "1").await;
    let forced = meta_ids(&state, "_yoast_wpseo_meta-robots-noindex", "2").await;
    let posts = wp_posts::Entity::find()
        .filter(wp_posts::Column::PostStatus.eq("publish"))
        .filter(wp_posts::Column::PostType.eq(post_type))
        .order_by_desc(wp_posts::Column::PostModifiedGmt)
        .all(&state.db)
        .await
        .unwrap_or_default();
    let rewrite = state.rewrite_rules.read().await;
    let mut links = Vec::new();
    if include_home {
        links.push(YoastSitemapLink {
            loc: format!("{}/", state.site_url.trim_end_matches('/')),
            lastmod: posts.first().map(|post| {
                format!(
                    "{}+00:00",
                    post.post_modified_gmt.format("%Y-%m-%dT%H:%M:%S")
                )
            }),
        });
    }
    for post in posts {
        if hidden.contains(&post.id) && !forced.contains(&post.id) {
            continue;
        }
        let permalink =
            PostTemplateData::from_model_with_rewrite(&post, &state.site_url, &rewrite).permalink;
        links.push(YoastSitemapLink {
            loc: permalink,
            lastmod: Some(format!(
                "{}+00:00",
                post.post_modified_gmt.format("%Y-%m-%dT%H:%M:%S")
            )),
        });
    }
    drop(rewrite);
    xml_response(urlset(&links))
}

async fn meta_ids(state: &AppState, key: &str, value: &str) -> HashSet<u64> {
    wp_postmeta::Entity::find()
        .filter(wp_postmeta::Column::MetaKey.eq(key))
        .filter(wp_postmeta::Column::MetaValue.eq(value))
        .all(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.post_id)
        .collect()
}

fn xml_response(body: String) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(body)
        .unwrap()
        .into_response()
}
