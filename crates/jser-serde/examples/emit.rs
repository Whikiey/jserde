//! Emit a serde-bridge stream for the Java verifier.

use std::fs;
use std::path::PathBuf;

use jser::FieldType;
use jser_serde::{to_vec, JavaSchema, SchemaField};
use serde::Serialize;

#[derive(Serialize)]
struct Point {
    x: i32,
    y: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/testdata");
    let out = testdata.join("build/rust");
    fs::create_dir_all(&out)?;

    let schema = JavaSchema {
        class: "Fixtures$Point",
        suid: 1,
        fields: vec![
            SchemaField {
                name: "x",
                typecode: FieldType::Int,
                class_name1: None,
                nested: None,
            },
            SchemaField {
                name: "y",
                typecode: FieldType::Int,
                class_name1: None,
                nested: None,
            },
        ],
    };
    fs::write(
        out.join("point-serde.ser"),
        to_vec(&Point { x: 11, y: 12 }, &schema)?,
    )?;
    Ok(())
}
