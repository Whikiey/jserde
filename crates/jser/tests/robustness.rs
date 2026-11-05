//! P8-6: deep object chains and concurrent read-only use.

mod common;

use std::sync::Arc;
use std::thread;

use common::{fixture_allowlist, read_fixture};
use jser::{
    decode_stream, decode_stream_with_options, encode_stream, Allowlist, ClassData, ClassDesc,
    Error, FieldDesc, FieldType, Graph, JValue, Limits,
};

fn deep_chain(depth: usize) -> Vec<u8> {
    let mut graph = Graph::new();
    let desc = graph
        .add_class_desc(ClassDesc {
            name: "Fixtures$DeepNode".to_owned(),
            suid: 1,
            flags: 0x02,
            fields: vec![FieldDesc {
                name: "next".to_owned(),
                typecode: FieldType::Object,
                class_name1: Some("LFixtures$DeepNode;".to_owned()),
                class_name_handle: None,
            }],
            super_desc: None,
            annotations: Vec::new(),
            proxy_interfaces: None,
        })
        .unwrap();

    let mut next = JValue::Null;
    for _ in 0..depth {
        let object = graph
            .new_object(vec![ClassData {
                desc,
                fields: vec![("next".to_owned(), next)],
                custom: None,
            }])
            .unwrap();
        next = JValue::Obj(object);
    }
    graph.add_item(next);
    encode_stream(&graph).unwrap()
}

#[test]
fn decodes_a_deep_object_chain() {
    let bytes = deep_chain(300);
    let graph = decode_stream_with_options(&bytes, Limits::default(), Allowlist::all()).unwrap();
    assert_eq!(graph.items().len(), 1);
    assert_eq!(
        graph.class_name(&graph.items()[0]).unwrap(),
        "Fixtures$DeepNode"
    );
}

#[test]
fn rejects_a_chain_beyond_max_depth() {
    let bytes = deep_chain(300);
    let limits = Limits {
        max_depth: 32,
        ..Limits::default()
    };
    let error = decode_stream_with_options(&bytes, limits, Allowlist::all()).unwrap_err();
    assert!(
        matches!(error, Error::LimitExceeded { kind: "depth", .. }),
        "{error:?}"
    );
}

#[test]
fn supports_concurrent_independent_decode_encode() {
    let bytes = Arc::new(read_fixture("object/29"));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let bytes = Arc::clone(&bytes);
            thread::spawn(move || {
                for _ in 0..16 {
                    let graph = decode_stream(&bytes).unwrap();
                    assert_eq!(encode_stream(&graph).unwrap(), *bytes);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn supports_concurrent_read_only_graph_use() {
    let bytes = read_fixture("object/29");
    let graph = Arc::new(decode_stream(&bytes).unwrap());
    thread::scope(|scope| {
        for _ in 0..8 {
            let graph = Arc::clone(&graph);
            let expected = bytes.clone();
            scope.spawn(move || {
                for _ in 0..16 {
                    assert_eq!(encode_stream(&graph).unwrap(), expected);
                }
            });
        }
    });
}

#[test]
fn keeps_the_strict_allowlist_during_deep_decoding() {
    let bytes = deep_chain(8);
    let error = decode_stream(&bytes).unwrap_err();
    assert!(
        matches!(error, Error::UnsupportedClass(ref name) if name == "Fixtures$DeepNode"),
        "{error:?}"
    );
    // The fixture allowlist closes over the same deep object; this guards against
    // accidentally bypassing the allowlist just because recursion is deep.
    let graph = decode_stream_with_options(&bytes, Limits::default(), fixture_allowlist()).unwrap();
    assert_eq!(graph.items().len(), 1);
}
