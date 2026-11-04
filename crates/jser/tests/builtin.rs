//! P6-1/P6-2: semantic views for Date/BigInteger/BigDecimal and java.time.Ser.

mod common;

use common::{decode_fixture, read_fixture};
use jser::builtin::{
    BigDecimal, BigInteger, BuiltinValue, Duration, Instant, JavaDate, LocalDate, LocalDateTime,
    LocalTime, MonthDay, OffsetDateTime, OffsetTime, Period, Registry, Year, YearMonth, ZoneId,
    ZoneOffset, ZonedDateTime,
};

fn array_item(graph: &jser::Graph, index: usize) -> jser::JValue {
    let array = graph.as_array(&graph.items()[0]).unwrap().unwrap();
    array[index].clone()
}

#[test]
fn decodes_misc_java_types_semantically() {
    let graph = decode_fixture(&read_fixture("object/36")).unwrap();

    assert_eq!(
        graph.builtin(&array_item(&graph, 0)).unwrap(),
        Some(BuiltinValue::Date(JavaDate { epoch_millis: 0 }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 1)).unwrap(),
        Some(BuiltinValue::BigInteger(BigInteger {
            signum: 1,
            magnitude: vec![
                0x01, 0x8e, 0xe9, 0x0f, 0xf6, 0xc3, 0x73, 0xe0, 0xee, 0x4e, 0x3f, 0x0a, 0xd2,
            ],
        }))
    );
    let big = graph.builtin(&array_item(&graph, 1)).unwrap().unwrap();
    let BuiltinValue::BigInteger(big) = big else {
        panic!("expected BigInteger");
    };
    assert_eq!(big.to_decimal_string(), "123456789012345678901234567890");

    let decimal = graph.builtin(&array_item(&graph, 2)).unwrap().unwrap();
    let BuiltinValue::BigDecimal(decimal) = decimal else {
        panic!("expected BigDecimal");
    };
    assert_eq!(decimal.scale, 2);
    assert_eq!(decimal.to_decimal_string(), "1.50");
    assert_eq!(
        decimal,
        BigDecimal {
            scale: 2,
            unscaled: BigInteger {
                signum: 1,
                magnitude: vec![150],
            },
        }
    );

    assert_eq!(
        graph.builtin(&array_item(&graph, 3)).unwrap(),
        Some(BuiltinValue::LocalDate(LocalDate {
            year: 2024,
            month: 2,
            day: 29,
        }))
    );
}

#[test]
fn decodes_java_time_values() {
    let graph = decode_fixture(&read_fixture("object/40")).unwrap();

    assert_eq!(
        graph.builtin(&array_item(&graph, 0)).unwrap(),
        Some(BuiltinValue::Duration(Duration {
            seconds: 123,
            nanos: 456_789_000,
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 1)).unwrap(),
        Some(BuiltinValue::Instant(Instant {
            epoch_second: 1_700_000_000,
            nano: 123_456_789,
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 2)).unwrap(),
        Some(BuiltinValue::LocalDate(LocalDate {
            year: 2024,
            month: 2,
            day: 29,
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 3)).unwrap(),
        Some(BuiltinValue::LocalDateTime(LocalDateTime {
            date: LocalDate {
                year: 2024,
                month: 2,
                day: 29,
            },
            time: LocalTime {
                hour: 12,
                minute: 34,
                second: 56,
                nano: 123_456_789,
            },
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 4)).unwrap(),
        Some(BuiltinValue::ZonedDateTime(ZonedDateTime {
            date_time: LocalDateTime {
                date: LocalDate {
                    year: 2024,
                    month: 2,
                    day: 29,
                },
                time: LocalTime {
                    hour: 12,
                    minute: 34,
                    second: 56,
                    nano: 123_456_789,
                },
            },
            offset_seconds: 8 * 3600,
            zone: "Asia/Shanghai".to_owned(),
        }))
    );
}

#[test]
fn decodes_year_month_day_and_period() {
    let graph = decode_fixture(&read_fixture("object/42")).unwrap();

    assert_eq!(
        graph.builtin(&array_item(&graph, 0)).unwrap(),
        Some(BuiltinValue::Year(Year { year: 2024 }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 1)).unwrap(),
        Some(BuiltinValue::YearMonth(YearMonth {
            year: 2024,
            month: 2,
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 2)).unwrap(),
        Some(BuiltinValue::MonthDay(MonthDay { month: 2, day: 29 }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 3)).unwrap(),
        Some(BuiltinValue::Period(Period {
            years: 1,
            months: 2,
            days: 3,
        }))
    );
}

#[test]
fn decodes_zone_and_offset_time_values() {
    let graph = decode_fixture(&read_fixture("object/43")).unwrap();

    assert_eq!(
        graph.builtin(&array_item(&graph, 0)).unwrap(),
        Some(BuiltinValue::ZoneId(ZoneId {
            id: "Asia/Shanghai".to_owned(),
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 1)).unwrap(),
        Some(BuiltinValue::ZoneOffset(ZoneOffset {
            total_seconds: 8 * 3600,
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 2)).unwrap(),
        Some(BuiltinValue::OffsetTime(OffsetTime {
            time: LocalTime {
                hour: 12,
                minute: 34,
                second: 56,
                nano: 123_456_789,
            },
            offset_seconds: 8 * 3600,
        }))
    );
    assert_eq!(
        graph.builtin(&array_item(&graph, 3)).unwrap(),
        Some(BuiltinValue::OffsetDateTime(OffsetDateTime {
            date_time: LocalDateTime {
                date: LocalDate {
                    year: 2024,
                    month: 2,
                    day: 29,
                },
                time: LocalTime {
                    hour: 12,
                    minute: 34,
                    second: 56,
                    nano: 123_456_789,
                },
            },
            offset_seconds: 8 * 3600,
        }))
    );
}

#[test]
fn registry_does_not_claim_unknown_classes() {
    let registry = Registry::new();
    assert!(registry.is_known("java.util.Date"));
    assert!(registry.is_known("java.time.Ser"));
    assert!(!registry.is_known("Fixtures$Point"));

    let graph = decode_fixture(&read_fixture("object/20")).unwrap();
    assert!(graph.builtin(&graph.items()[0]).unwrap().is_none());
}
