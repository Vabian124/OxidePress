// Modified by Vabian124 on 2026-11-07 to read and write official Yoast SEO options.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Official Yoast SEO option blobs.
//!
//! WordPress stores these as PHP-serialized arrays in `wpseo_titles`,
//! `wpseo_social`, and `wpseo`. Missing keys use the Yoast SEO 28.8 defaults.

use oxidepress_core::{php_serialize, php_unserialize, PhpValue};

/// Current free Yoast SEO release these defaults follow.
pub const YOAST_VERSION: &str = "28.8";

/// Title templates and index flags from `wpseo_titles`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoastTitles {
    pub separator_code: String,
    pub separator: String,
    pub title_home: String,
    pub metadesc_home: String,
    pub title_post: String,
    pub title_page: String,
    pub metadesc_post: String,
    pub metadesc_page: String,
    pub title_tax: String,
    pub title_author: String,
    pub title_archive: String,
    pub title_search: String,
    pub title_404: String,
    pub noindex_post: bool,
    pub noindex_page: bool,
    pub noindex_category: bool,
    pub noindex_tag: bool,
    pub noindex_author: bool,
    pub noindex_archive: bool,
    pub company_or_person: String,
    pub company_name: String,
    pub person_name: String,
}

/// Social flags from `wpseo_social`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoastSocial {
    pub opengraph: bool,
    pub twitter: bool,
    pub og_default_image: String,
    pub og_frontpage_title: String,
    pub og_frontpage_desc: String,
    pub og_frontpage_image: String,
    pub twitter_card_type: String,
    pub facebook_site: String,
    pub twitter_site: String,
}

/// General flags from `wpseo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoastGeneral {
    pub enable_xml_sitemap: bool,
    pub googleverify: String,
    pub msverify: String,
    pub yandexverify: String,
    pub baiduverify: String,
    pub version: String,
}

impl Default for YoastTitles {
    fn default() -> Self {
        Self {
            separator_code: "sc-dash".to_string(),
            separator: "-".to_string(),
            title_home: "%%sitename%% %%page%% %%sep%% %%sitedesc%%".to_string(),
            metadesc_home: String::new(),
            title_post: "%%title%% %%page%% %%sep%% %%sitename%%".to_string(),
            title_page: "%%title%% %%page%% %%sep%% %%sitename%%".to_string(),
            metadesc_post: String::new(),
            metadesc_page: String::new(),
            title_tax: "%%term_title%% Archives %%page%% %%sep%% %%sitename%%".to_string(),
            title_author: "%%name%%, Author at %%sitename%% %%page%%".to_string(),
            title_archive: "%%date%% %%page%% %%sep%% %%sitename%%".to_string(),
            title_search: "You searched for %%searchphrase%% %%page%% %%sep%% %%sitename%%"
                .to_string(),
            title_404: "Page not found %%sep%% %%sitename%%".to_string(),
            noindex_post: false,
            noindex_page: false,
            noindex_category: false,
            noindex_tag: false,
            noindex_author: false,
            noindex_archive: true,
            company_or_person: "company".to_string(),
            company_name: String::new(),
            person_name: String::new(),
        }
    }
}

impl Default for YoastSocial {
    fn default() -> Self {
        Self {
            opengraph: true,
            twitter: true,
            og_default_image: String::new(),
            og_frontpage_title: String::new(),
            og_frontpage_desc: String::new(),
            og_frontpage_image: String::new(),
            twitter_card_type: "summary_large_image".to_string(),
            facebook_site: String::new(),
            twitter_site: String::new(),
        }
    }
}

impl Default for YoastGeneral {
    fn default() -> Self {
        Self {
            enable_xml_sitemap: true,
            googleverify: String::new(),
            msverify: String::new(),
            yandexverify: String::new(),
            baiduverify: String::new(),
            version: YOAST_VERSION.to_string(),
        }
    }
}

impl YoastTitles {
    pub fn parse(serialized: &str) -> Self {
        let mut titles = Self::default();
        let Ok(value) = php_unserialize(serialized) else {
            return titles;
        };
        if let Some(code) = php_string(&value, "separator") {
            titles.separator_code = code.clone();
            titles.separator = separator_char(&code);
        }
        copy_string(&value, "title-home-wpseo", &mut titles.title_home);
        copy_string(&value, "metadesc-home-wpseo", &mut titles.metadesc_home);
        copy_string(&value, "title-post", &mut titles.title_post);
        copy_string(&value, "title-page", &mut titles.title_page);
        copy_string(&value, "metadesc-post", &mut titles.metadesc_post);
        copy_string(&value, "metadesc-page", &mut titles.metadesc_page);
        copy_string(&value, "title-tax-category", &mut titles.title_tax);
        copy_string(&value, "title-author-wpseo", &mut titles.title_author);
        copy_string(&value, "title-archive-wpseo", &mut titles.title_archive);
        copy_string(&value, "title-search-wpseo", &mut titles.title_search);
        copy_string(&value, "title-404-wpseo", &mut titles.title_404);
        titles.noindex_post = php_bool(&value, "noindex-post", titles.noindex_post);
        titles.noindex_page = php_bool(&value, "noindex-page", titles.noindex_page);
        titles.noindex_category = php_bool(&value, "noindex-tax-category", titles.noindex_category);
        titles.noindex_tag = php_bool(&value, "noindex-tax-post_tag", titles.noindex_tag);
        titles.noindex_author = php_bool(&value, "noindex-author-wpseo", titles.noindex_author);
        titles.noindex_archive = php_bool(&value, "noindex-archive-wpseo", titles.noindex_archive);
        copy_string(&value, "company_or_person", &mut titles.company_or_person);
        copy_string(&value, "company_name", &mut titles.company_name);
        copy_string(&value, "person_name", &mut titles.person_name);
        titles
    }

    pub fn pairs(&self) -> Vec<(String, PhpValue)> {
        vec![
            ("separator".into(), text(&self.separator_code)),
            ("title-home-wpseo".into(), text(&self.title_home)),
            ("metadesc-home-wpseo".into(), text(&self.metadesc_home)),
            ("title-post".into(), text(&self.title_post)),
            ("title-page".into(), text(&self.title_page)),
            ("metadesc-post".into(), text(&self.metadesc_post)),
            ("metadesc-page".into(), text(&self.metadesc_page)),
            ("title-tax-category".into(), text(&self.title_tax)),
            ("title-tax-post_tag".into(), text(&self.title_tax)),
            ("title-author-wpseo".into(), text(&self.title_author)),
            ("title-archive-wpseo".into(), text(&self.title_archive)),
            ("title-search-wpseo".into(), text(&self.title_search)),
            ("title-404-wpseo".into(), text(&self.title_404)),
            ("noindex-post".into(), flag(self.noindex_post)),
            ("noindex-page".into(), flag(self.noindex_page)),
            ("noindex-tax-category".into(), flag(self.noindex_category)),
            ("noindex-tax-post_tag".into(), flag(self.noindex_tag)),
            ("noindex-author-wpseo".into(), flag(self.noindex_author)),
            ("noindex-archive-wpseo".into(), flag(self.noindex_archive)),
            ("company_or_person".into(), text(&self.company_or_person)),
            ("company_name".into(), text(&self.company_name)),
            ("person_name".into(), text(&self.person_name)),
        ]
    }
}

impl YoastSocial {
    pub fn parse(serialized: &str) -> Self {
        let mut social = Self::default();
        let Ok(value) = php_unserialize(serialized) else {
            return social;
        };
        social.opengraph = php_bool(&value, "opengraph", social.opengraph);
        social.twitter = php_bool(&value, "twitter", social.twitter);
        copy_string(&value, "og_default_image", &mut social.og_default_image);
        copy_string(&value, "og_frontpage_title", &mut social.og_frontpage_title);
        copy_string(&value, "og_frontpage_desc", &mut social.og_frontpage_desc);
        copy_string(&value, "og_frontpage_image", &mut social.og_frontpage_image);
        copy_string(&value, "twitter_card_type", &mut social.twitter_card_type);
        copy_string(&value, "facebook_site", &mut social.facebook_site);
        copy_string(&value, "twitter_site", &mut social.twitter_site);
        social
    }

    pub fn pairs(&self) -> Vec<(String, PhpValue)> {
        vec![
            ("opengraph".into(), flag(self.opengraph)),
            ("twitter".into(), flag(self.twitter)),
            ("og_default_image".into(), text(&self.og_default_image)),
            ("og_frontpage_title".into(), text(&self.og_frontpage_title)),
            ("og_frontpage_desc".into(), text(&self.og_frontpage_desc)),
            ("og_frontpage_image".into(), text(&self.og_frontpage_image)),
            ("twitter_card_type".into(), text(&self.twitter_card_type)),
            ("facebook_site".into(), text(&self.facebook_site)),
            ("twitter_site".into(), text(&self.twitter_site)),
        ]
    }
}

impl YoastGeneral {
    pub fn parse(serialized: &str) -> Self {
        let mut general = Self::default();
        let Ok(value) = php_unserialize(serialized) else {
            return general;
        };
        general.enable_xml_sitemap =
            php_bool(&value, "enable_xml_sitemap", general.enable_xml_sitemap);
        copy_string(&value, "googleverify", &mut general.googleverify);
        copy_string(&value, "msverify", &mut general.msverify);
        copy_string(&value, "yandexverify", &mut general.yandexverify);
        copy_string(&value, "baiduverify", &mut general.baiduverify);
        copy_string(&value, "version", &mut general.version);
        general
    }

    pub fn pairs(&self) -> Vec<(String, PhpValue)> {
        vec![
            ("enable_xml_sitemap".into(), flag(self.enable_xml_sitemap)),
            ("googleverify".into(), text(&self.googleverify)),
            ("msverify".into(), text(&self.msverify)),
            ("yandexverify".into(), text(&self.yandexverify)),
            ("baiduverify".into(), text(&self.baiduverify)),
            ("version".into(), text(&self.version)),
        ]
    }
}

/// The character Yoast shows for a stored separator code.
pub fn separator_char(stored: &str) -> String {
    match stored {
        "sc-dash" => "-".to_string(),
        "sc-ndash" => "–".to_string(),
        "sc-mdash" => "—".to_string(),
        "sc-colon" => ":".to_string(),
        "sc-middot" => "·".to_string(),
        "sc-bull" => "•".to_string(),
        "sc-star" => "*".to_string(),
        "sc-smstar" => "⋆".to_string(),
        "sc-pipe" => "|".to_string(),
        "sc-tilde" => "~".to_string(),
        "sc-laquo" => "«".to_string(),
        "sc-raquo" => "»".to_string(),
        "sc-lt" => ">".to_string(),
        "sc-gt" => "<".to_string(),
        other => other.to_string(),
    }
}

/// Store a separator chosen in the admin form as Yoast's code when it has one.
pub fn separator_code(display: &str) -> String {
    match display {
        "-" => "sc-dash".to_string(),
        "–" | "&ndash;" => "sc-ndash".to_string(),
        "—" | "&mdash;" => "sc-mdash".to_string(),
        ":" => "sc-colon".to_string(),
        "·" => "sc-middot".to_string(),
        "•" | "&bull;" => "sc-bull".to_string(),
        "*" => "sc-star".to_string(),
        "⋆" => "sc-smstar".to_string(),
        "|" => "sc-pipe".to_string(),
        "~" => "sc-tilde".to_string(),
        "«" => "sc-laquo".to_string(),
        "»" => "sc-raquo".to_string(),
        ">" => "sc-lt".to_string(),
        "<" => "sc-gt".to_string(),
        other => other.to_string(),
    }
}

/// Update keys inside a PHP array and keep every other key.
pub fn merge_php_map(existing: Option<&str>, updates: &[(String, PhpValue)]) -> String {
    let mut pairs = match existing.and_then(|raw| php_unserialize(raw).ok()) {
        Some(PhpValue::Array(pairs)) => pairs,
        _ => Vec::new(),
    };
    for (key, value) in updates {
        if let Some(slot) = pairs
            .iter_mut()
            .find(|(name, _)| name.as_str() == Some(key))
        {
            slot.1 = value.clone();
        } else {
            pairs.push((PhpValue::String(key.clone()), value.clone()));
        }
    }
    php_serialize(&PhpValue::Array(pairs))
}

fn text(value: &str) -> PhpValue {
    PhpValue::String(value.to_string())
}

fn flag(value: bool) -> PhpValue {
    PhpValue::Bool(value)
}

fn copy_string(value: &PhpValue, key: &str, slot: &mut String) {
    if let Some(found) = php_string(value, key) {
        *slot = found;
    }
}

fn php_string(value: &PhpValue, key: &str) -> Option<String> {
    match value.get(key)? {
        PhpValue::String(text) => Some(text.clone()),
        PhpValue::Int(number) => Some(number.to_string()),
        PhpValue::Bool(flag) => Some(if *flag { "1" } else { "0" }.to_string()),
        _ => None,
    }
}

fn php_bool(value: &PhpValue, key: &str, default: bool) -> bool {
    match value.get(key) {
        Some(PhpValue::Bool(flag)) => *flag,
        Some(PhpValue::Int(number)) => *number != 0,
        Some(PhpValue::String(text)) => matches!(text.as_str(), "1" | "true" | "on" | "yes"),
        _ => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_separator_and_home_title_round_trip() {
        let mut titles = YoastTitles::default();
        titles.separator_code = separator_code("|");
        titles.separator = separator_char(&titles.separator_code);
        titles.title_home = "Linen Workshop".to_string();
        titles.metadesc_home = "Stoneware from a small workshop.".to_string();
        let stored = merge_php_map(None, &titles.pairs());
        let parsed = YoastTitles::parse(&stored);
        assert_eq!(parsed.separator, "|");
        assert_eq!(parsed.separator_code, "sc-pipe");
        assert_eq!(parsed.title_home, "Linen Workshop");
        assert_eq!(parsed.metadesc_home, "Stoneware from a small workshop.");
        assert!(parsed.noindex_archive);
        assert!(!parsed.noindex_post);
    }

    #[test]
    fn merge_keeps_keys_this_crate_does_not_own() {
        let original = php_serialize(&PhpValue::Array(vec![(
            PhpValue::String("company_logo".into()),
            PhpValue::String("https://example.com/logo.png".into()),
        )]));
        let merged = merge_php_map(
            Some(&original),
            &[(
                "title-post".to_string(),
                PhpValue::String("%%title%%".into()),
            )],
        );
        let parsed = php_unserialize(&merged).unwrap();
        assert_eq!(
            parsed.get("company_logo").and_then(|v| v.as_str()),
            Some("https://example.com/logo.png")
        );
        assert_eq!(
            parsed.get("title-post").and_then(|v| v.as_str()),
            Some("%%title%%")
        );
    }
}
