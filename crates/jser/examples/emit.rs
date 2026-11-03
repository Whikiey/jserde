//! Emit Rust-written `.ser` files for the Java interop verifier.
//!
//! Run from the repository root:
//!   cargo run -p jser --example emit
//! Output: `docs/testdata/build/rust/*.ser`.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use jser::builtin::new_hash_map;
use jser::{
    decode_stream_with_allowlist, encode_stream, encode_typed, Allowlist, ClassData, ClassDesc,
    FieldDesc, FieldType, Graph, JValue,
};

type BoxError = Box<dyn std::error::Error>;

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
    let out = testdata.join("build/rust");
    fs::create_dir_all(&out)?;

    let point = fs::read(testdata.join("fixtures/object/20-object-point.ser"))?;
    fs::write(out.join("point.ser"), encode_stream(&decode(&point)?)?)?;

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
