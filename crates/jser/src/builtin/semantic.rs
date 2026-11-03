//! Semantic views for Java's built-in serializable classes.
//!
//! The decoder keeps these values as the generic `JValue::Obj` graph (so byte
//! replay still works); `Registry`/`Graph::builtin` interpret the preserved
//! descriptor and custom data for callers that need typed semantics.

use std::fmt;

use crate::error::{Error, Result};
use crate::value::{Graph, JValue};
use crate::wire::mutf8;
use crate::wire::reader::StreamReader;

/// `java.util.Date`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaDate {
    pub epoch_millis: i64,
}

/// Sign-magnitude `java.math.BigInteger`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigInteger {
    pub signum: i32,
    /// Unsigned big-endian magnitude, as written by Java.
    pub magnitude: Vec<u8>,
}

impl BigInteger {
    /// Decimal representation.
    pub fn to_decimal_string(&self) -> String {
        if self.signum == 0 || self.magnitude.iter().all(|byte| *byte == 0) {
            return "0".to_owned();
        }

        let mut bytes = self.magnitude.clone();
        let mut digits = Vec::new();
        while bytes.iter().any(|byte| *byte != 0) {
            let mut remainder = 0u32;
            for byte in &mut bytes {
                let current = (remainder << 8) | u32::from(*byte);
                *byte = (current / 10) as u8;
                remainder = current % 10;
            }
            digits.push(b'0' + remainder as u8);
        }
        if self.signum < 0 {
            digits.push(b'-');
        }
        digits.reverse();
        String::from_utf8(digits).expect("decimal digits are ASCII")
    }
}

impl fmt::Display for BigInteger {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_decimal_string())
    }
}

/// `java.math.BigDecimal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigDecimal {
    pub scale: i32,
    pub unscaled: BigInteger,
}

impl BigDecimal {
    /// Scale-preserving decimal representation (`1.50`, not `1.5`).
    pub fn to_decimal_string(&self) -> String {
        let sign = if self.unscaled.signum < 0 { "-" } else { "" };
        let digits = self.unscaled.to_decimal_string();
        let digits = digits.trim_start_matches('-');

        if self.scale <= 0 {
            let zeros = "0".repeat(self.scale.unsigned_abs() as usize);
            return format!("{sign}{digits}{zeros}");
        }

        let scale = self.scale as usize;
        if digits.len() <= scale {
            format!("{sign}0.{}{}", "0".repeat(scale - digits.len()), digits)
        } else {
            let split = digits.len() - scale;
            format!("{sign}{}.{}", &digits[..split], &digits[split..])
        }
    }
}

impl fmt::Display for BigDecimal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_decimal_string())
    }
}

/// `java.time.LocalDate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

/// `java.time.LocalTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nano: i32,
}

/// `java.time.LocalDateTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub date: LocalDate,
    pub time: LocalTime,
}

/// `java.time.Instant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instant {
    pub epoch_second: i64,
    pub nano: i32,
}

/// `java.time.Duration`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    pub seconds: i64,
    pub nanos: i32,
}

/// `java.time.ZonedDateTime`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZonedDateTime {
    pub date_time: LocalDateTime,
    pub offset_seconds: i32,
    pub zone: String,
}

/// A semantic value decoded from a built-in Java class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuiltinValue {
    Date(JavaDate),
    BigInteger(BigInteger),
    BigDecimal(BigDecimal),
    LocalDate(LocalDate),
    LocalTime(LocalTime),
    LocalDateTime(LocalDateTime),
    Instant(Instant),
    Duration(Duration),
    ZonedDateTime(ZonedDateTime),
}

/// Maps Java class names to semantic codecs; unknown classes are left to the
/// generic `JValue::Obj` path.
#[derive(Debug, Clone, Copy, Default)]
pub struct Registry;

impl Registry {
    pub fn new() -> Self {
        Self
    }

    /// Whether this registry has a semantic codec for `class`.
    pub fn is_known(&self, class: &str) -> bool {
        matches!(
            class,
            "java.util.Date" | "java.math.BigInteger" | "java.math.BigDecimal" | "java.time.Ser"
        )
    }

    /// Decode `value` if it is a known built-in class; return `None` otherwise.
    pub fn decode(&self, graph: &Graph, value: &JValue) -> Result<Option<BuiltinValue>> {
        let class = match graph.class_name(value) {
            Ok(class) => class,
            Err(_) => return Ok(None),
        };

        let decoded = match class {
            "java.util.Date" => BuiltinValue::Date(decode_date(graph, value)?),
            "java.math.BigInteger" => BuiltinValue::BigInteger(decode_big_integer(graph, value)?),
            "java.math.BigDecimal" => BuiltinValue::BigDecimal(decode_big_decimal(graph, value)?),
            "java.time.Ser" => decode_java_time(graph, value)?,
            _ => return Ok(None),
        };
        Ok(Some(decoded))
    }
}

fn decode_date(graph: &Graph, value: &JValue) -> Result<JavaDate> {
    let bytes = custom_bytes(graph, value, "java.util.Date")?;
    let mut reader = StreamReader::new(&bytes);
    let epoch_millis = reader.read_i64()?;
    Ok(JavaDate { epoch_millis })
}

fn decode_big_integer(graph: &Graph, value: &JValue) -> Result<BigInteger> {
    let signum = int_field(graph, value, "signum")?;
    let magnitude = graph
        .require_field(value, "magnitude")
        .and_then(|value| {
            graph.as_array(value)?.ok_or_else(|| Error::FieldMismatch {
                message: "BigInteger.magnitude is not an array".to_owned(),
            })
        })?
        .iter()
        .map(|value| match value {
            JValue::Byte(byte) => Ok(*byte as u8),
            other => Err(Error::FieldMismatch {
                message: format!("BigInteger.magnitude contains {other:?}"),
            }),
        })
        .collect::<Result<Vec<u8>>>()?;

    Ok(BigInteger { signum, magnitude })
}

fn decode_big_decimal(graph: &Graph, value: &JValue) -> Result<BigDecimal> {
    let scale = int_field(graph, value, "scale")?;
    let int_val = graph.require_field(value, "intVal")?;
    let unscaled = decode_big_integer(graph, int_val)?;
    Ok(BigDecimal { scale, unscaled })
}

fn int_field(graph: &Graph, value: &JValue, name: &str) -> Result<i32> {
    graph.as_i32(graph.require_field(value, name)?)
}

fn custom_bytes(graph: &Graph, value: &JValue, class: &str) -> Result<Vec<u8>> {
    let object = graph
        .as_object(value)?
        .ok_or_else(|| Error::FieldMismatch {
            message: format!("`{class}` is not an object"),
        })?;

    let mut bytes = Vec::new();
    let mut found = false;
    for layer in &object.classes {
        if let Some(items) = &layer.custom {
            found = true;
            for item in items {
                match item {
                    JValue::Block(block) => bytes.extend_from_slice(&block.bytes),
                    other => {
                        return Err(Error::FieldMismatch {
                            message: format!("`{class}` custom data contains {other:?}"),
                        })
                    }
                }
            }
        }
    }
    if !found {
        return Err(Error::FieldMismatch {
            message: format!("`{class}` has no custom data"),
        });
    }
    Ok(bytes)
}

fn decode_java_time(graph: &Graph, value: &JValue) -> Result<BuiltinValue> {
    let bytes = custom_bytes(graph, value, "java.time.Ser")?;
    let mut reader = StreamReader::new(&bytes);

    let value = match reader.read_u8()? {
        1 => BuiltinValue::Duration(Duration {
            seconds: reader.read_i64()?,
            nanos: reader.read_i32()?,
        }),
        2 => BuiltinValue::Instant(Instant {
            epoch_second: reader.read_i64()?,
            nano: reader.read_i32()?,
        }),
        3 => BuiltinValue::LocalDate(read_local_date(&mut reader)?),
        4 => BuiltinValue::LocalTime(read_local_time(&mut reader)?),
        5 => BuiltinValue::LocalDateTime(read_local_date_time(&mut reader)?),
        6 => BuiltinValue::ZonedDateTime(read_zoned_date_time(&mut reader)?),
        other => {
            return Err(Error::UnsupportedClass(format!(
                "java.time.Ser type {other}"
            )))
        }
    };
    Ok(value)
}

fn read_local_date(reader: &mut StreamReader<'_>) -> Result<LocalDate> {
    Ok(LocalDate {
        year: reader.read_i32()?,
        month: reader.read_u8()?,
        day: reader.read_u8()?,
    })
}

fn read_local_time(reader: &mut StreamReader<'_>) -> Result<LocalTime> {
    Ok(LocalTime {
        hour: reader.read_u8()?,
        minute: reader.read_u8()?,
        second: reader.read_u8()?,
        nano: reader.read_i32()?,
    })
}

fn read_local_date_time(reader: &mut StreamReader<'_>) -> Result<LocalDateTime> {
    Ok(LocalDateTime {
        date: read_local_date(reader)?,
        time: read_local_time(reader)?,
    })
}

fn read_zoned_date_time(reader: &mut StreamReader<'_>) -> Result<ZonedDateTime> {
    let date_time = read_local_date_time(reader)?;
    let offset_seconds = read_zone_offset(reader)?;
    let zone = read_zone_id(reader, offset_seconds)?;
    Ok(ZonedDateTime {
        date_time,
        offset_seconds,
        zone,
    })
}

fn read_zone_offset(reader: &mut StreamReader<'_>) -> Result<i32> {
    let encoded = reader.read_u8()?;
    if encoded == 127 {
        reader.read_i32()
    } else {
        Ok(i32::from(encoded) * 900)
    }
}

fn read_zone_id(reader: &mut StreamReader<'_>, offset_seconds: i32) -> Result<String> {
    match reader.read_u8()? {
        7 => {
            let length = usize::from(reader.read_u16()?);
            let offset = reader.position();
            mutf8::decode_at(reader.read_exact(length)?, offset)
        }
        8 => {
            let seconds = read_zone_offset(reader)?;
            let _ = offset_seconds;
            Ok(format_offset(seconds))
        }
        other => Err(Error::UnsupportedClass(format!(
            "java.time zone type {other}"
        ))),
    }
}

fn format_offset(seconds: i32) -> String {
    if seconds == 0 {
        return "Z".to_owned();
    }
    let sign = if seconds < 0 { '-' } else { '+' };
    let seconds = seconds.unsigned_abs();
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    if minutes == 0 {
        format!("{sign}{hours:02}:00")
    } else {
        format!("{sign}{hours:02}:{minutes:02}")
    }
}
