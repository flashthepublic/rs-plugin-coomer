use extism::*;
use rs_plugin_common_interfaces::lookup::{
    RsLookupMetadataResult, RsLookupMetadataResults, RsLookupPerson, RsLookupQuery,
    RsLookupSourceResult, RsLookupWrapper,
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

#[test]
fn test_lookup_empty_name_returns_404() {
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

#[test]
fn test_lookup_creator_listing_live() {
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
fn test_lookup_creator_url_format_live() {
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
        "Expected at least one result for URL format"
    );
    println!(
        "URL format listing returned {} results",
        results.results.len()
    );
}

#[test]
fn test_lookup_returns_group_download() {
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
fn test_lookup_metadata_creator_returns_next_page_key() {
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
fn test_lookup_metadata_page_2_returns_different_results() {
    let mut plugin = build_plugin();

    let page1_input = RsLookupWrapper {
        query: RsLookupQuery::Person(RsLookupPerson {
            name: Some("coomer:onlyfans/belledelphine".to_string()),
            ids: None,
            page_key: None,
        }),
        credential: None,
        params: None,
    };

    let page1 = call_lookup(&mut plugin, &page1_input);
    assert!(!page1.results.is_empty(), "Expected page 1 results");

    let page2_input = RsLookupWrapper {
        query: RsLookupQuery::Person(RsLookupPerson {
            name: Some("coomer:onlyfans/belledelphine".to_string()),
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
