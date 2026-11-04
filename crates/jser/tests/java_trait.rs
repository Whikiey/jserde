//! P5-1: a hand-written JavaSerializable implementation, independent of derive.

mod common;

use common::fixture_allowlist;
use jser::{
    decode_typed_with_allowlist, encode_typed, ClassData, ClassDesc, FieldDesc, FieldType,
    FromJValue, Graph, IntoJValue, JValue, JavaSerializable,
};

#[derive(Debug, Clone, PartialEq)]
struct ManualPoint {
    x: i32,
    y: i32,
}

impl JavaSerializable for ManualPoint {
    const CLASS: &'static str = "Fixtures$Point";
    const SUID: i64 = 1;
    const DESCRIPTOR: &'static str = "LFixtures$Point;";

    fn schema() -> &'static [jser::JavaField] {
        &[
            jser::JavaField {
                name: "x",
                typecode: FieldType::Int,
                class_name1: None,
            },
            jser::JavaField {
                name: "y",
                typecode: FieldType::Int,
                class_name1: None,
            },
        ]
    }

    fn to_jvalue(&self, graph: &mut Graph) -> jser::Result<JValue> {
        let desc = graph.add_class_desc(ClassDesc {
            name: Self::CLASS.to_owned(),
            suid: Self::SUID,
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
                ("x".to_owned(), JValue::Int(self.x)),
                ("y".to_owned(), JValue::Int(self.y)),
            ],
            custom: None,
        }])?;
        Ok(JValue::Obj(object))
    }

    fn from_jvalue(value: &JValue, graph: &Graph) -> jser::Result<Self> {
        Ok(Self {
            x: i32::from_jvalue(graph.require_field(value, "x")?, graph)?,
            y: i32::from_jvalue(graph.require_field(value, "y")?, graph)?,
        })
    }
}

impl FromJValue for ManualPoint {
    fn from_jvalue(value: &JValue, graph: &Graph) -> jser::Result<Self> {
        <Self as JavaSerializable>::from_jvalue(value, graph)
    }
}

impl IntoJValue for ManualPoint {
    fn into_jvalue(self, graph: &mut Graph) -> jser::Result<JValue> {
        <Self as JavaSerializable>::to_jvalue(&self, graph)
    }
}

#[test]
fn hand_written_java_serializable_round_trips() {
    let value = ManualPoint { x: 3, y: 4 };
    let bytes = encode_typed(value.clone()).unwrap();
    let decoded: ManualPoint = decode_typed_with_allowlist(&bytes, &fixture_allowlist()).unwrap();
    assert_eq!(decoded, value);
}
