//! P5-3: default SUID algorithm against the JDK-computed Fixtures$NoSuid value.

use jser::suid::{
    array_class_suid, compute_default_suid, ClassSignature, FieldSignature, MemberSignature,
};

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

#[test]
fn computes_the_jdk_default_suid_for_array_classes() {
    // Values taken from the committed fixtures / `new_java_array` table.
    for (class_name, expected) in [
        ("[Z", 6_309_297_032_502_205_922),
        ("[B", -5_984_413_125_824_719_648),
        ("[C", -5_753_798_564_021_173_076),
        ("[S", -1_188_055_269_542_874_886),
        ("[I", 5_600_894_804_908_749_477),
        ("[J", 8_655_923_659_555_304_851),
        ("[F", 836_686_056_779_680_834),
        ("[D", 4_514_449_696_888_150_558),
        ("[Ljava.lang.String;", -5_921_575_005_990_323_385),
        ("[Ljava.lang.Object;", -8_012_369_246_846_506_644),
    ] {
        assert_eq!(
            array_class_suid(class_name).unwrap(),
            expected,
            "SUID mismatch for `{class_name}`"
        );
    }
}
