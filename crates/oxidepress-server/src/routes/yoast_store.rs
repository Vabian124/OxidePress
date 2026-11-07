// Modified by Vabian124 on 2026-11-07 to import official Yoast SEO options.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Load `wpseo`, `wpseo_titles`, and `wpseo_social`.
//!
//! The first read copies the SEO settings screen's saved values into those
//! official keys. Later reads leave an imported blob alone.

use oxidepress_seo::yoast_options::{
    merge_php_map, separator_char, separator_code, YoastGeneral, YoastSocial, YoastTitles,
};

use crate::state::AppState;

#[derive(Debug, Clone)]
pub(crate) struct ActiveYoast {
    pub titles: YoastTitles,
    pub social: YoastSocial,
    pub general: YoastGeneral,
}

pub(crate) async fn active_yoast(state: &AppState) -> ActiveYoast {
    let titles_raw = option(state, "wpseo_titles").await;
    let social_raw = option(state, "wpseo_social").await;
    let general_raw = option(state, "wpseo").await;
    if titles_raw.is_some() {
        return ActiveYoast {
            titles: titles_raw
                .as_deref()
                .map(YoastTitles::parse)
                .unwrap_or_default(),
            social: social_raw
                .as_deref()
                .map(YoastSocial::parse)
                .unwrap_or_default(),
            general: general_raw
                .as_deref()
                .map(YoastGeneral::parse)
                .unwrap_or_default(),
        };
    }

    let mut titles = YoastTitles::default();
    let homepage_title = option(state, "_yoast_wpseo_title-homepage")
        .await
        .unwrap_or_default();
    if !homepage_title.trim().is_empty() {
        titles.title_home = homepage_title;
    }
    let homepage_desc = option(state, "_yoast_wpseo_metadesc-homepage")
        .await
        .unwrap_or_default();
    if !homepage_desc.trim().is_empty() {
        titles.metadesc_home = homepage_desc;
    }
    let separator = option(state, "_yoast_wpseo_separator")
        .await
        .unwrap_or_default();
    if !separator.trim().is_empty() {
        titles.separator_code = separator_code(separator.trim());
        titles.separator = separator_char(&titles.separator_code);
    }
    let post_title = option(state, "_yoast_wpseo_title-post")
        .await
        .unwrap_or_default();
    if !post_title.trim().is_empty() {
        titles.title_post = post_title;
    }
    let page_title = option(state, "_yoast_wpseo_title-page")
        .await
        .unwrap_or_default();
    if !page_title.trim().is_empty() {
        titles.title_page = page_title;
    }

    let mut social = YoastSocial::default();
    let opengraph = option(state, "_yoast_wpseo_opengraph").await;
    if opengraph.as_deref() == Some("0") {
        social.opengraph = false;
    }
    let image = option(state, "_yoast_wpseo_og_default_image")
        .await
        .unwrap_or_default();
    if !image.trim().is_empty() {
        social.og_default_image = image;
    }

    let general = YoastGeneral::default();
    let _ = state
        .options
        .update_option("wpseo_titles", &merge_php_map(None, &titles.pairs()))
        .await;
    let _ = state
        .options
        .update_option("wpseo_social", &merge_php_map(None, &social.pairs()))
        .await;
    let _ = state
        .options
        .update_option("wpseo", &merge_php_map(None, &general.pairs()))
        .await;
    ActiveYoast {
        titles,
        social,
        general,
    }
}

pub(crate) async fn save_yoast_settings(
    state: &AppState,
    titles: &YoastTitles,
    social: &YoastSocial,
) {
    let _ = active_yoast(state).await;
    let titles_raw = option(state, "wpseo_titles").await;
    let social_raw = option(state, "wpseo_social").await;
    let _ = state
        .options
        .update_option(
            "wpseo_titles",
            &merge_php_map(titles_raw.as_deref(), &titles.pairs()),
        )
        .await;
    let _ = state
        .options
        .update_option(
            "wpseo_social",
            &merge_php_map(social_raw.as_deref(), &social.pairs()),
        )
        .await;
}

async fn option(state: &AppState, key: &str) -> Option<String> {
    state.options.get_option(key).await.ok().flatten()
}
