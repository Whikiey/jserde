//! The committed fixture manifest is the contract between Java-generated data
//! and the Rust decoder. These tests only validate the manifest and files.

mod common;

use common::{fixture_path, load_manifest, testdata_dir};
use std::collections::HashSet;
use std::fs;

#[test]
fn manifest_declares_32_fixture_cases() {
    let manifest = load_manifest();

    assert_eq!(manifest.schema, "jser-fixtures/1");
    assert_eq!(manifest.case_count, 32);
    assert_eq!(manifest.cases.len(), manifest.case_count);
    assert!(!manifest.jdk.is_empty());
    assert!(!manifest.generator.is_empty());
    assert!(!manifest.cross_check.is_empty());
}

#[test]
fn case_ids_are_unique_and_paths_are_safe() {
    let manifest = load_manifest();
    let mut ids = HashSet::new();

    for case in &manifest.cases {
        assert!(
            ids.insert(case.id.as_str()),
            "duplicate case id: {}",
            case.id
        );
        assert!(!case.id.is_empty(), "case id must not be empty");
        assert!(
            case.file.starts_with("fixtures/"),
            "{} must reference fixtures/: {}",
            case.id,
            case.file
        );
        assert!(
            !case.file.contains(".."),
            "{} must not escape testdata: {}",
            case.id,
            case.file
        );
        assert!(!case.case.is_empty(), "{} has no description", case.id);
        assert!(!case.features.is_empty(), "{} has no features", case.id);
    }
}

#[test]
fn fixtures_exist_and_match_manifest_sizes() {
    let manifest = load_manifest();
    let mut total_bytes = 0usize;

    for case in &manifest.cases {
        let path = fixture_path(case);
        let metadata = fs::metadata(&path)
            .unwrap_or_else(|err| panic!("missing fixture {} ({err})", path.display()));

        assert_eq!(
            metadata.len() as usize,
            case.size,
            "size mismatch for {}",
            case.id
        );
        assert_eq!(
            case.sha256.len(),
            64,
            "{} must carry a SHA-256 hex digest",
            case.id
        );
        assert!(
            case.sha256.chars().all(|ch| ch.is_ascii_hexdigit()),
            "{} has a non-hex SHA-256 digest",
            case.id
        );

        total_bytes += case.size;
    }

    assert_eq!(total_bytes, manifest.total_bytes);
    assert!(testdata_dir().is_dir());
}

#[test]
fn every_fixture_starts_with_the_stream_header() {
    let manifest = load_manifest();

    for case in &manifest.cases {
        let path = fixture_path(case);
        let bytes =
            fs::read(&path).unwrap_or_else(|err| panic!("cannot read {} ({err})", path.display()));

        assert!(
            bytes.len() >= 4,
            "{} is shorter than the stream header",
            case.id
        );
        assert_eq!(
            &bytes[..4],
            &[0xAC, 0xED, 0x00, 0x05],
            "{} does not start with AC ED 00 05",
            case.id
        );
    }
}
