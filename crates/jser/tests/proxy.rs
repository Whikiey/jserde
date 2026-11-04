//! P9-2: dynamic-proxy (`TC_PROXYCLASSDESC`) fidelity.

mod common;

use common::{decode_fixture, read_fixture};
use jser::encode_stream;

#[test]
fn decodes_and_replays_a_dynamic_proxy() {
    let bytes = read_fixture("object/48");
    let graph = decode_fixture(&bytes).unwrap();
    let value = &graph.items()[0];

    let object = graph.as_object(value).unwrap().unwrap();
    let proxy = object
        .classes
        .iter()
        .find(|class| {
            graph
                .descriptor(class.desc)
                .map(|desc| desc.proxy_interfaces.is_some())
                .unwrap_or(false)
        })
        .expect("proxy class layer");
    let desc = graph.descriptor(proxy.desc).unwrap();
    assert_eq!(
        desc.proxy_interfaces.as_deref(),
        Some(["Fixtures$Greeter".to_owned()].as_slice())
    );
    assert_eq!(desc.fields.len(), 0);

    let super_desc = graph.descriptor(desc.super_desc.unwrap()).unwrap();
    assert_eq!(super_desc.name, "java.lang.reflect.Proxy");

    let handler = graph.require_field(value, "h").unwrap();
    assert_eq!(
        graph.class_name(handler).unwrap(),
        "Fixtures$GreeterHandler"
    );

    assert_eq!(encode_stream(&graph).unwrap(), bytes);
}
