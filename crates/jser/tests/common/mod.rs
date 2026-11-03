//! Shared helpers for integration tests.

#![allow(dead_code)]

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
