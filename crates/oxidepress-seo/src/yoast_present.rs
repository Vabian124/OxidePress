// Modified by Vabian124 on 2026-11-07 to present the public Yoast SEO head.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Public head output for an imported Yoast SEO configuration.
//!
//! Titles, robots, Open Graph, Twitter, and the schema graph follow the
//! free plugin's presenters. PHP is not executed.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::meta_tags::auto_generate_description;
use crate::yoast_options::{YoastGeneral, YoastSocial, YoastTitles};

/// The document title and the tags that follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoastHead {
    pub title: String,
    pub robots: String,
    pub markup: String,
}

/// One sitemap URL or sitemap-index entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoastSitemapLink {
    pub loc: String,
    pub lastmod: Option<String>,
}

/// A public post or page.
pub struct SingularSeo<'a> {
    pub post_type: &'a str,
    pub title: &'a str,
    pub excerpt: &'a str,
    pub content: &'a str,
    pub permalink: &'a str,
    pub site_name: &'a str,
    pub site_description: &'a str,
    pub home_url: &'a str,
    pub locale: &'a str,
    pub language: &'a str,
    pub author_name: &'a str,
    pub published: &'a str,
    pub modified: &'a str,
    pub featured_image: Option<&'a str>,
    pub primary_category: &'a str,
    pub meta: &'a HashMap<String, String>,
}

/// An archive, search, or 404 page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingKind {
    Category,
    Tag,
    Author,
    Date,
    Search,
    NotFound,
}

pub struct ListingSeo<'a> {
    pub kind: ListingKind,
    pub name: &'a str,
    pub permalink: &'a str,
    pub site_name: &'a str,
    pub site_description: &'a str,
    pub home_url: &'a str,
    pub locale: &'a str,
    pub language: &'a str,
}

struct Vars<'a> {
    title: &'a str,
    site_name: &'a str,
    site_description: &'a str,
    separator: &'a str,
    excerpt: &'a str,
    focus_kw: &'a str,
    primary_category: &'a str,
    term_title: &'a str,
    name: &'a str,
    date: &'a str,
    search_phrase: &'a str,
}

#[allow(clippy::too_many_arguments)]
pub fn present_front(
    titles: &YoastTitles,
    social: &YoastSocial,
    general: &YoastGeneral,
    site_name: &str,
    site_description: &str,
    home_url: &str,
    locale: &str,
    language: &str,
) -> YoastHead {
    let vars = Vars {
        title: site_name,
        site_name,
        site_description,
        separator: &titles.separator,
        excerpt: "",
        focus_kw: "",
        primary_category: "",
        term_title: "",
        name: "",
        date: "",
        search_phrase: "",
    };
    let title = expand(&titles.title_home, &vars);
    let description = expand(&titles.metadesc_home, &vars);
    let og_title = first_nonempty(&[&expand(&social.og_frontpage_title, &vars), &title]);
    let og_description = first_nonempty(&[
        &expand(&social.og_frontpage_desc, &vars),
        &description,
        site_description,
    ]);
    let image = first_nonempty(&[&social.og_frontpage_image, &social.og_default_image]);
    let schema = schema_graph(
        &title,
        "Home",
        home_url,
        home_url,
        language,
        site_name,
        site_description,
        titles,
        None,
    );
    YoastHead {
        robots: robots_content(true, true),
        markup: markup(
            general,
            social,
            site_name,
            &description,
            home_url,
            locale,
            "website",
            &og_title,
            &og_description,
            &image,
            "",
            "",
            "",
            &title,
            &og_description,
            &image,
            &schema,
        ),
        title,
    }
}

pub fn present_singular(
    titles: &YoastTitles,
    social: &YoastSocial,
    general: &YoastGeneral,
    source: &SingularSeo<'_>,
) -> YoastHead {
    let focus = meta(source.meta, "_yoast_wpseo_focuskw");
    let vars = Vars {
        title: source.title,
        site_name: source.site_name,
        site_description: source.site_description,
        separator: &titles.separator,
        excerpt: source.excerpt,
        focus_kw: &focus,
        primary_category: source.primary_category,
        term_title: "",
        name: source.author_name,
        date: "",
        search_phrase: "",
    };
    let custom_title = meta(source.meta, "_yoast_wpseo_title");
    let template = if source.post_type == "page" {
        &titles.title_page
    } else {
        &titles.title_post
    };
    let title = if custom_title.is_empty() {
        expand(template, &vars)
    } else {
        expand(&custom_title, &vars)
    };
    let custom_desc = meta(source.meta, "_yoast_wpseo_metadesc");
    let desc_template = if source.post_type == "page" {
        &titles.metadesc_page
    } else {
        &titles.metadesc_post
    };
    let description = if custom_desc.is_empty() {
        expand(desc_template, &vars)
    } else {
        expand(&custom_desc, &vars)
    };
    let type_noindex = if source.post_type == "page" {
        titles.noindex_page
    } else {
        titles.noindex_post
    };
    let noindex = index_flag(
        source
            .meta
            .get("_yoast_wpseo_meta-robots-noindex")
            .map(String::as_str),
        type_noindex,
    );
    let nofollow = source
        .meta
        .get("_yoast_wpseo_meta-robots-nofollow")
        .is_some_and(|value| value == "1");
    let canonical = {
        let custom = meta(source.meta, "_yoast_wpseo_canonical");
        if custom.is_empty() {
            source.permalink.to_string()
        } else {
            custom
        }
    };
    let og_title = {
        let custom = meta(source.meta, "_yoast_wpseo_opengraph-title");
        if custom.is_empty() {
            title.clone()
        } else {
            expand(&custom, &vars)
        }
    };
    let og_description = {
        let custom = meta(source.meta, "_yoast_wpseo_opengraph-description");
        if !custom.is_empty() {
            expand(&custom, &vars)
        } else if !description.is_empty() {
            description.clone()
        } else if !source.excerpt.trim().is_empty() {
            source.excerpt.trim().to_string()
        } else {
            auto_generate_description(source.content, 156)
        }
    };
    let image = {
        let custom = meta(source.meta, "_yoast_wpseo_opengraph-image");
        first_nonempty(&[
            &custom,
            source.featured_image.unwrap_or(""),
            &social.og_default_image,
        ])
    };
    let twitter_title = {
        let custom = meta(source.meta, "_yoast_wpseo_twitter-title");
        if custom.is_empty() {
            og_title.clone()
        } else {
            expand(&custom, &vars)
        }
    };
    let twitter_desc = {
        let custom = meta(source.meta, "_yoast_wpseo_twitter-description");
        if custom.is_empty() {
            og_description.clone()
        } else {
            expand(&custom, &vars)
        }
    };
    let twitter_image = {
        let custom = meta(source.meta, "_yoast_wpseo_twitter-image");
        if custom.is_empty() {
            image.clone()
        } else {
            custom
        }
    };
    let article = if source.post_type == "post" {
        Some(ArticleSchema {
            headline: source.title,
            published: source.published,
            modified: source.modified,
            author: source.author_name,
        })
    } else {
        None
    };
    let schema = schema_graph(
        &title,
        source.title,
        source.permalink,
        source.home_url,
        source.language,
        source.site_name,
        source.site_description,
        titles,
        article.as_ref(),
    );
    let og_type = if source.post_type == "post" {
        "article"
    } else {
        "website"
    };
    YoastHead {
        robots: robots_content(!noindex, !nofollow),
        markup: markup(
            general,
            social,
            source.site_name,
            &description,
            &canonical,
            source.locale,
            og_type,
            &og_title,
            &og_description,
            &image,
            if source.post_type == "post" {
                source.published
            } else {
                ""
            },
            if source.post_type == "post" {
                source.modified
            } else {
                ""
            },
            if source.post_type == "post" {
                source.author_name
            } else {
                ""
            },
            &twitter_title,
            &twitter_desc,
            &twitter_image,
            &schema,
        ),
        title,
    }
}

pub fn present_listing(
    titles: &YoastTitles,
    social: &YoastSocial,
    general: &YoastGeneral,
    source: &ListingSeo<'_>,
) -> YoastHead {
    let (template, noindex) = match source.kind {
        ListingKind::Category => (&titles.title_tax, titles.noindex_category),
        ListingKind::Tag => (&titles.title_tax, titles.noindex_tag),
        ListingKind::Author => (&titles.title_author, titles.noindex_author),
        ListingKind::Date => (&titles.title_archive, titles.noindex_archive),
        ListingKind::Search => (&titles.title_search, true),
        ListingKind::NotFound => (&titles.title_404, true),
    };
    let vars = Vars {
        title: source.name,
        site_name: source.site_name,
        site_description: source.site_description,
        separator: &titles.separator,
        excerpt: "",
        focus_kw: "",
        primary_category: "",
        term_title: source.name,
        name: source.name,
        date: source.name,
        search_phrase: source.name,
    };
    let title = expand(template, &vars);
    let schema = schema_graph(
        &title,
        source.name,
        source.permalink,
        source.home_url,
        source.language,
        source.site_name,
        source.site_description,
        titles,
        None,
    );
    YoastHead {
        robots: robots_content(!noindex, true),
        markup: markup(
            general,
            social,
            source.site_name,
            "",
            source.permalink,
            source.locale,
            "website",
            &title,
            "",
            &social.og_default_image,
            "",
            "",
            "",
            &title,
            "",
            &social.og_default_image,
            &schema,
        ),
        title,
    }
}

pub fn urlset(links: &[YoastSitemapLink]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    xml.push('\n');
    for link in links {
        xml.push_str("\t<url>\n");
        xml.push_str(&format!("\t\t<loc>{}</loc>\n", xml_escape(&link.loc)));
        if let Some(lastmod) = &link.lastmod {
            xml.push_str(&format!("\t\t<lastmod>{}</lastmod>\n", xml_escape(lastmod)));
        }
        xml.push_str("\t</url>\n");
    }
    xml.push_str("</urlset>\n");
    xml
}

pub fn sitemap_index(links: &[YoastSitemapLink]) -> String {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    xml.push('\n');
    xml.push_str(r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#);
    xml.push('\n');
    for link in links {
        xml.push_str("\t<sitemap>\n");
        xml.push_str(&format!("\t\t<loc>{}</loc>\n", xml_escape(&link.loc)));
        if let Some(lastmod) = &link.lastmod {
            xml.push_str(&format!("\t\t<lastmod>{}</lastmod>\n", xml_escape(lastmod)));
        }
        xml.push_str("\t</sitemap>\n");
    }
    xml.push_str("</sitemapindex>\n");
    xml
}

struct ArticleSchema<'a> {
    headline: &'a str,
    published: &'a str,
    modified: &'a str,
    author: &'a str,
}

#[allow(clippy::too_many_arguments)]
fn schema_graph(
    title: &str,
    crumb: &str,
    page_url: &str,
    home_url: &str,
    language: &str,
    site_name: &str,
    site_description: &str,
    titles: &YoastTitles,
    article: Option<&ArticleSchema<'_>>,
) -> String {
    let webpage_id = format!("{page_url}#webpage");
    let website_id = format!("{home_url}#website");
    let org_name = if titles.company_or_person == "person" && !titles.person_name.is_empty() {
        titles.person_name.clone()
    } else if !titles.company_name.is_empty() {
        titles.company_name.clone()
    } else {
        site_name.to_string()
    };
    let (org_type, org_type_slug) = if titles.company_or_person == "person" {
        ("Person", "person")
    } else {
        ("Organization", "organization")
    };
    let org_id = format!("{home_url}#{org_type_slug}");
    let mut graph = vec![
        json!({
            "@type": "WebPage",
            "@id": webpage_id,
            "url": page_url,
            "name": title,
            "isPartOf": {"@id": website_id},
            "breadcrumb": {"@id": format!("{page_url}#breadcrumb")},
            "inLanguage": language,
            "potentialAction": [{
                "@type": "ReadAction",
                "target": [page_url]
            }]
        }),
        json!({
            "@type": "BreadcrumbList",
            "@id": format!("{page_url}#breadcrumb"),
            "itemListElement": breadcrumb(home_url, page_url, crumb)
        }),
        json!({
            "@type": "WebSite",
            "@id": website_id,
            "url": home_url,
            "name": site_name,
            "description": site_description,
            "publisher": {"@id": org_id},
            "potentialAction": [{
                "@type": "SearchAction",
                "target": {
                    "@type": "EntryPoint",
                    "urlTemplate": format!("{home_url}?s={{search_term_string}}")
                },
                "query-input": {
                    "@type": "PropertyValueSpecification",
                    "valueRequired": true,
                    "valueName": "search_term_string"
                }
            }],
            "inLanguage": language
        }),
        json!({
            "@type": org_type,
            "@id": org_id,
            "name": org_name,
            "url": home_url
        }),
    ];
    if let Some(article) = article {
        graph.push(json!({
            "@type": "Article",
            "@id": format!("{page_url}#article"),
            "isPartOf": {"@id": webpage_id},
            "author": {"@type": "Person", "name": article.author},
            "headline": article.headline,
            "datePublished": article.published,
            "dateModified": article.modified,
            "mainEntityOfPage": {"@id": webpage_id},
            "inLanguage": language
        }));
    }
    let value = json!({
        "@context": "https://schema.org",
        "@graph": graph
    });
    let mut encoded = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    encoded = encoded.replace('<', "\\u003c");
    format!(r#"<script type="application/ld+json" class="yoast-schema-graph">{encoded}</script>"#)
}

fn breadcrumb(home_url: &str, page_url: &str, title: &str) -> Value {
    if page_url.trim_end_matches('/') == home_url.trim_end_matches('/') {
        json!([{
            "@type": "ListItem",
            "position": 1,
            "name": "Home",
            "item": home_url
        }])
    } else {
        json!([
            {
                "@type": "ListItem",
                "position": 1,
                "name": "Home",
                "item": home_url
            },
            {
                "@type": "ListItem",
                "position": 2,
                "name": title
            }
        ])
    }
}

#[allow(clippy::too_many_arguments)]
fn markup(
    general: &YoastGeneral,
    social: &YoastSocial,
    site_name: &str,
    description: &str,
    canonical: &str,
    locale: &str,
    og_type: &str,
    og_title: &str,
    og_description: &str,
    og_image: &str,
    published: &str,
    modified: &str,
    author: &str,
    twitter_title: &str,
    twitter_description: &str,
    twitter_image: &str,
    schema: &str,
) -> String {
    let mut lines = vec![format!(
        "<!-- This site is optimized with the Yoast SEO plugin v{} - https://yoast.com/product/yoast-seo-wordpress/ -->",
        escape_text(&general.version)
    )];
    if !description.is_empty() {
        lines.push(meta_name("description", description));
    }
    lines.push(format!(
        r#"<link rel="canonical" href="{}" />"#,
        escape_attr(canonical)
    ));
    push_verify(
        &mut lines,
        "google-site-verification",
        &general.googleverify,
    );
    push_verify(&mut lines, "msvalidate.01", &general.msverify);
    push_verify(&mut lines, "yandex-verification", &general.yandexverify);
    push_verify(&mut lines, "baidu-site-verification", &general.baiduverify);
    if social.opengraph {
        lines.push(meta_property("og:locale", locale));
        lines.push(meta_property("og:type", og_type));
        lines.push(meta_property("og:title", og_title));
        if !og_description.is_empty() {
            lines.push(meta_property("og:description", og_description));
        }
        lines.push(meta_property("og:url", canonical));
        lines.push(meta_property("og:site_name", site_name));
        if !published.is_empty() {
            lines.push(meta_property("article:published_time", published));
            lines.push(meta_property("article:modified_time", modified));
        }
        if !og_image.is_empty() {
            lines.push(meta_property("og:image", og_image));
        }
        if !social.facebook_site.is_empty() && og_type == "article" {
            lines.push(meta_property("article:publisher", &social.facebook_site));
        }
        if !author.is_empty() {
            lines.push(meta_name("author", author));
        }
    }
    if social.twitter {
        let card = if social.twitter_card_type.is_empty() {
            "summary_large_image"
        } else {
            &social.twitter_card_type
        };
        lines.push(meta_name("twitter:card", card));
        if !social.twitter_site.is_empty() {
            let handle = if social.twitter_site.starts_with('@') {
                social.twitter_site.clone()
            } else {
                format!("@{}", social.twitter_site)
            };
            lines.push(meta_name("twitter:site", &handle));
        }
        lines.push(meta_name("twitter:title", twitter_title));
        if !twitter_description.is_empty() {
            lines.push(meta_name("twitter:description", twitter_description));
        }
        if !twitter_image.is_empty() {
            lines.push(meta_name("twitter:image", twitter_image));
        }
    }
    lines.push(schema.to_string());
    lines.join("\n")
}

fn robots_content(index: bool, follow: bool) -> String {
    let index = if index { "index" } else { "noindex" };
    let follow = if follow { "follow" } else { "nofollow" };
    if index == "index" {
        format!("{index}, {follow}, max-image-preview:large, max-snippet:-1, max-video-preview:-1")
    } else {
        format!("{index}, {follow}")
    }
}

fn index_flag(meta_value: Option<&str>, type_default: bool) -> bool {
    match meta_value.map(str::trim) {
        Some("1") => true,
        Some("2") => false,
        _ => type_default,
    }
}

fn expand(template: &str, vars: &Vars<'_>) -> String {
    if template.is_empty() {
        return String::new();
    }
    let expanded = template
        .replace("%%title%%", vars.title)
        .replace("%%sitename%%", vars.site_name)
        .replace("%%sitedesc%%", vars.site_description)
        .replace("%%sep%%", vars.separator)
        .replace("%%page%%", "")
        .replace("%%excerpt%%", vars.excerpt)
        .replace("%%excerpt_only%%", vars.excerpt)
        .replace("%%focuskw%%", vars.focus_kw)
        .replace("%%primary_category%%", vars.primary_category)
        .replace("%%term_title%%", vars.term_title)
        .replace("%%name%%", vars.name)
        .replace("%%date%%", vars.date)
        .replace("%%searchphrase%%", vars.search_phrase);
    let collapsed = expanded.split_whitespace().collect::<Vec<_>>().join(" ");
    trim_separator(&collapsed, vars.separator)
}

fn trim_separator(value: &str, separator: &str) -> String {
    let mut text = value.trim().to_string();
    if separator.is_empty() {
        return text;
    }
    while text.starts_with(separator) {
        text = text[separator.len()..].trim().to_string();
    }
    while text.ends_with(separator) {
        text = text[..text.len() - separator.len()].trim().to_string();
    }
    text
}

fn meta(map: &HashMap<String, String>, key: &str) -> String {
    map.get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn first_nonempty(values: &[&str]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
}

fn push_verify(lines: &mut Vec<String>, name: &str, content: &str) {
    if !content.is_empty() {
        lines.push(meta_name(name, content));
    }
}

fn meta_name(name: &str, content: &str) -> String {
    format!(
        r#"<meta name="{}" content="{}" />"#,
        escape_attr(name),
        escape_attr(content)
    )
}

fn meta_property(name: &str, content: &str) -> String {
    format!(
        r#"<meta property="{}" content="{}" />"#,
        escape_attr(name),
        escape_attr(content)
    )
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_titles() -> YoastTitles {
        let mut titles = YoastTitles::default();
        titles.separator = "|".to_string();
        titles.separator_code = "sc-pipe".to_string();
        titles.title_home = "Linen Workshop".to_string();
        titles.metadesc_home = "Stoneware from a small workshop.".to_string();
        titles
    }

    #[test]
    fn homepage_uses_the_saved_title_and_yoast_robots() {
        let head = present_front(
            &sample_titles(),
            &YoastSocial::default(),
            &YoastGeneral::default(),
            "OxidePress",
            "Just another OxidePress site",
            "http://127.0.0.1:3000/",
            "en_US",
            "en-US",
        );
        assert_eq!(head.title, "Linen Workshop");
        assert_eq!(
            head.robots,
            "index, follow, max-image-preview:large, max-snippet:-1, max-video-preview:-1"
        );
        assert!(head.markup.contains("Stoneware from a small workshop."));
        assert!(head.markup.contains("yoast-schema-graph"));
        assert!(head.markup.contains("v28.8"));
        assert!(head.markup.contains("og:title"));
    }

    #[test]
    fn post_template_uses_the_separator_and_omits_an_empty_description() {
        let head = present_singular(
            &sample_titles(),
            &YoastSocial::default(),
            &YoastGeneral::default(),
            &SingularSeo {
                post_type: "post",
                title: "Hello OxidePress",
                excerpt: "",
                content: "<p>A short note about the mug.</p>",
                permalink: "http://127.0.0.1:3000/hello-oxidepress/",
                site_name: "OxidePress",
                site_description: "Just another OxidePress site",
                home_url: "http://127.0.0.1:3000/",
                locale: "en_US",
                language: "en-US",
                author_name: "Admin",
                published: "2026-11-07T12:00:00+00:00",
                modified: "2026-11-07T12:00:00+00:00",
                featured_image: None,
                primary_category: "",
                meta: &HashMap::new(),
            },
        );
        assert_eq!(head.title, "Hello OxidePress | OxidePress");
        assert!(!head.markup.contains("name=\"description\""));
        assert!(head
            .markup
            .contains("og:title\" content=\"Hello OxidePress | OxidePress\""));
        assert!(head.markup.contains("article:published_time"));
        assert!(head.markup.contains("A short note about the mug."));
        assert!(head.markup.contains("\"@type\":\"Article\""));
    }

    #[test]
    fn noindex_meta_drops_the_preview_directives() {
        let mut meta = HashMap::new();
        meta.insert(
            "_yoast_wpseo_meta-robots-noindex".to_string(),
            "1".to_string(),
        );
        let head = present_singular(
            &sample_titles(),
            &YoastSocial::default(),
            &YoastGeneral::default(),
            &SingularSeo {
                post_type: "post",
                title: "Hidden",
                excerpt: "",
                content: "",
                permalink: "http://127.0.0.1:3000/hidden/",
                site_name: "OxidePress",
                site_description: "",
                home_url: "http://127.0.0.1:3000/",
                locale: "en_US",
                language: "en-US",
                author_name: "",
                published: "",
                modified: "",
                featured_image: None,
                primary_category: "",
                meta: &meta,
            },
        );
        assert_eq!(head.robots, "noindex, follow");
        assert!(!head.robots.contains("max-snippet"));
    }

    #[test]
    fn search_and_date_titles_follow_the_official_templates() {
        let titles = sample_titles();
        let social = YoastSocial::default();
        let general = YoastGeneral::default();
        let search = present_listing(
            &titles,
            &social,
            &general,
            &ListingSeo {
                kind: ListingKind::Search,
                name: "mug",
                permalink: "http://127.0.0.1:3000/?s=mug",
                site_name: "OxidePress",
                site_description: "",
                home_url: "http://127.0.0.1:3000/",
                locale: "en_US",
                language: "en-US",
            },
        );
        assert_eq!(search.title, "You searched for mug | OxidePress");
        assert!(search.robots.starts_with("noindex"));
        let date = present_listing(
            &titles,
            &social,
            &general,
            &ListingSeo {
                kind: ListingKind::Date,
                name: "2026",
                permalink: "http://127.0.0.1:3000/2026/",
                site_name: "OxidePress",
                site_description: "",
                home_url: "http://127.0.0.1:3000/",
                locale: "en_US",
                language: "en-US",
            },
        );
        assert_eq!(date.title, "2026 | OxidePress");
        assert!(date.robots.starts_with("noindex"));
    }

    #[test]
    fn sitemap_lists_locations_without_priority() {
        let xml = urlset(&[YoastSitemapLink {
            loc: "http://127.0.0.1:3000/hello-oxidepress/".to_string(),
            lastmod: Some("2026-11-07T12:00:00+00:00".to_string()),
        }]);
        assert!(xml.contains("<loc>http://127.0.0.1:3000/hello-oxidepress/</loc>"));
        assert!(!xml.contains("priority"));
        assert!(!xml.contains("changefreq"));
    }
}
