//! Shared helpers for integration tests.

#![allow(dead_code)]

use jser::Allowlist;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Parsed `docs/testdata/manifest.json`.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub jdk: String,
    pub generator: String,
    pub cross_check: String,
    pub case_count: usize,
    pub total_bytes: usize,
    pub cases: Vec<Case>,
}

/// One fixture case in the manifest.
#[derive(Debug, Deserialize)]
pub struct Case {
    pub id: String,
    pub file: String,
    pub case: String,
    pub features: Vec<String>,
    pub expect: serde_json::Value,
    pub size: usize,
    pub sha256: String,
}

/// Absolute path to `docs/testdata`.
pub fn testdata_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/testdata")
}

/// Absolute path to `docs/testdata/manifest.json`.
pub fn manifest_path() -> PathBuf {
    testdata_dir().join("manifest.json")
}

/// Absolute path to a fixture referenced by the manifest.
pub fn fixture_path(case: &Case) -> PathBuf {
    testdata_dir().join(&case.file)
}

/// Read and parse the manifest, with paths in panic messages.
pub fn load_manifest() -> Manifest {
    let path = manifest_path();
    let json = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()));
    serde_json::from_str(&json)
        .unwrap_or_else(|err| panic!("cannot parse {}: {err}", path.display()))
}

/// Read a fixture by manifest id, e.g. `codec/01`.
pub fn read_fixture(id: &str) -> Vec<u8> {
    let manifest = load_manifest();
    let case = manifest
        .cases
        .iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("manifest has no case `{id}`"));
    let path = fixture_path(case);
    fs::read(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

/// Allowlist used by fixture tests: strict defaults plus the test classes.
pub fn fixture_allowlist() -> Allowlist {
    let mut allowlist = Allowlist::strict();
    for pattern in [
        "Fixtures$*",
        "java.math.*",
        "java.time.*",
        "java.sql.*",
        "java.util.regex.*",
    ] {
        allowlist.add(pattern).expect("valid fixture allowlist");
    }
    allowlist
}

/// Decode a committed fixture with the fixture allowlist.
pub fn decode_fixture(bytes: &[u8]) -> jser::Result<jser::Graph> {
    jser::decode_stream_with_allowlist(bytes, &fixture_allowlist())
}
