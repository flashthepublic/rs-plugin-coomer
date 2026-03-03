use rs_plugin_common_interfaces::{
    domain::{
        external_images::{ExternalImage, ImageType},
        media::{FileType, Media},
        person::Person,
        tag::Tag,
        Relations,
    },
    lookup::{RsLookupMetadataResult, RsLookupMetadataResultWrapper},
    RsRequest,
};
use serde_json::json;

use crate::coomer::CoomerPost;

pub fn coomer_post_to_result(post: CoomerPost) -> RsLookupMetadataResultWrapper {
    let images = coomer_post_to_images(&post);

    let id = post
        .id
        .as_ref()
        .map(|pid| format!("coomer:{pid}"))
        .unwrap_or_else(|| fallback_local_id(&post.title));

    let params = json!({
        "coomerUrl": post.post_url,
        "service": post.service,
        "creatorId": post.creator_id,
        "creatorName": post.creator_name,
        "postId": post.post_id,
        "published": post.published,
        "tags": post.tags,
    });

    let kind = determine_file_type(&post);
    let mimetype = post
        .file_urls
        .first()
        .and_then(|f| f.mime.clone())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let file_count = post.file_urls.len();

    let people_details = vec![Person {
        id: format!("coomer-creator:{}/{}", post.service, post.creator_id),
        name: post.creator_name.clone(),
        generated: true,
        ..Default::default()
    }];

    let tag_details: Vec<Tag> = post
        .tags
        .iter()
        .map(|tag_name| Tag {
            id: format!("coomer-tag:{tag_name}"),
            name: tag_name.clone(),
            parent: None,
            kind: None,
            alt: None,
            thumb: None,
            params: None,
            modified: 0,
            added: 0,
            generated: true,
            path: "/".to_string(),
            otherids: Some(vec![format!("coomer-tag:{tag_name}")].into()),
        })
        .collect();

    let media = Media {
        id,
        name: post.title,
        description: post.content,
        kind,
        mimetype,
        params: Some(params),
        pages: if file_count > 0 {
            Some(file_count)
        } else {
            None
        },
        ..Default::default()
    };

    RsLookupMetadataResultWrapper {
        metadata: RsLookupMetadataResult::Media(media),
        relations: Some(Relations {
            ext_images: if images.is_empty() {
                None
            } else {
                Some(images)
            },
            people_details: if people_details.is_empty() {
                None
            } else {
                Some(people_details)
            },
            tags_details: if tag_details.is_empty() {
                None
            } else {
                Some(tag_details)
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub fn coomer_post_to_images(post: &CoomerPost) -> Vec<ExternalImage> {
    post.file_urls
        .iter()
        .filter(|f| {
            f.mime
                .as_deref()
                .map(|m| m.starts_with("image/"))
                .unwrap_or(true)
        })
        .enumerate()
        .map(|(idx, file_info)| ExternalImage {
            kind: Some(if idx == 0 {
                ImageType::Poster
            } else {
                ImageType::Still
            }),
            url: RsRequest {
                url: file_info.url.clone(),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect()
}

fn determine_file_type(post: &CoomerPost) -> FileType {
    if post.file_urls.len() > 1 {
        return FileType::Album;
    }

    if let Some(first) = post.file_urls.first() {
        if let Some(mime) = &first.mime {
            if mime.starts_with("video/") {
                return FileType::Video;
            }
            if mime.starts_with("image/") {
                return FileType::Photo;
            }
        }
    }

    FileType::Other
}

fn fallback_local_id(title: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;

    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "coomer-post".to_string()
    } else {
        format!("coomer-post-{slug}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coomer::CoomerFileInfo;

    #[test]
    fn maps_post_to_media_result() {
        let post = CoomerPost {
            id: Some("onlyfans/creator1/12345".to_string()),
            service: "onlyfans".to_string(),
            creator_id: "creator1".to_string(),
            creator_name: "Creator One".to_string(),
            post_id: "12345".to_string(),
            title: "Test Post".to_string(),
            content: Some("Post content".to_string()),
            post_url: "https://coomer.st/onlyfans/user/creator1/post/12345".to_string(),
            file_urls: vec![CoomerFileInfo {
                url: "https://coomer.st/data/photo.jpg".to_string(),
                filename: Some("photo.jpg".to_string()),
                mime: Some("image/jpeg".to_string()),
            }],
            ..Default::default()
        };

        let result = coomer_post_to_result(post);
        if let RsLookupMetadataResult::Media(media) = &result.metadata {
            assert_eq!(media.id, "coomer:onlyfans/creator1/12345");
            assert_eq!(media.name, "Test Post");
            assert_eq!(media.description, Some("Post content".to_string()));
            assert_eq!(media.kind, FileType::Photo);
            assert_eq!(media.mimetype, "image/jpeg");
            assert_eq!(media.pages, Some(1));
        } else {
            panic!("Expected Media metadata");
        }

        let relations = result.relations.expect("expected relations");
        let people = relations.people_details.expect("expected people_details");
        assert_eq!(people[0].id, "coomer-creator:onlyfans/creator1");
        assert_eq!(people[0].name, "Creator One");

        let tags = relations.tags_details.expect("expected tags_details");
        assert_eq!(tags[0].id, "coomer-tag:onlyfans");
    }

    #[test]
    fn maps_multi_file_post_as_album() {
        let post = CoomerPost {
            id: Some("onlyfans/c/1".to_string()),
            title: "Album Post".to_string(),
            file_urls: vec![
                CoomerFileInfo {
                    url: "https://coomer.st/1.jpg".to_string(),
                    mime: Some("image/jpeg".to_string()),
                    ..Default::default()
                },
                CoomerFileInfo {
                    url: "https://coomer.st/2.jpg".to_string(),
                    mime: Some("image/jpeg".to_string()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let result = coomer_post_to_result(post);
        if let RsLookupMetadataResult::Media(media) = &result.metadata {
            assert_eq!(media.kind, FileType::Album);
            assert_eq!(media.pages, Some(2));
        } else {
            panic!("Expected Media metadata");
        }
    }

    #[test]
    fn maps_video_post() {
        let post = CoomerPost {
            id: Some("onlyfans/c/1".to_string()),
            title: "Video Post".to_string(),
            file_urls: vec![CoomerFileInfo {
                url: "https://coomer.st/video.mp4".to_string(),
                mime: Some("video/mp4".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = coomer_post_to_result(post);
        if let RsLookupMetadataResult::Media(media) = &result.metadata {
            assert_eq!(media.kind, FileType::Video);
        } else {
            panic!("Expected Media metadata");
        }
    }

    #[test]
    fn images_filters_videos_and_sets_types() {
        let post = CoomerPost {
            file_urls: vec![
                CoomerFileInfo {
                    url: "https://coomer.st/1.jpg".to_string(),
                    mime: Some("image/jpeg".to_string()),
                    ..Default::default()
                },
                CoomerFileInfo {
                    url: "https://coomer.st/2.mp4".to_string(),
                    mime: Some("video/mp4".to_string()),
                    ..Default::default()
                },
                CoomerFileInfo {
                    url: "https://coomer.st/3.png".to_string(),
                    mime: Some("image/png".to_string()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let images = coomer_post_to_images(&post);
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].kind, Some(ImageType::Poster));
        assert_eq!(images[0].url.url, "https://coomer.st/1.jpg");
        assert_eq!(images[1].kind, Some(ImageType::Still));
        assert_eq!(images[1].url.url, "https://coomer.st/3.png");
    }

    #[test]
    fn fallback_local_id_slugifies() {
        assert_eq!(fallback_local_id("My Test Post"), "coomer-post-my-test-post");
        assert_eq!(fallback_local_id(""), "coomer-post");
        assert_eq!(fallback_local_id("  "), "coomer-post");
    }

    #[test]
    fn post_no_id_uses_fallback() {
        let post = CoomerPost {
            id: None,
            title: "Untitled Post".to_string(),
            ..Default::default()
        };

        let result = coomer_post_to_result(post);
        if let RsLookupMetadataResult::Media(media) = &result.metadata {
            assert_eq!(media.id, "coomer-post-untitled-post");
        } else {
            panic!("Expected Media metadata");
        }
    }
}
