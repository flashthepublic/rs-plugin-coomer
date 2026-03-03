use extism::*;
use rs_plugin_common_interfaces::lookup::{
    RsLookupMedia, RsLookupMetadataResult, RsLookupMetadataResults, RsLookupPerson,
    RsLookupQuery, RsLookupSourceResult, RsLookupWrapper,
};

fn build_plugin() -> Plugin {
    let wasm = Wasm::file("target/wasm32-unknown-unknown/release/rs_plugin_coomer.wasm");
    let manifest = Manifest::new([wasm]).with_allowed_host("coomer.st");
    Plugin::new(&manifest, [], true).expect("Failed to create plugin")
}

fn call_lookup_source(plugin: &mut Plugin, input: &RsLookupWrapper) -> RsLookupSourceResult {
    let input_str = serde_json::to_string(input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup", &input_str)
        .expect("lookup call failed");
    serde_json::from_slice(output).expect("Failed to parse lookup source result")
}

fn call_lookup(plugin: &mut Plugin, input: &RsLookupWrapper) -> RsLookupMetadataResults {
    let input_str = serde_json::to_string(input).unwrap();
    let output = plugin
        .call::<&str, &[u8]>("lookup_metadata", &input_str)
        .expect("lookup_metadata call failed");
    serde_json::from_slice(output).expect("Failed to parse lookup output")
}

// ---- Person lookup tests ----

#[test]
fn test_lookup_person_returns_person_metadata() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Person(RsLookupPerson {
            name: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for person lookup"
    );

    let first = &results.results[0];
    let person = match &first.metadata {
        RsLookupMetadataResult::Person(person) => person,
        other => panic!("Expected Person metadata, got {:?}", other),
    };
    assert!(
        !person.name.trim().is_empty(),
        "Expected a non-empty name in person result"
    );
    assert!(
        person.id.starts_with("coomer-creator:"),
        "Expected person id to start with coomer-creator:, got: {}",
        person.id
    );
    assert!(
        person.portrait.is_some(),
        "Expected person to have a portrait URL"
    );

    println!(
        "Person lookup returned: {} (id: {}, portrait: {:?})",
        person.name, person.id, person.portrait
    );
}

#[test]
fn test_lookup_person_url_format() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Person(RsLookupPerson {
            name: Some("https://coomer.st/onlyfans/user/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for URL format person lookup"
    );
    assert!(
        matches!(&results.results[0].metadata, RsLookupMetadataResult::Person(_)),
        "Expected Person metadata for URL format"
    );
    println!("Person URL format lookup returned {} results", results.results.len());
}

#[test]
fn test_lookup_person_empty_name_returns_404() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Person(RsLookupPerson {
            name: Some(String::new()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let input_str = serde_json::to_string(&input).unwrap();
    let error = plugin
        .call::<&str, &[u8]>("lookup_metadata", &input_str)
        .expect_err("Expected 404 error for empty name");

    let message = error.to_string();
    assert!(
        message.contains("Not supported") || message.contains("404"),
        "Expected error message to mention 404/Not supported, got: {message}"
    );
}

// ---- Media lookup tests ----

#[test]
fn test_lookup_media_creator_listing_live() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Media(RsLookupMedia {
            search: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for belledelphine"
    );

    let first = &results.results[0];
    let media = match &first.metadata {
        RsLookupMetadataResult::Media(media) => media,
        other => panic!("Expected Media metadata, got {:?}", other),
    };
    assert!(
        !media.name.trim().is_empty(),
        "Expected a non-empty name in the first result"
    );

    println!(
        "Creator listing returned {} results, first: {}",
        results.results.len(),
        media.name
    );
}

#[test]
fn test_lookup_media_url_format_live() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Media(RsLookupMedia {
            search: Some("https://coomer.st/onlyfans/user/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result for URL format"
    );
    println!(
        "URL format listing returned {} results",
        results.results.len()
    );
}

#[test]
fn test_lookup_media_returns_group_download() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Media(RsLookupMedia {
            search: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let result = call_lookup_source(&mut plugin, &input);
    let RsLookupSourceResult::GroupRequest(groups) = result else {
        panic!("Expected GroupRequest for creator listing");
    };
    assert!(
        !groups.is_empty(),
        "Expected at least one group from creator listing"
    );
    assert!(
        groups.iter().all(|g| g.group),
        "Expected all results to have group flag set"
    );
    println!(
        "Creator lookup returned {} groups, first has {} requests",
        groups.len(),
        groups[0].requests.len()
    );
}

#[test]
fn test_lookup_media_creator_returns_next_page_key() {
    let mut plugin = build_plugin();

    let input = RsLookupWrapper {
        query: RsLookupQuery::Media(RsLookupMedia {
            search: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let results = call_lookup(&mut plugin, &input);
    assert!(
        !results.results.is_empty(),
        "Expected at least one result"
    );
    // belledelphine should have enough posts for pagination
    assert_eq!(
        results.next_page_key,
        Some("50".to_string()),
        "Expected next_page_key to be '50' for the first page"
    );
    println!(
        "Search returned {} results, next_page_key: {:?}",
        results.results.len(),
        results.next_page_key
    );
}

#[test]
fn test_lookup_media_page_2_returns_different_results() {
    let mut plugin = build_plugin();

    let page1_input = RsLookupWrapper {
        query: RsLookupQuery::Media(RsLookupMedia {
            search: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let page1 = call_lookup(&mut plugin, &page1_input);
    assert!(!page1.results.is_empty(), "Expected page 1 results");

    let page2_input = RsLookupWrapper {
        query: RsLookupQuery::Media(RsLookupMedia {
            search: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: Some("50".to_string()),
        }),
        credential: None,
        params: None,
    };

    let page2 = call_lookup(&mut plugin, &page2_input);
    assert!(!page2.results.is_empty(), "Expected page 2 results");

    let page1_first_id = match &page1.results[0].metadata {
        RsLookupMetadataResult::Media(media) => media.id.clone(),
        _ => panic!("Expected Media metadata on page 1"),
    };
    let page2_first_id = match &page2.results[0].metadata {
        RsLookupMetadataResult::Media(media) => media.id.clone(),
        _ => panic!("Expected Media metadata on page 2"),
    };

    assert_ne!(
        page1_first_id, page2_first_id,
        "Expected page 1 and page 2 to return different results"
    );
    println!(
        "Page 1 first: {}, Page 2 first: {}, Page 2 next_page_key: {:?}",
        page1_first_id, page2_first_id, page2.next_page_key
    );
}
