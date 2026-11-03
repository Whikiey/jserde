//! P2 container fixtures (21, 24–32, 36, 38): arrays, enums and the built-in
//! collection layouts, all compared against `manifest.expect`.

mod common;

use common::{load_manifest, read_fixture};
use jser::{decode_stream, Graph, JValue, MapView};

fn graph(id: &str) -> Graph {
    decode_stream(&read_fixture(id)).unwrap_or_else(|err| panic!("{id}: {err}"))
}

fn expect(id: &str) -> serde_json::Value {
    load_manifest()
        .cases
        .into_iter()
        .find(|case| case.id == id)
        .unwrap_or_else(|| panic!("manifest has no case `{id}`"))
        .expect
}

fn object<'a>(graph: &'a Graph, value: &JValue) -> &'a jser::JObject {
    graph
        .as_object(value)
        .unwrap_or_else(|err| panic!("expected object: {err}"))
        .unwrap_or_else(|| panic!("expected object, got {value:?}"))
}

fn field<'a>(graph: &'a Graph, value: &'a JValue, name: &str) -> &'a JValue {
    graph
        .field(value, name)
        .unwrap_or_else(|err| panic!("field `{name}`: {err}"))
        .unwrap_or_else(|| panic!("missing field `{name}`"))
}

fn assert_json(graph: &Graph, value: &JValue, expected: &serde_json::Value, context: &str) {
    match expected {
        serde_json::Value::Null => assert!(
            matches!(value, JValue::Null),
            "{context}: expected null, got {value:?}"
        ),
        serde_json::Value::Bool(expected) => assert_eq!(
            graph.as_bool(value).unwrap(),
            *expected,
            "{context}: bool mismatch"
        ),
        serde_json::Value::Number(number) if number.is_i64() => assert_eq!(
            graph.as_i64(value).unwrap(),
            number.as_i64().unwrap(),
            "{context}: integer mismatch"
        ),
        serde_json::Value::Number(number) => {
            let expected = number.as_f64().unwrap();
            let actual = graph.as_f64(value).unwrap();
            assert!(
                (actual - expected).abs() < f64::EPSILON,
                "{context}: float mismatch: {actual} != {expected}"
            );
        }
        serde_json::Value::String(expected) => {
            if let JValue::Char(unit) = value {
                let actual = char::from_u32(u32::from(*unit)).unwrap().to_string();
                assert_eq!(&actual, expected, "{context}: char mismatch");
            } else {
                assert_eq!(
                    graph.as_str(value).unwrap(),
                    expected,
                    "{context}: string mismatch"
                );
            }
        }
        serde_json::Value::Array(expected) => {
            let actual = graph
                .as_array(value)
                .unwrap()
                .unwrap_or_else(|| panic!("{context}: expected an array, got {value:?}"));
            assert_eq!(actual.len(), expected.len(), "{context}: array length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_json(graph, actual, expected, &format!("{context}[{index}]"));
            }
        }
        serde_json::Value::Object(_) => panic!("{context}: nested object needs assert_map"),
    }
}

fn map_entry<'a>(graph: &Graph, view: &'a MapView<'a>, key: &str) -> (&'a JValue, &'a JValue) {
    view.entries
        .iter()
        .find(|(candidate, _)| graph.as_str(candidate).ok() == Some(key))
        .copied()
        .unwrap_or_else(|| panic!("map has no key `{key}`"))
}

fn assert_map(graph: &Graph, value: &JValue, expected: &serde_json::Value, context: &str) {
    let view = graph
        .as_map(value)
        .unwrap()
        .unwrap_or_else(|| panic!("{context}: expected a map, got {value:?}"));

    if let Some(capacity) = expected.get("capacity").and_then(serde_json::Value::as_i64) {
        assert_eq!(i64::from(view.capacity), capacity, "{context}: capacity");
    }
    if let Some(load_factor) = expected
        .get("load_factor")
        .and_then(serde_json::Value::as_f64)
    {
        assert!(
            (f64::from(view.load_factor) - load_factor).abs() < f64::EPSILON,
            "{context}: load factor"
        );
    }

    if let Some(entries) = expected
        .get("entries")
        .and_then(serde_json::Value::as_object)
    {
        for (key, expected_value) in entries {
            let (_, actual_value) = map_entry(graph, &view, key);
            if expected_value.is_object() {
                assert_map(
                    graph,
                    actual_value,
                    expected_value,
                    &format!("{context}.{key}"),
                );
            } else {
                assert_json(
                    graph,
                    actual_value,
                    expected_value,
                    &format!("{context}.{key}"),
                );
            }
        }
    }

    if let Some(entries) = expected
        .get("ordered_entries")
        .and_then(serde_json::Value::as_array)
    {
        assert_eq!(view.entries.len(), entries.len(), "{context}: entry count");
        for (index, pair) in entries.iter().enumerate() {
            let pair = pair.as_array().expect("ordered entry must be a pair");
            let (actual_key, actual_value) = view.entries[index];
            assert_json(
                graph,
                actual_key,
                &pair[0],
                &format!("{context}[{index}].key"),
            );
            assert_json(
                graph,
                actual_value,
                &pair[1],
                &format!("{context}[{index}].value"),
            );
        }
    }
}

fn assert_values(graph: &Graph, value: &JValue, expected: &serde_json::Value, context: &str) {
    let expected = expected["values"]
        .as_array()
        .unwrap_or_else(|| panic!("{context}: manifest has no `values` array"));
    let values = graph
        .as_list(value)
        .unwrap()
        .or_else(|| graph.as_set(value).unwrap())
        .unwrap_or_else(|| panic!("{context}: expected a list/set, got {value:?}"));

    assert_eq!(values.len(), expected.len(), "{context}: element count");
    for (index, (actual, expected)) in values.iter().zip(expected).enumerate() {
        assert_json(graph, actual, expected, &format!("{context}[{index}]"));
    }
}

#[test]
fn decodes_mixed_fields_in_descriptor_order() {
    let graph = graph("object/21");
    let value = &graph.items()[0];
    let data = object(&graph, value);
    let expected = expect("object/21");

    let order: Vec<&str> = data.classes[0]
        .fields
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let expected_order: Vec<&str> = expected["field_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert_eq!(order, expected_order);

    for (name, expected_value) in expected["fields"].as_object().unwrap() {
        assert_json(
            &graph,
            field(&graph, value, name),
            expected_value,
            &format!("fields.{name}"),
        );
    }
}

#[test]
fn decodes_enum_constants() {
    let graph = graph("object/24");
    let value = &graph.items()[0];
    let expected = expect("object/24");

    assert_eq!(
        graph.class_name(value).unwrap(),
        expected["class"].as_str().unwrap()
    );
    assert_eq!(
        graph.as_enum_constant(value).unwrap().unwrap(),
        expected["constant"].as_str().unwrap()
    );

    let desc = graph.descriptor_of(value).unwrap();
    assert_eq!(desc.suid, 0);
    assert_eq!(desc.flags & 0x10, 0x10, "SC_ENUM must be set");
    assert_eq!(desc.flags & 0x02, 0x02, "SC_SERIALIZABLE must be set");
}

#[test]
fn decodes_primitive_and_nested_arrays() {
    for (id, name) in [("object/25", "[I"), ("object/26", "[Ljava.lang.String;")] {
        let graph = graph(id);
        let value = &graph.items()[0];
        let expected = expect(id);

        assert_eq!(graph.class_name(value).unwrap(), name);
        let values = graph.as_array(value).unwrap().unwrap();
        let expected_values = expected["values"].as_array().unwrap();
        assert_eq!(values.len(), expected_values.len());
        for (index, (actual, expected)) in values.iter().zip(expected_values).enumerate() {
            assert_json(&graph, actual, expected, &format!("{id}[{index}]"));
        }
    }

    let graph = graph("object/27");
    let value = &graph.items()[0];
    let expected = expect("object/27");
    assert_eq!(graph.class_name(value).unwrap(), "[[I");
    assert_json(&graph, value, &expected["values"], "array/27");

    let outer = graph.as_array(value).unwrap().unwrap();
    assert_eq!(outer.len(), 2);
    assert_eq!(graph.class_name(&outer[0]).unwrap(), "[I");
    assert_eq!(graph.class_name(&outer[1]).unwrap(), "[I");
}

#[test]
fn decodes_array_list_and_linked_list() {
    for id in ["object/28", "object/32"] {
        let graph = graph(id);
        let value = &graph.items()[0];
        let expected = expect(id);

        assert_eq!(
            graph.class_name(value).unwrap(),
            expected["class"].as_str().unwrap()
        );
        assert_values(&graph, value, &expected, id);
    }
}

#[test]
fn decodes_hash_map_entries_and_metadata() {
    let graph = graph("object/29");
    let value = &graph.items()[0];
    let expected = expect("object/29");

    assert_eq!(
        graph.class_name(value).unwrap(),
        expected["class"].as_str().unwrap()
    );
    let view = graph.as_map(value).unwrap().unwrap();
    assert_eq!(view.entries.len(), 3);
    assert!(!view.ordered);
    assert_map(&graph, value, &expected, "map/29");
}

#[test]
fn decodes_linked_hash_map_in_insertion_order() {
    let graph = graph("object/30");
    let value = &graph.items()[0];
    let expected = expect("object/30");

    assert_eq!(
        graph.class_name(value).unwrap(),
        expected["class"].as_str().unwrap()
    );
    let view = graph.as_map(value).unwrap().unwrap();
    assert!(view.ordered);
    assert_map(&graph, value, &expected, "map/30");

    let data = object(&graph, value);
    assert_eq!(
        graph.descriptor(data.classes[0].desc).unwrap().name,
        "java.util.HashMap",
        "classdata is ordered parent → child"
    );
    assert_eq!(
        graph.descriptor(data.classes[1].desc).unwrap().name,
        "java.util.LinkedHashMap"
    );
}

#[test]
fn decodes_hash_set() {
    let graph = graph("object/31");
    let value = &graph.items()[0];
    let expected = expect("object/31");

    assert_eq!(
        graph.class_name(value).unwrap(),
        expected["class"].as_str().unwrap()
    );
    assert_values(&graph, value, &expected, "set/31");
}

#[test]
fn decodes_nested_maps() {
    let graph = graph("object/38");
    let value = &graph.items()[0];
    let expected = expect("object/38");

    assert_map(&graph, value, &expected, "map/38");
    let view = graph.as_map(value).unwrap().unwrap();
    let (_, inner) = map_entry(&graph, &view, "outer");
    let inner = graph.as_map(inner).unwrap().unwrap();
    assert_eq!(inner.entries.len(), 1);
    assert_eq!(graph.as_i64(inner.entries[0].1).unwrap(), 7);
}

#[test]
fn decodes_misc_java_types_structurally() {
    let graph = graph("object/36");
    let value = &graph.items()[0];
    let expected = expect("object/36");

    assert_eq!(graph.class_name(value).unwrap(), "[Ljava.lang.Object;");
    let values = graph.as_array(value).unwrap().unwrap();
    assert_eq!(values.len(), expected["values"].as_array().unwrap().len());

    let names: Vec<&str> = values
        .iter()
        .map(|value| graph.class_name(value).unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "java.util.Date",
            "java.math.BigInteger",
            "java.math.BigDecimal",
            "java.time.Ser"
        ]
    );

    let ser = object(&graph, &values[3]);
    let ser_desc = graph.descriptor(ser.classes[0].desc).unwrap();
    assert_eq!(ser_desc.flags & 0x0C, 0x0C, "Externalizable + block data");
    assert!(matches!(ser.classes[0].custom, Some(ref items) if !items.is_empty()));

    let date = object(&graph, &values[0]);
    assert!(matches!(date.classes[0].custom, Some(ref items) if !items.is_empty()));
}
