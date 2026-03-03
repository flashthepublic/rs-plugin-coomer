use serde::Deserialize;

const BASE_URL: &str = "https://coomer.st";
const API_BASE: &str = "https://coomer.st/api/v1";

// ---------------------------------------------------------------------------
// API response structs (deserialized from coomer.st JSON API)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct CoomerApiPost {
    pub id: Option<String>,
    pub service: Option<String>,
    pub user: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub published: Option<String>,
    pub added: Option<String>,
    pub file: Option<CoomerApiFile>,
    pub attachments: Option<Vec<CoomerApiFile>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoomerApiFile {
    pub name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoomerSearchResponse {
    pub count: Option<u64>,
    pub true_count: Option<u64>,
    pub posts: Option<Vec<CoomerApiPost>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CoomerApiProfile {
    pub id: Option<String>,
    pub name: Option<String>,
    pub service: Option<String>,
    pub indexed: Option<String>,
    pub updated: Option<String>,
    pub public_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Domain structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoomerPost {
    pub id: Option<String>,
    pub service: String,
    pub creator_id: String,
    pub creator_name: String,
    pub post_id: String,
    pub title: String,
    pub content: Option<String>,
    pub published: Option<String>,
    pub post_url: String,
    pub thumbnail_url: Option<String>,
    pub file_urls: Vec<CoomerFileInfo>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoomerFileInfo {
    pub url: String,
    pub filename: Option<String>,
    pub mime: Option<String>,
}

// ---------------------------------------------------------------------------
// ID parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoomerLookupId {
    Post {
        service: String,
        creator_id: String,
        post_id: String,
    },
    Creator {
        service: String,
        creator_id: String,
    },
}

/// Parse a coomer identifier from various formats:
/// - "coomer:onlyfans/creator_id/post_id" -> Post
/// - "coomer:onlyfans/creator_id" -> Creator
/// - "https://coomer.st/onlyfans/user/creator_id/post/post_id" -> Post
/// - "https://coomer.st/onlyfans/user/creator_id" -> Creator
/// - Accepts both coomer.st and coomer.su domains
pub fn parse_coomer_id(value: &str) -> Option<CoomerLookupId> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Try "coomer:" prefix format
    if let Some(rest) = strip_prefix_case_insensitive(trimmed, "coomer:") {
        return parse_coomer_path(rest);
    }

    // Try URL format
    extract_from_url(trimmed)
}

fn strip_prefix_case_insensitive<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    if value.len() >= prefix.len()
        && value[..prefix.len()].eq_ignore_ascii_case(prefix)
    {
        Some(&value[prefix.len()..])
    } else {
        None
    }
}

fn parse_coomer_path(path: &str) -> Option<CoomerLookupId> {
    let parts: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    match parts.len() {
        2 => Some(CoomerLookupId::Creator {
            service: parts[0].to_string(),
            creator_id: parts[1].to_string(),
        }),
        3 => Some(CoomerLookupId::Post {
            service: parts[0].to_string(),
            creator_id: parts[1].to_string(),
            post_id: parts[2].to_string(),
        }),
        _ => None,
    }
}

fn extract_from_url(value: &str) -> Option<CoomerLookupId> {
    let without_scheme = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))?;

    let path = without_scheme
        .strip_prefix("coomer.st/")
        .or_else(|| without_scheme.strip_prefix("coomer.su/"))
        .or_else(|| without_scheme.strip_prefix("www.coomer.st/"))
        .or_else(|| without_scheme.strip_prefix("www.coomer.su/"))?;

    let clean = path
        .split('?')
        .next()
        .unwrap_or(path)
        .split('#')
        .next()
        .unwrap_or(path)
        .trim_matches('/');

    let parts: Vec<&str> = clean.split('/').filter(|s| !s.is_empty()).collect();

    // URL format: {service}/user/{creator_id}[/post/{post_id}]
    match parts.as_slice() {
        [service, "user", creator_id, "post", post_id] => Some(CoomerLookupId::Post {
            service: service.to_string(),
            creator_id: creator_id.to_string(),
            post_id: post_id.to_string(),
        }),
        [service, "user", creator_id] => Some(CoomerLookupId::Creator {
            service: service.to_string(),
            creator_id: creator_id.to_string(),
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// URL builders
// ---------------------------------------------------------------------------

pub fn build_creator_posts_url(
    service: &str,
    creator_id: &str,
    offset: Option<u32>,
) -> String {
    let mut url = format!("{API_BASE}/{service}/user/{creator_id}/posts");
    if let Some(o) = offset {
        if o > 0 {
            url.push_str(&format!("?o={o}"));
        }
    }
    url
}

pub fn build_post_url(service: &str, creator_id: &str, post_id: &str) -> String {
    format!("{API_BASE}/{service}/user/{creator_id}/post/{post_id}")
}

pub fn build_profile_url(service: &str, creator_id: &str) -> String {
    format!("{API_BASE}/{service}/user/{creator_id}/profile")
}

pub fn build_post_web_url(service: &str, creator_id: &str, post_id: &str) -> String {
    format!("{BASE_URL}/{service}/user/{creator_id}/post/{post_id}")
}

pub fn build_search_posts_url(query: &str, offset: Option<u32>) -> String {
    let encoded_query = url_encode(query);
    let mut url = format!("{API_BASE}/posts?q={encoded_query}");
    if let Some(o) = offset {
        if o > 0 {
            url.push_str(&format!("&o={o}"));
        }
    }
    url
}

fn url_encode(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}

pub fn build_creator_icon_url(service: &str, creator_id: &str) -> String {
    format!("{BASE_URL}/icons/{service}/{creator_id}")
}

pub fn build_creator_web_url(service: &str, creator_id: &str) -> String {
    format!("{BASE_URL}/{service}/user/{creator_id}")
}

pub fn build_file_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{BASE_URL}{path}")
    }
}

// ---------------------------------------------------------------------------
// MIME type guessing
// ---------------------------------------------------------------------------

pub fn mime_from_filename(filename: &str) -> Option<String> {
    let ext = filename.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "png" => Some("image/png".to_string()),
        "gif" => Some("image/gif".to_string()),
        "webp" => Some("image/webp".to_string()),
        "mp4" => Some("video/mp4".to_string()),
        "webm" => Some("video/webm".to_string()),
        "mov" => Some("video/quicktime".to_string()),
        "m4v" => Some("video/mp4".to_string()),
        "avi" => Some("video/x-msvideo".to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

pub fn parse_posts_json(json_str: &str) -> Vec<CoomerApiPost> {
    serde_json::from_str(json_str).unwrap_or_default()
}

pub fn parse_post_json(json_str: &str) -> Option<CoomerApiPost> {
    serde_json::from_str(json_str).ok()
}

pub fn parse_profile_json(json_str: &str) -> Option<CoomerApiProfile> {
    serde_json::from_str(json_str).ok()
}

pub fn parse_search_json(json_str: &str) -> Option<CoomerSearchResponse> {
    serde_json::from_str(json_str).ok()
}

// ---------------------------------------------------------------------------
// API -> Domain conversion
// ---------------------------------------------------------------------------

pub fn api_post_to_coomer_post(api_post: CoomerApiPost, creator_name: &str) -> CoomerPost {
    let post_id = api_post.id.clone().unwrap_or_default();
    let service = api_post.service.clone().unwrap_or_default();
    let creator_id = api_post.user.clone().unwrap_or_default();

    let mut file_urls = Vec::new();

    // Main file
    if let Some(file) = &api_post.file {
        if let Some(path) = &file.path {
            if !path.is_empty() {
                let url = build_file_url(path);
                let filename = file.name.clone();
                let mime = filename
                    .as_deref()
                    .and_then(mime_from_filename)
                    .or_else(|| mime_from_filename(path));
                file_urls.push(CoomerFileInfo {
                    url,
                    filename,
                    mime,
                });
            }
        }
    }

    // Attachments
    if let Some(attachments) = &api_post.attachments {
        for att in attachments {
            if let Some(path) = &att.path {
                if !path.is_empty() {
                    let url = build_file_url(path);
                    let filename = att.name.clone();
                    let mime = filename
                        .as_deref()
                        .and_then(mime_from_filename)
                        .or_else(|| mime_from_filename(path));
                    file_urls.push(CoomerFileInfo {
                        url,
                        filename,
                        mime,
                    });
                }
            }
        }
    }

    let thumbnail_url = file_urls.first().map(|f| f.url.clone());

    let title = api_post
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{} - {}", creator_name, post_id));

    let composite_id = format!("{service}/{creator_id}/{post_id}");
    let post_url = build_post_web_url(&service, &creator_id, &post_id);

    let tags = vec![service.clone()];

    CoomerPost {
        id: Some(composite_id),
        service,
        creator_id,
        creator_name: creator_name.to_string(),
        post_id,
        title,
        content: api_post.content,
        published: api_post.published,
        post_url,
        thumbnail_url,
        file_urls,
        tags,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_coomer_id tests --

    #[test]
    fn parse_coomer_id_empty_returns_none() {
        assert_eq!(parse_coomer_id(""), None);
        assert_eq!(parse_coomer_id("   "), None);
    }

    #[test]
    fn parse_coomer_id_prefix_creator() {
        assert_eq!(
            parse_coomer_id("coomer:onlyfans/belledelphine"),
            Some(CoomerLookupId::Creator {
                service: "onlyfans".to_string(),
                creator_id: "belledelphine".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_prefix_post() {
        assert_eq!(
            parse_coomer_id("coomer:onlyfans/belledelphine/12345"),
            Some(CoomerLookupId::Post {
                service: "onlyfans".to_string(),
                creator_id: "belledelphine".to_string(),
                post_id: "12345".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_prefix_case_insensitive() {
        assert_eq!(
            parse_coomer_id("Coomer:fansly/creator1"),
            Some(CoomerLookupId::Creator {
                service: "fansly".to_string(),
                creator_id: "creator1".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_url_creator_st() {
        assert_eq!(
            parse_coomer_id("https://coomer.st/onlyfans/user/belledelphine"),
            Some(CoomerLookupId::Creator {
                service: "onlyfans".to_string(),
                creator_id: "belledelphine".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_url_post_st() {
        assert_eq!(
            parse_coomer_id("https://coomer.st/onlyfans/user/belledelphine/post/12345"),
            Some(CoomerLookupId::Post {
                service: "onlyfans".to_string(),
                creator_id: "belledelphine".to_string(),
                post_id: "12345".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_url_su_domain() {
        assert_eq!(
            parse_coomer_id("https://coomer.su/fansly/user/creator1"),
            Some(CoomerLookupId::Creator {
                service: "fansly".to_string(),
                creator_id: "creator1".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_url_with_query_params() {
        assert_eq!(
            parse_coomer_id("https://coomer.st/onlyfans/user/creator1?o=50"),
            Some(CoomerLookupId::Creator {
                service: "onlyfans".to_string(),
                creator_id: "creator1".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_url_trailing_slash() {
        assert_eq!(
            parse_coomer_id("https://coomer.st/onlyfans/user/creator1/"),
            Some(CoomerLookupId::Creator {
                service: "onlyfans".to_string(),
                creator_id: "creator1".to_string(),
            })
        );
    }

    #[test]
    fn parse_coomer_id_plain_text_returns_none() {
        assert_eq!(parse_coomer_id("belledelphine"), None);
        assert_eq!(parse_coomer_id("some random text"), None);
    }

    #[test]
    fn parse_coomer_id_prefix_single_part_returns_none() {
        assert_eq!(parse_coomer_id("coomer:onlyfans"), None);
    }

    // -- URL builder tests --

    #[test]
    fn build_creator_posts_url_no_offset() {
        assert_eq!(
            build_creator_posts_url("onlyfans", "creator1", None),
            "https://coomer.st/api/v1/onlyfans/user/creator1/posts"
        );
    }

    #[test]
    fn build_creator_posts_url_zero_offset() {
        assert_eq!(
            build_creator_posts_url("onlyfans", "creator1", Some(0)),
            "https://coomer.st/api/v1/onlyfans/user/creator1/posts"
        );
    }

    #[test]
    fn build_creator_posts_url_with_offset() {
        assert_eq!(
            build_creator_posts_url("onlyfans", "creator1", Some(50)),
            "https://coomer.st/api/v1/onlyfans/user/creator1/posts?o=50"
        );
    }

    #[test]
    fn build_post_url_test() {
        assert_eq!(
            build_post_url("onlyfans", "creator1", "12345"),
            "https://coomer.st/api/v1/onlyfans/user/creator1/post/12345"
        );
    }

    #[test]
    fn build_profile_url_test() {
        assert_eq!(
            build_profile_url("fansly", "creator1"),
            "https://coomer.st/api/v1/fansly/user/creator1/profile"
        );
    }

    #[test]
    fn build_post_web_url_test() {
        assert_eq!(
            build_post_web_url("onlyfans", "creator1", "12345"),
            "https://coomer.st/onlyfans/user/creator1/post/12345"
        );
    }

    #[test]
    fn build_file_url_relative_path() {
        assert_eq!(
            build_file_url("/data/ab/cd/abcdef123.jpg"),
            "https://coomer.st/data/ab/cd/abcdef123.jpg"
        );
    }

    #[test]
    fn build_file_url_absolute_passes_through() {
        assert_eq!(
            build_file_url("https://cdn.example.com/file.jpg"),
            "https://cdn.example.com/file.jpg"
        );
    }

    // -- MIME tests --

    #[test]
    fn mime_from_filename_images() {
        assert_eq!(mime_from_filename("photo.jpg"), Some("image/jpeg".to_string()));
        assert_eq!(mime_from_filename("photo.jpeg"), Some("image/jpeg".to_string()));
        assert_eq!(mime_from_filename("photo.png"), Some("image/png".to_string()));
        assert_eq!(mime_from_filename("photo.gif"), Some("image/gif".to_string()));
        assert_eq!(mime_from_filename("photo.webp"), Some("image/webp".to_string()));
    }

    #[test]
    fn mime_from_filename_videos() {
        assert_eq!(mime_from_filename("video.mp4"), Some("video/mp4".to_string()));
        assert_eq!(mime_from_filename("video.webm"), Some("video/webm".to_string()));
        assert_eq!(mime_from_filename("video.mov"), Some("video/quicktime".to_string()));
    }

    #[test]
    fn mime_from_filename_unknown() {
        assert_eq!(mime_from_filename("file.xyz"), None);
    }

    #[test]
    fn mime_from_filename_case_insensitive() {
        assert_eq!(mime_from_filename("photo.JPG"), Some("image/jpeg".to_string()));
        assert_eq!(mime_from_filename("video.MP4"), Some("video/mp4".to_string()));
    }

    // -- JSON parsing tests --

    #[test]
    fn parse_posts_json_valid() {
        let json = r#"[{"id":"123","service":"onlyfans","user":"creator1","title":"Test Post"}]"#;
        let posts = parse_posts_json(json);
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, Some("123".to_string()));
        assert_eq!(posts[0].title, Some("Test Post".to_string()));
    }

    #[test]
    fn parse_posts_json_invalid_returns_empty() {
        assert!(parse_posts_json("invalid json").is_empty());
        assert!(parse_posts_json("").is_empty());
    }

    #[test]
    fn parse_post_json_valid() {
        let json = r#"{"id":"123","service":"onlyfans","user":"creator1","title":"Test"}"#;
        let post = parse_post_json(json);
        assert!(post.is_some());
        assert_eq!(post.unwrap().id, Some("123".to_string()));
    }

    #[test]
    fn parse_profile_json_valid() {
        let json = r#"{"id":"creator1","name":"Belle Delphine","service":"onlyfans"}"#;
        let profile = parse_profile_json(json);
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().name, Some("Belle Delphine".to_string()));
    }

    // -- api_post_to_coomer_post tests --

    #[test]
    fn api_post_converts_with_files() {
        let api_post = CoomerApiPost {
            id: Some("12345".to_string()),
            service: Some("onlyfans".to_string()),
            user: Some("creator1".to_string()),
            title: Some("My Post".to_string()),
            content: Some("Post content".to_string()),
            published: Some("2024-01-01".to_string()),
            added: None,
            file: Some(CoomerApiFile {
                name: Some("photo.jpg".to_string()),
                path: Some("/data/ab/cd/photo.jpg".to_string()),
            }),
            attachments: Some(vec![CoomerApiFile {
                name: Some("video.mp4".to_string()),
                path: Some("/data/ef/gh/video.mp4".to_string()),
            }]),
        };

        let post = api_post_to_coomer_post(api_post, "Creator One");
        assert_eq!(post.id, Some("onlyfans/creator1/12345".to_string()));
        assert_eq!(post.title, "My Post");
        assert_eq!(post.creator_name, "Creator One");
        assert_eq!(post.file_urls.len(), 2);
        assert_eq!(
            post.file_urls[0].url,
            "https://coomer.st/data/ab/cd/photo.jpg"
        );
        assert_eq!(post.file_urls[0].mime, Some("image/jpeg".to_string()));
        assert_eq!(
            post.file_urls[1].url,
            "https://coomer.st/data/ef/gh/video.mp4"
        );
        assert_eq!(post.file_urls[1].mime, Some("video/mp4".to_string()));
        assert_eq!(
            post.thumbnail_url,
            Some("https://coomer.st/data/ab/cd/photo.jpg".to_string())
        );
    }

    #[test]
    fn api_post_title_fallback() {
        let api_post = CoomerApiPost {
            id: Some("99".to_string()),
            service: Some("fansly".to_string()),
            user: Some("user1".to_string()),
            title: None,
            content: None,
            published: None,
            added: None,
            file: None,
            attachments: None,
        };

        let post = api_post_to_coomer_post(api_post, "User One");
        assert_eq!(post.title, "User One - 99");
    }

    #[test]
    fn api_post_empty_title_uses_fallback() {
        let api_post = CoomerApiPost {
            id: Some("99".to_string()),
            service: Some("fansly".to_string()),
            user: Some("user1".to_string()),
            title: Some("  ".to_string()),
            content: None,
            published: None,
            added: None,
            file: None,
            attachments: None,
        };

        let post = api_post_to_coomer_post(api_post, "User One");
        assert_eq!(post.title, "User One - 99");
    }

    #[test]
    fn api_post_skips_empty_file_paths() {
        let api_post = CoomerApiPost {
            id: Some("1".to_string()),
            service: Some("onlyfans".to_string()),
            user: Some("u".to_string()),
            title: Some("Title".to_string()),
            content: None,
            published: None,
            added: None,
            file: Some(CoomerApiFile {
                name: None,
                path: Some(String::new()),
            }),
            attachments: Some(vec![CoomerApiFile {
                name: Some("a.jpg".to_string()),
                path: Some("/data/a.jpg".to_string()),
            }]),
        };

        let post = api_post_to_coomer_post(api_post, "U");
        assert_eq!(post.file_urls.len(), 1);
        assert_eq!(post.file_urls[0].url, "https://coomer.st/data/a.jpg");
    }
}
