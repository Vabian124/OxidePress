// Modified by Vabian124 on 2026-11-07 to present official Yoast SEO options.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
pub mod analysis;
pub mod meta_tags;
pub mod open_graph;
pub mod robots;
pub mod schema;
pub mod sitemap;
pub mod yoast_compat;
pub mod yoast_options;
pub mod yoast_present;

pub use analysis::{analyze, AnalysisInput, AnalysisResult, SeoRecommendation, SeoScore};
pub use meta_tags::{auto_generate_description, generate_meta_tags, generate_title, SeoMeta};
pub use open_graph::{generate_og_tags, generate_twitter_tags, OpenGraphData, TwitterCardData};
pub use robots::RobotsGenerator;
pub use schema::{
    generate_article_schema, generate_breadcrumb_schema, generate_website_schema, BreadcrumbItem,
    SchemaOrg,
};
pub use sitemap::{SitemapEntry, SitemapGenerator, SitemapUrl};
pub use yoast_compat::{YoastPostSeo, YoastSocialSettings};
