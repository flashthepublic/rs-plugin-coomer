use extism_pdk::{http, log, plugin_fn, FnResult, HttpRequest, Json, LogLevel, WithReturnCode};
use flate2::read::GzDecoder;
use std::collections::HashSet;
use std::io::Read;

use rs_plugin_common_interfaces::{
    domain::external_images::ExternalImage,
    domain::media::{MediaForUpdate, MediaItemReference},
    lookup::{
        RsLookupMatchType, RsLookupMetadataResults, RsLookupPerson, RsLookupQuery,
        RsLookupSourceResult, RsLookupWrapper,
    },
    request::{RsGroupDownload, RsRequest},
    PluginInformation, PluginType,
};

mod convert;
mod coomer;

use convert::{coomer_post_to_images, coomer_post_to_result};
use coomer::{
    api_post_to_coomer_post, build_creator_posts_url, build_post_url, build_profile_url,
    parse_coomer_id, parse_post_json, parse_posts_json, parse_profile_json, CoomerLookupId,
    CoomerPost,
};

const POSTS_PER_PAGE: u32 = 50;

enum LookupTarget {
    DirectPost {
        service: String,
        creator_id: String,
        post_id: String,
    },
    CreatorListing {
        service: String,
        creator_id: String,
    },
}

#[plugin_fn]
pub fn infos() -> FnResult<Json<PluginInformation>> {
    Ok(Json(PluginInformation {
        name: "coomer_metadata".into(),
        capabilities: vec![PluginType::LookupMetadata, PluginType::Lookup],
        version: 1,
        interface_version: 1,
        repo: Some("https://github.com/flashthepublic/rs-plugin-coomer".to_string()),
        publisher: "neckaros".into(),
        description: "Look up content metadata from coomer.st".into(),
        credential_kind: None,
        settings: vec![],
        ..Default::default()
    }))
}

fn build_http_request(url: String) -> HttpRequest {
    let mut request = HttpRequest {
        url,
        headers: Default::default(),
        method: Some("GET".into()),
    };

    // DDoS-Guard blocks application/json; text/css is the workaround
    request
        .headers
        .insert("Accept".to_string(), "text/css".to_string());
    request.headers.insert(
        "User-Agent".to_string(),
        "Mozilla/5.0 (compatible; rs-plugin-coomer/0.1)".to_string(),
    );

    request
}

fn decompress_gzip(data: &[u8]) -> Option<String> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed).ok()?;
    Some(decompressed)
}

fn execute_json_request(url: String) -> FnResult<String> {
    let request = build_http_request(url);
    let res = http::request::<Vec<u8>>(&request, None);

    match res {
        Ok(res) if res.status_code() >= 200 && res.status_code() < 300 => {
            let body = res.body();
            // Try gzip decompression first (coomer.st always returns gzip)
            let text = decompress_gzip(&body)
                .unwrap_or_else(|| String::from_utf8_lossy(&body).to_string());
            Ok(text)
        }
        Ok(res) => {
            log!(
                LogLevel::Error,
                "coomer HTTP error {}: {}",
                res.status_code(),
                String::from_utf8_lossy(&res.body())
            );
            Err(WithReturnCode::new(
                extism_pdk::Error::msg(format!("HTTP error: {}", res.status_code())),
                res.status_code() as i32,
            ))
        }
        Err(e) => {
            log!(LogLevel::Error, "coomer request failed: {}", e);
            Err(WithReturnCode(e, 500))
        }
    }
}

fn resolve_person_lookup_target(person: &RsLookupPerson) -> Option<LookupTarget> {
    // Check name field for coomer: prefix or URL
    if let Some(id) = person.name.as_deref().and_then(parse_coomer_id) {
        return Some(coomer_id_to_target(id));
    }

    // Check ids fields
    if let Some(ids) = person.ids.as_ref() {
        if let Some(id) = ids.redseat.as_deref().and_then(parse_coomer_id) {
            return Some(coomer_id_to_target(id));
        }

        if let Some(id) = ids.slug.as_deref().and_then(parse_coomer_id) {
            return Some(coomer_id_to_target(id));
        }

        if let Some(id) = ids.other_ids.as_ref().and_then(|other_ids| {
            other_ids
                .as_slice()
                .iter()
                .find_map(|value| parse_coomer_id(value))
        }) {
            return Some(coomer_id_to_target(id));
        }
    }

    None
}

fn coomer_id_to_target(id: CoomerLookupId) -> LookupTarget {
    match id {
        CoomerLookupId::Post {
            service,
            creator_id,
            post_id,
        } => LookupTarget::DirectPost {
            service,
            creator_id,
            post_id,
        },
        CoomerLookupId::Creator {
            service,
            creator_id,
        } => LookupTarget::CreatorListing {
            service,
            creator_id,
        },
    }
}

fn fetch_creator_name(service: &str, creator_id: &str) -> String {
    let url = build_profile_url(service, creator_id);
    match execute_json_request(url) {
        Ok(body) => parse_profile_json(&body)
            .and_then(|p| p.name)
            .unwrap_or_else(|| creator_id.to_string()),
        Err(_) => creator_id.to_string(),
    }
}

fn execute_post_request(
    service: &str,
    creator_id: &str,
    post_id: &str,
) -> FnResult<Vec<CoomerPost>> {
    let creator_name = fetch_creator_name(service, creator_id);
    let url = build_post_url(service, creator_id, post_id);
    let body = execute_json_request(url)?;
    let api_post = parse_post_json(&body);
    Ok(api_post
        .into_iter()
        .map(|p| api_post_to_coomer_post(p, &creator_name))
        .collect())
}

fn execute_creator_listing_request(
    service: &str,
    creator_id: &str,
    offset: Option<u32>,
) -> FnResult<(Vec<CoomerPost>, Option<String>)> {
    let creator_name = fetch_creator_name(service, creator_id);
    let url = build_creator_posts_url(service, creator_id, offset);
    let body = execute_json_request(url)?;
    let api_posts = parse_posts_json(&body);
    let current_offset = offset.unwrap_or(0);
    let next_page_key = if api_posts.len() >= POSTS_PER_PAGE as usize {
        Some((current_offset + POSTS_PER_PAGE).to_string())
    } else {
        None
    };
    let posts = api_posts
        .into_iter()
        .map(|p| api_post_to_coomer_post(p, &creator_name))
        .collect();
    Ok((posts, next_page_key))
}

fn lookup_posts(
    lookup: &RsLookupWrapper,
) -> FnResult<(Vec<CoomerPost>, Option<String>, Option<RsLookupMatchType>)> {
    let person = match &lookup.query {
        RsLookupQuery::Person(person) => person,
        _ => return Ok((vec![], None, None)),
    };

    let page_offset = person
        .page_key
        .as_deref()
        .and_then(|k| k.parse::<u32>().ok());

    match resolve_person_lookup_target(person) {
        Some(LookupTarget::DirectPost {
            service,
            creator_id,
            post_id,
        }) => {
            let posts =
                execute_post_request(&service, &creator_id, &post_id)
                    .unwrap_or_default();
            Ok((posts, None, Some(RsLookupMatchType::ExactId)))
        }
        Some(LookupTarget::CreatorListing {
            service,
            creator_id,
        }) => {
            let (posts, next_page_key) =
                execute_creator_listing_request(&service, &creator_id, page_offset)?;
            Ok((posts, next_page_key, Some(RsLookupMatchType::ExactId)))
        }
        None => Err(WithReturnCode::new(
            extism_pdk::Error::msg("Not supported"),
            404,
        )),
    }
}

#[plugin_fn]
pub fn lookup_metadata(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<RsLookupMetadataResults>> {
    let (posts, next_page_key, match_type) = lookup_posts(&lookup)?;

    let results = posts
        .into_iter()
        .map(|p| {
            let mut result = coomer_post_to_result(p);
            result.match_type = match_type.clone();
            result
        })
        .collect();

    Ok(Json(RsLookupMetadataResults {
        results,
        next_page_key,
    }))
}

#[plugin_fn]
pub fn lookup_metadata_images(
    Json(lookup): Json<RsLookupWrapper>,
) -> FnResult<Json<Vec<ExternalImage>>> {
    let (posts, _, match_type) = lookup_posts(&lookup)?;

    let images: Vec<ExternalImage> = posts
        .iter()
        .flat_map(coomer_post_to_images)
        .map(|mut img| {
            img.match_type = match_type.clone();
            img
        })
        .collect();

    Ok(Json(deduplicate_images(images)))
}

#[plugin_fn]
pub fn lookup(Json(lookup): Json<RsLookupWrapper>) -> FnResult<Json<RsLookupSourceResult>> {
    let person = match &lookup.query {
        RsLookupQuery::Person(person) => person,
        _ => return Ok(Json(RsLookupSourceResult::NotApplicable)),
    };

    match resolve_person_lookup_target(person) {
        Some(LookupTarget::DirectPost {
            service,
            creator_id,
            post_id,
        }) => {
            let posts =
                execute_post_request(&service, &creator_id, &post_id)
                    .unwrap_or_default();
            Ok(Json(posts_to_group_result(
                posts,
                Some(RsLookupMatchType::ExactId),
            )))
        }
        Some(LookupTarget::CreatorListing {
            service,
            creator_id,
        }) => {
            let (posts, _) =
                execute_creator_listing_request(&service, &creator_id, None)?;
            Ok(Json(posts_to_group_result(
                posts,
                Some(RsLookupMatchType::ExactId),
            )))
        }
        None => Ok(Json(RsLookupSourceResult::NotApplicable)),
    }
}

fn post_to_group_download(
    post: CoomerPost,
    match_type: Option<RsLookupMatchType>,
) -> RsGroupDownload {
    let requests: Vec<RsRequest> = post
        .file_urls
        .iter()
        .map(|file_info| RsRequest {
            url: file_info.url.clone(),
            permanent: true,
            mime: file_info.mime.clone(),
            filename: file_info.filename.clone(),
            instant: Some(true),
            ..Default::default()
        })
        .collect();

    let infos = post_to_infos(&post);

    RsGroupDownload {
        group: true,
        group_thumbnail_url: post.thumbnail_url.clone(),
        requests,
        infos,
        match_type,
        ..Default::default()
    }
}

fn post_to_infos(post: &CoomerPost) -> Option<MediaForUpdate> {
    let add_people = vec![MediaItemReference {
        id: format!("coomer-creator:{}/{}", post.service, post.creator_id),
        conf: None,
    }];
    let people_lookup = vec![post.creator_name.clone()];

    Some(MediaForUpdate {
        add_people: Some(add_people),
        people_lookup: Some(people_lookup),
        ..Default::default()
    })
}

fn posts_to_group_result(
    posts: Vec<CoomerPost>,
    match_type: Option<RsLookupMatchType>,
) -> RsLookupSourceResult {
    if posts.is_empty() {
        return RsLookupSourceResult::NotFound;
    }
    let group_downloads = posts
        .into_iter()
        .map(|p| post_to_group_download(p, match_type.clone()))
        .collect();
    RsLookupSourceResult::GroupRequest(group_downloads)
}

fn deduplicate_images(images: Vec<ExternalImage>) -> Vec<ExternalImage> {
    let mut seen_urls = HashSet::new();
    let mut deduped = Vec::new();

    for image in images {
        if seen_urls.insert(image.url.url.clone()) {
            deduped.push(image);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;
    use coomer::CoomerFileInfo;
    use rs_plugin_common_interfaces::domain::rs_ids::RsIds;

    #[test]
    fn lookup_non_person_query_returns_empty() {
        let lookup = RsLookupWrapper {
            query: RsLookupQuery::Movie(Default::default()),
            credential: None,
            params: None,
        };

        let (posts, _, match_type) = lookup_posts(&lookup).expect("lookup should succeed");
        assert!(posts.is_empty());
        assert!(match_type.is_none());
    }

    #[test]
    fn lookup_empty_person_name_returns_404() {
        let lookup = RsLookupWrapper {
            query: RsLookupQuery::Person(RsLookupPerson {
                name: Some(String::new()),
                ids: None,
                page_key: None,
            }),
            credential: None,
            params: None,
        };

        let err = lookup_posts(&lookup).expect_err("expected 404");
        assert_eq!(err.1, 404);
    }

    #[test]
    fn resolve_target_from_name_creator() {
        let person = RsLookupPerson {
            name: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        };

        let target = resolve_person_lookup_target(&person);
        assert!(matches!(
            target,
            Some(LookupTarget::CreatorListing {
                service,
                creator_id,
            }) if service == "onlyfans" && creator_id == "belledelphine"
        ));
    }

    #[test]
    fn resolve_target_from_name_post() {
        let person = RsLookupPerson {
            name: Some("coomer:onlyfans/belledelphine/12345".to_string()),
            ids: None,
            page_key: None,
        };

        let target = resolve_person_lookup_target(&person);
        assert!(matches!(
            target,
            Some(LookupTarget::DirectPost {
                service,
                creator_id,
                post_id,
            }) if service == "onlyfans" && creator_id == "belledelphine" && post_id == "12345"
        ));
    }

    #[test]
    fn resolve_target_from_url_in_name() {
        let person = RsLookupPerson {
            name: Some(
                "https://coomer.st/onlyfans/user/belledelphine/post/12345".to_string(),
            ),
            ids: None,
            page_key: None,
        };

        let target = resolve_person_lookup_target(&person);
        assert!(matches!(
            target,
            Some(LookupTarget::DirectPost {
                service,
                creator_id,
                post_id,
            }) if service == "onlyfans" && creator_id == "belledelphine" && post_id == "12345"
        ));
    }

    #[test]
    fn resolve_target_from_other_ids() {
        let person = RsLookupPerson {
            name: Some("some name".to_string()),
            ids: Some(RsIds {
                other_ids: Some(vec!["coomer:fansly/creator1".to_string()].into()),
                ..Default::default()
            }),
            page_key: None,
        };

        let target = resolve_person_lookup_target(&person);
        assert!(matches!(
            target,
            Some(LookupTarget::CreatorListing {
                service,
                creator_id,
            }) if service == "fansly" && creator_id == "creator1"
        ));
    }

    #[test]
    fn resolve_target_from_slug() {
        let person = RsLookupPerson {
            name: None,
            ids: Some(RsIds {
                slug: Some("coomer:onlyfans/user1".to_string()),
                ..Default::default()
            }),
            page_key: None,
        };

        let target = resolve_person_lookup_target(&person);
        assert!(matches!(
            target,
            Some(LookupTarget::CreatorListing {
                service,
                creator_id,
            }) if service == "onlyfans" && creator_id == "user1"
        ));
    }

    #[test]
    fn resolve_target_plain_name_returns_none() {
        let person = RsLookupPerson {
            name: Some("belledelphine".to_string()),
            ids: None,
            page_key: None,
        };

        let target = resolve_person_lookup_target(&person);
        assert!(target.is_none());
    }

    #[test]
    fn posts_to_group_result_empty_returns_not_found() {
        let result = posts_to_group_result(vec![], None);
        assert!(matches!(result, RsLookupSourceResult::NotFound));
    }

    #[test]
    fn posts_to_group_result_maps_each_post() {
        let posts = vec![
            CoomerPost {
                id: Some("onlyfans/c/1".to_string()),
                service: "onlyfans".to_string(),
                creator_id: "c".to_string(),
                creator_name: "Creator".to_string(),
                post_id: "1".to_string(),
                title: "Post One".to_string(),
                thumbnail_url: Some("https://coomer.st/thumb1.jpg".to_string()),
                file_urls: vec![
                    CoomerFileInfo {
                        url: "https://coomer.st/1.jpg".to_string(),
                        filename: Some("1.jpg".to_string()),
                        mime: Some("image/jpeg".to_string()),
                    },
                    CoomerFileInfo {
                        url: "https://coomer.st/2.jpg".to_string(),
                        filename: Some("2.jpg".to_string()),
                        mime: Some("image/jpeg".to_string()),
                    },
                ],
                ..Default::default()
            },
            CoomerPost {
                id: Some("onlyfans/c/2".to_string()),
                service: "onlyfans".to_string(),
                creator_id: "c".to_string(),
                creator_name: "Creator".to_string(),
                post_id: "2".to_string(),
                title: "Post Two".to_string(),
                file_urls: vec![CoomerFileInfo {
                    url: "https://coomer.st/3.mp4".to_string(),
                    filename: Some("3.mp4".to_string()),
                    mime: Some("video/mp4".to_string()),
                }],
                ..Default::default()
            },
        ];

        let result = posts_to_group_result(posts, Some(RsLookupMatchType::ExactId));
        let RsLookupSourceResult::GroupRequest(downloads) = result else {
            panic!("Expected GroupRequest");
        };
        assert_eq!(downloads.len(), 2);
        assert_eq!(downloads[0].requests.len(), 2);
        assert_eq!(
            downloads[0].group_thumbnail_url,
            Some("https://coomer.st/thumb1.jpg".to_string())
        );
        assert_eq!(downloads[1].requests.len(), 1);
        assert_eq!(
            downloads[1].requests[0].mime,
            Some("video/mp4".to_string())
        );
        assert_eq!(
            downloads[0].match_type,
            Some(RsLookupMatchType::ExactId)
        );
    }

    #[test]
    fn post_to_group_download_sets_request_fields() {
        let post = CoomerPost {
            service: "onlyfans".to_string(),
            creator_id: "c".to_string(),
            creator_name: "Creator".to_string(),
            thumbnail_url: Some("https://coomer.st/thumb.jpg".to_string()),
            file_urls: vec![CoomerFileInfo {
                url: "https://coomer.st/photo.jpg".to_string(),
                filename: Some("photo.jpg".to_string()),
                mime: Some("image/jpeg".to_string()),
            }],
            ..Default::default()
        };

        let download = post_to_group_download(post, Some(RsLookupMatchType::ExactId));
        assert!(download.group);
        assert_eq!(download.requests[0].mime, Some("image/jpeg".to_string()));
        assert_eq!(
            download.requests[0].filename,
            Some("photo.jpg".to_string())
        );
        assert!(download.requests[0].permanent);
        assert_eq!(download.requests[0].instant, Some(true));
        assert_eq!(download.match_type, Some(RsLookupMatchType::ExactId));

        let infos = download.infos.expect("expected infos");
        assert_eq!(
            infos.add_people.expect("expected add_people")[0].id,
            "coomer-creator:onlyfans/c"
        );
        assert_eq!(
            infos.people_lookup.expect("expected people_lookup")[0],
            "Creator"
        );
    }

    #[test]
    fn deduplicate_images_by_url() {
        let images = vec![
            ExternalImage {
                url: RsRequest {
                    url: "https://a.com/1.jpg".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ExternalImage {
                url: RsRequest {
                    url: "https://a.com/1.jpg".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let deduped = deduplicate_images(images);
        assert_eq!(deduped.len(), 1);
    }
}
