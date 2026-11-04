//! The committed fixture manifest is the contract between Java-generated data
//! and the Rust decoder. These tests only validate the manifest and files.

mod common;

use common::{fixture_path, load_manifest, testdata_dir};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn manifest_declares_40_fixture_cases() {
    let manifest = load_manifest();

    assert_eq!(manifest.schema, "jser-fixtures/1");
    assert_eq!(manifest.case_count, 40);
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
        assert!(
            case.file.ends_with(".ser"),
            "{} must reference a .ser file: {}",
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

#[test]
fn case_ids_match_their_fixture_tier() {
    let manifest = load_manifest();

    for case in &manifest.cases {
        let (tier, _) = case
            .id
            .split_once('/')
            .unwrap_or_else(|| panic!("case id has no tier: {}", case.id));
        assert!(
            case.file.starts_with(&format!("fixtures/{tier}/")),
            "{} points outside its tier: {}",
            case.id,
            case.file
        );
    }
}

#[test]
fn expectations_use_known_kinds() {
    const KINDS: &[&str] = &[
        "array",
        "blockdata",
        "cycle",
        "enum",
        "list",
        "map",
        "null",
        "object",
        "primitive_stream",
        "reset_stream",
        "sequence",
        "set",
        "string",
    ];

    for case in &load_manifest().cases {
        let kind = case
            .expect
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{} has no expect.kind", case.id));
        assert!(
            KINDS.contains(&kind),
            "{} uses unknown expect.kind `{kind}`",
            case.id
        );
    }
}

#[test]
fn every_ser_file_is_registered_in_the_manifest() {
    let manifest = load_manifest();
    let testdata = testdata_dir();
    let registered: HashSet<String> = manifest
        .cases
        .iter()
        .map(|case| case.file.clone())
        .collect();

    let mut found = Vec::new();
    collect_ser_files(&testdata.join("fixtures"), &mut found);
    assert_eq!(found.len(), manifest.cases.len());

    for path in found {
        let relative = path
            .strip_prefix(&testdata)
            .expect("fixture must live under docs/testdata")
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            registered.contains(&relative),
            "unregistered fixture: {relative}"
        );
    }
}

fn collect_ser_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("cannot read fixture directory {}: {err}", dir.display()));

    for entry in entries {
        let entry =
            entry.unwrap_or_else(|err| panic!("cannot read entry in {}: {err}", dir.display()));
        let path = entry.path();

        if path.is_dir() {
            collect_ser_files(&path, found);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("ser") {
            found.push(path);
        }
    }
}
