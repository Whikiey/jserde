//! P3 Java interop: Rust-written streams must be readable by
//! `ObjectInputStream`. Skipped when no JDK is available (CI runs the same flow
//! in the `interop` job via `docs/testdata/gen/interop.sh`).

mod common;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use common::{decode_fixture, load_manifest, read_fixture, testdata_dir};
use jser::builtin::new_hash_map;
use jser::{
    encode_stream, encode_typed, ClassData, ClassDesc, FieldDesc, FieldType, Graph, JValue,
};

fn has(command: &str) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_java(classes: &Path, file: &Path, class: &str, fields: &[&str]) {
    let mut command = Command::new("java");
    command
        .arg("-cp")
        .arg(classes)
        .arg("Verify")
        .arg(file)
        .arg(class);
    for field in fields {
        command.arg(field);
    }

    let output = command.output().expect("run java Verify");
    assert!(
        output.status.success(),
        "java Verify failed for {}:\nstdout: {}\nstderr: {}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_java_read(classes: &Path, file: &Path) {
    let output = Command::new("java")
        .arg("-cp")
        .arg(classes)
        .arg("Verify")
        .arg("--read")
        .arg(file)
        .output()
        .expect("run java Verify --read");
    assert!(
        output.status.success(),
        "java Verify --read failed for {}:\nstdout: {}\nstderr: {}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_java_primitives(classes: &Path, file: &Path, specs: &[&str]) {
    let mut command = Command::new("java");
    command
        .arg("-cp")
        .arg(classes)
        .arg("Verify")
        .arg("--primitives")
        .arg(file);
    command.args(specs);
    let output = command.output().expect("run java Verify --primitives");
    assert!(
        output.status.success(),
        "java Verify --primitives failed for {}:\nstdout: {}\nstderr: {}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_java_blocks(classes: &Path, file: &Path, length: usize) {
    let output = Command::new("java")
        .arg("-cp")
        .arg(classes)
        .arg("Verify")
        .arg("--blocks")
        .arg(file)
        .arg(length.to_string())
        .output()
        .expect("run java Verify --blocks");
    assert!(
        output.status.success(),
        "java Verify --blocks failed for {}:\nstdout: {}\nstderr: {}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_java_array(classes: &Path, file: &Path, array_class: &str, elements: &[&str]) {
    let mut command = Command::new("java");
    command
        .arg("-cp")
        .arg(classes)
        .arg("Verify")
        .arg("--array")
        .arg(file)
        .arg(array_class);
    command.args(elements);
    let output = command.output().expect("run java Verify --array");
    assert!(
        output.status.success(),
        "java Verify --array failed for {}:\nstdout: {}\nstderr: {}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn constructed_point() -> Graph {
    let mut graph = Graph::new();
    let desc = graph
        .add_class_desc(ClassDesc {
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
        })
        .unwrap();

    let object = graph
        .new_object(vec![ClassData {
            desc,
            fields: vec![
                ("x".to_owned(), JValue::Int(7)),
                ("y".to_owned(), JValue::Int(8)),
            ],
            custom: None,
        }])
        .unwrap();
    graph.add_item(JValue::Obj(object));
    graph
}

#[test]
fn java_can_read_rust_written_streams() {
    if !has("java") || !has("javac") {
        eprintln!("skipping Java interop test: JDK not available");
        return;
    }

    let testdata = testdata_dir();
    let classes = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("jser-interop");
    let _ = fs::remove_dir_all(&classes);
    fs::create_dir_all(&classes).unwrap();

    let status = Command::new("javac")
        .arg("-d")
        .arg(&classes)
        .arg(testdata.join("gen/Fixtures.java"))
        .arg(testdata.join("gen/Verify.java"))
        .status()
        .expect("run javac");
    assert!(status.success(), "javac failed");

    let point = read_fixture("object/20");
    fs::write(
        classes.join("point.ser"),
        encode_stream(&decode_fixture(&point).unwrap()).unwrap(),
    )
    .unwrap();

    let mut mutated = decode_fixture(&point).unwrap();
    let value = mutated.items()[0].clone();
    mutated.set_field(&value, "x", JValue::Int(99)).unwrap();
    fs::write(
        classes.join("point-mutated.ser"),
        encode_stream(&mutated).unwrap(),
    )
    .unwrap();

    let mixed = read_fixture("object/21");
    let mut mutated_mixed = decode_fixture(&mixed).unwrap();
    let value = mutated_mixed.items()[0].clone();
    mutated_mixed
        .set_field(&value, "i", JValue::Int(42))
        .unwrap();
    let text = mutated_mixed.new_string("changed");
    mutated_mixed
        .set_field(&value, "s", JValue::Str(text))
        .unwrap();
    mutated_mixed
        .set_field(&value, "b", JValue::Bool(false))
        .unwrap();
    fs::write(
        classes.join("mixed-mutated.ser"),
        encode_stream(&mutated_mixed).unwrap(),
    )
    .unwrap();

    fs::write(
        classes.join("point-constructed.ser"),
        encode_stream(&constructed_point()).unwrap(),
    )
    .unwrap();

    let map = read_fixture("object/29");
    fs::write(
        classes.join("map.ser"),
        encode_stream(&decode_fixture(&map).unwrap()).unwrap(),
    )
    .unwrap();

    let mut mutated_map = decode_fixture(&map).unwrap();
    let value = mutated_map.items()[0].clone();
    let entry = {
        let view = mutated_map.as_map(&value).unwrap().unwrap();
        view.entries[0].1.clone()
    };
    let key = mutated_map.new_string("four");
    mutated_map
        .insert_map_entry(&value, JValue::Str(key), entry)
        .unwrap();
    fs::write(
        classes.join("map-mutated.ser"),
        encode_stream(&mutated_map).unwrap(),
    )
    .unwrap();

    let mut constructed_map = Graph::new();
    let map = new_hash_map(&mut constructed_map, &[("alpha", "one"), ("beta", "two")]).unwrap();
    constructed_map.add_item(JValue::Obj(map));
    fs::write(
        classes.join("map-constructed.ser"),
        encode_stream(&constructed_map).unwrap(),
    )
    .unwrap();
    let typed_map: HashMap<String, i32> = [("alpha".to_owned(), 1), ("beta".to_owned(), 2)]
        .into_iter()
        .collect();
    fs::write(
        classes.join("typed-map.ser"),
        encode_typed(typed_map).unwrap(),
    )
    .unwrap();
    fs::write(
        classes.join("typed-list.ser"),
        encode_typed(vec![1i32, 2, 3]).unwrap(),
    )
    .unwrap();

    run_java(
        &classes,
        &classes.join("point.ser"),
        "Fixtures$Point",
        &["x=1", "y=2"],
    );
    run_java(
        &classes,
        &classes.join("point-mutated.ser"),
        "Fixtures$Point",
        &["x=99", "y=2"],
    );
    run_java(
        &classes,
        &classes.join("mixed-mutated.ser"),
        "Fixtures$Mixed",
        &["i=42", "s=changed", "b=false"],
    );
    run_java(
        &classes,
        &classes.join("point-constructed.ser"),
        "Fixtures$Point",
        &["x=7", "y=8"],
    );
    run_java(
        &classes,
        &classes.join("map.ser"),
        "java.util.HashMap",
        &["one=1", "two=2", "three=3"],
    );
    run_java(
        &classes,
        &classes.join("map-mutated.ser"),
        "java.util.HashMap",
        &["one=1", "two=2", "three=3", "four=1"],
    );
    run_java(
        &classes,
        &classes.join("map-constructed.ser"),
        "java.util.HashMap",
        &["alpha=one", "beta=two"],
    );
    run_java(
        &classes,
        &classes.join("typed-map.ser"),
        "java.util.HashMap",
        &["alpha=1", "beta=2"],
    );
    run_java(
        &classes,
        &classes.join("typed-list.ser"),
        "java.util.ArrayList",
        &["0=1", "1=2", "2=3"],
    );

    // AC-02 coverage: Java reads back one Rust-re-encoded stream per object-like
    // fixture (primitive/block streams are covered below and by interop.sh).
    for case in load_manifest().cases {
        let kind = case
            .expect
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if matches!(kind, "primitive_stream" | "blockdata") {
            continue;
        }
        let bytes = read_fixture(&case.id);
        let encoded = encode_stream(&decode_fixture(&bytes).unwrap()).unwrap();
        let file = classes.join(format!("all-{}.ser", case.id.replace('/', "-")));
        fs::write(&file, encoded).unwrap();
        run_java_read(&classes, &file);
    }

    // Primitive and block-data streams use dedicated ObjectInputStream reads.
    let primitives = encode_stream(&decode_fixture(&read_fixture("codec/09")).unwrap()).unwrap();
    let file = classes.join("all-codec-09.ser");
    fs::write(&file, primitives).unwrap();
    run_java_primitives(&classes, &file, &["i=1", "z=true", "d=1.5"]);

    let blocks = encode_stream(&decode_fixture(&read_fixture("codec/11")).unwrap()).unwrap();
    let file = classes.join("all-codec-11.ser");
    fs::write(&file, blocks).unwrap();
    run_java_blocks(&classes, &file, 2000);

    // Built-in classes deserialize to their Java types; check array elements.
    for (id, elements) in [
        (
            "object/36",
            &[
                "java.util.Date",
                "java.math.BigInteger",
                "java.math.BigDecimal",
                "java.time.LocalDate",
            ][..],
        ),
        (
            "object/40",
            &[
                "java.time.Duration",
                "java.time.Instant",
                "java.time.LocalDate",
                "java.time.LocalDateTime",
                "java.time.ZonedDateTime",
            ][..],
        ),
        (
            "object/42",
            &[
                "java.time.Year",
                "java.time.YearMonth",
                "java.time.MonthDay",
                "java.time.Period",
            ][..],
        ),
        (
            "object/43",
            &[
                "java.time.ZoneRegion",
                "java.time.ZoneOffset",
                "java.time.OffsetTime",
                "java.time.OffsetDateTime",
            ][..],
        ),
    ] {
        let bytes = read_fixture(id);
        let encoded = encode_stream(&decode_fixture(&bytes).unwrap()).unwrap();
        let file = classes.join(format!("all-{}.ser", id.replace('/', "-")));
        fs::write(&file, encoded).unwrap();
        run_java_array(&classes, &file, "[Ljava.lang.Object;", elements);
    }
}
