//! Emit Rust-written `.ser` files for the Java interop verifier.
//!
//! Run from the repository root:
//!   cargo run -p jser --example emit
//! Output: `docs/testdata/build/rust/*.ser` (override with `JSER_EMIT_DIR`).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use jser::builtin::new_hash_map;
use jser::{
    decode_stream_with_allowlist, encode_stream, encode_typed, Allowlist, ClassData, ClassDesc,
    FieldDesc, FieldType, Graph, JValue,
};
use jser_derive::JavaSerializable;

type BoxError = Box<dyn std::error::Error>;

#[derive(JavaSerializable, Clone)]
#[java(class = "Fixtures$Point", suid = 1)]
struct DerivedPoint {
    #[java(rename = "x")]
    x: i32,
    #[java(rename = "y")]
    y: i32,
}

fn fixture_allowlist() -> Allowlist {
    Allowlist::strict()
        .allow("Fixtures$*")
        .unwrap()
        .allow("java.math.*")
        .unwrap()
        .allow("java.time.*")
        .unwrap()
}

fn decode(bytes: &[u8]) -> Result<Graph, BoxError> {
    Ok(decode_stream_with_allowlist(bytes, &fixture_allowlist())?)
}

fn main() -> Result<(), BoxError> {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/testdata");
    let out = std::env::var_os("JSER_EMIT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| testdata.join("build/rust"));
    fs::create_dir_all(&out)
        .map_err(|error| format!("create_dir_all {}: {error}", out.display()))?;

    let point = fs::read(testdata.join("fixtures/object/20-object-point.ser"))
        .map_err(|error| format!("read point fixture: {error}"))?;
    fs::write(out.join("point.ser"), encode_stream(&decode(&point)?)?)
        .map_err(|error| format!("write point.ser: {error}"))?;

    let mut mutated = decode(&point)?;
    let value = mutated.items()[0].clone();
    mutated.set_field(&value, "x", JValue::Int(99))?;
    fs::write(out.join("point-mutated.ser"), encode_stream(&mutated)?)?;

    fs::write(
        out.join("point-constructed.ser"),
        encode_stream(&constructed_point()?)?,
    )?;

    let mixed = fs::read(testdata.join("fixtures/object/21-object-mixed-fields.ser"))?;
    fs::write(out.join("mixed.ser"), encode_stream(&decode(&mixed)?)?)?;

    let map = fs::read(testdata.join("fixtures/object/29-collection-hashmap.ser"))?;
    fs::write(out.join("map.ser"), encode_stream(&decode(&map)?)?)?;

    let mut mutated_map = decode(&map)?;
    let value = mutated_map.items()[0].clone();
    let entry = {
        let view = mutated_map.as_map(&value)?.unwrap();
        view.entries[0].1.clone()
    };
    let key = mutated_map.new_string("four");
    mutated_map.insert_map_entry(&value, JValue::Str(key), entry)?;
    fs::write(out.join("map-mutated.ser"), encode_stream(&mutated_map)?)?;

    let cycle = fs::read(testdata.join("fixtures/object/34-ref-cycle.ser"))?;
    fs::write(out.join("node-cycle.ser"), encode_stream(&decode(&cycle)?)?)?;

    let color = fs::read(testdata.join("fixtures/object/24-object-enum.ser"))?;
    fs::write(out.join("enum.ser"), encode_stream(&decode(&color)?)?)?;

    let mut constructed_map = Graph::new();
    let map = new_hash_map(&mut constructed_map, &[("alpha", "one"), ("beta", "two")])?;
    constructed_map.add_item(JValue::Obj(map));
    fs::write(
        out.join("map-constructed.ser"),
        encode_stream(&constructed_map)?,
    )?;

    let typed_map: HashMap<String, i32> = [("alpha".to_owned(), 1), ("beta".to_owned(), 2)]
        .into_iter()
        .collect();
    fs::write(out.join("typed-map.ser"), encode_typed(typed_map)?)?;

    fs::write(out.join("typed-list.ser"), encode_typed(vec![1i32, 2, 3])?)?;
    fs::write(
        out.join("point-derived.ser"),
        encode_typed(DerivedPoint { x: 9, y: 10 })?,
    )?;

    // AC-02 coverage: re-encode every committed fixture so Java can read back
    // one Rust-written stream per fixture type.
    let all = out.join("all");
    fs::create_dir_all(&all)?;
    for tier in ["codec", "object"] {
        let dir = testdata.join("fixtures").join(tier);
        let mut files = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|ext| ext.to_str()) == Some("ser") {
                files.push(entry.path());
            }
        }
        files.sort();
        for file in files {
            let bytes = fs::read(&file)?;
            let graph = decode(&bytes)?;
            let name = file.file_name().ok_or("fixture without a file name")?;
            fs::write(all.join(name), encode_stream(&graph)?)?;
        }
    }

    Ok(())
}

fn constructed_point() -> Result<Graph, BoxError> {
    let mut graph = Graph::new();
    let desc = graph.add_class_desc(ClassDesc {
        name: "Fixtures$Point".to_owned(),
        suid: 1,
        flags: 0x02,
        fields: vec![
            FieldDesc {
                name: "x".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            },
            FieldDesc {
                name: "y".to_owned(),
                typecode: FieldType::Int,
                class_name1: None,
                class_name_handle: None,
            },
        ],
        super_desc: None,
        annotations: Vec::new(),
        proxy_interfaces: None,
    })?;

    let object = graph.new_object(vec![ClassData {
        desc,
        fields: vec![
            ("x".to_owned(), JValue::Int(7)),
            ("y".to_owned(), JValue::Int(8)),
        ],
        custom: None,
    }])?;
    graph.add_item(JValue::Obj(object));
    Ok(graph)
}
