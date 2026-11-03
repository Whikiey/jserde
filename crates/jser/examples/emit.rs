//! Emit Rust-written `.ser` files for the Java interop verifier.
//!
//! Run from the repository root:
//!   cargo run -p jser --example emit
//! Output: `docs/testdata/build/rust/*.ser`.

use std::fs;
use std::path::PathBuf;

use jser::builtin::new_hash_map;
use jser::{
    decode_stream, encode_stream, ClassData, ClassDesc, FieldDesc, FieldType, Graph, JValue,
};

type BoxError = Box<dyn std::error::Error>;

fn main() -> Result<(), BoxError> {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/testdata");
    let out = testdata.join("build/rust");
    fs::create_dir_all(&out)?;

    let point = fs::read(testdata.join("fixtures/object/20-object-point.ser"))?;
    fs::write(
        out.join("point.ser"),
        encode_stream(&decode_stream(&point)?)?,
    )?;

    let mut mutated = decode_stream(&point)?;
    let value = mutated.items()[0].clone();
    mutated.set_field(&value, "x", JValue::Int(99))?;
    fs::write(out.join("point-mutated.ser"), encode_stream(&mutated)?)?;

    fs::write(
        out.join("point-constructed.ser"),
        encode_stream(&constructed_point()?)?,
    )?;

    let mixed = fs::read(testdata.join("fixtures/object/21-object-mixed-fields.ser"))?;
    fs::write(
        out.join("mixed.ser"),
        encode_stream(&decode_stream(&mixed)?)?,
    )?;

    let map = fs::read(testdata.join("fixtures/object/29-collection-hashmap.ser"))?;
    fs::write(out.join("map.ser"), encode_stream(&decode_stream(&map)?)?)?;

    let mut mutated_map = decode_stream(&map)?;
    let value = mutated_map.items()[0].clone();
    let entry = {
        let view = mutated_map.as_map(&value)?.unwrap();
        view.entries[0].1.clone()
    };
    let key = mutated_map.new_string("four");
    mutated_map.insert_map_entry(&value, JValue::Str(key), entry)?;
    fs::write(out.join("map-mutated.ser"), encode_stream(&mutated_map)?)?;

    let cycle = fs::read(testdata.join("fixtures/object/34-ref-cycle.ser"))?;
    fs::write(
        out.join("node-cycle.ser"),
        encode_stream(&decode_stream(&cycle)?)?,
    )?;

    let color = fs::read(testdata.join("fixtures/object/24-object-enum.ser"))?;
    fs::write(
        out.join("enum.ser"),
        encode_stream(&decode_stream(&color)?)?,
    )?;

    let mut constructed_map = Graph::new();
    let map = new_hash_map(&mut constructed_map, &[("alpha", "one"), ("beta", "two")])?;
    constructed_map.add_item(JValue::Obj(map));
    fs::write(
        out.join("map-constructed.ser"),
        encode_stream(&constructed_map)?,
    )?;

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
