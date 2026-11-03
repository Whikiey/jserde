//! P5-3: default SUID algorithm against the JDK-computed Fixtures$NoSuid value.

use jser::suid::{compute_default_suid, ClassSignature, FieldSignature, MemberSignature};

#[test]
fn computes_the_jdk_default_suid_for_nosuid() {
    let signature = ClassSignature {
        class_name: "Fixtures$NoSuid".to_owned(),
        modifiers: 0,
        interfaces: vec!["java.io.Serializable".to_owned()],
        fields: vec![
            FieldSignature {
                name: "x".to_owned(),
                modifiers: 0,
                descriptor: "I".to_owned(),
            },
            FieldSignature {
                name: "y".to_owned(),
                modifiers: 0,
                descriptor: "Ljava/lang/String;".to_owned(),
            },
        ],
        has_static_initializer: false,
        constructors: vec![MemberSignature {
            name: "<init>".to_owned(),
            modifiers: 0,
            descriptor: "()V".to_owned(),
        }],
        methods: Vec::new(),
    };

    assert_eq!(
        compute_default_suid(&signature).unwrap(),
        -3_559_046_683_250_708_543
    );
}
