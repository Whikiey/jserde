//! Semantic views for Java's built-in serializable classes.
//!
//! The decoder keeps these values as the generic `JValue::Obj` graph (so byte
//! replay still works); `Registry`/`Graph::builtin` interpret the preserved
//! descriptor and custom data for callers that need typed semantics.

use std::fmt;

use crate::error::{Error, Result};
use crate::value::{FieldType, Graph, JValue};
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
        String::from_utf8_lossy(&digits).into_owned()
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

        if self.scale.unsigned_abs() > 100_000 {
            return format!("{sign}{digits}E{}", -self.scale);
        }

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

/// `java.time.Year`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Year {
    pub year: i32,
}

/// `java.time.YearMonth`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearMonth {
    pub year: i32,
    pub month: u8,
}

/// `java.time.MonthDay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonthDay {
    pub month: u8,
    pub day: u8,
}

/// `java.time.Period`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    pub years: i32,
    pub months: i32,
    pub days: i32,
}

/// `java.time.ZoneId` (region form, e.g. `Asia/Shanghai`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneId {
    pub id: String,
}

/// `java.time.ZoneOffset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoneOffset {
    pub total_seconds: i32,
}

/// `java.time.OffsetTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetTime {
    pub time: LocalTime,
    pub offset_seconds: i32,
}

/// `java.time.OffsetDateTime`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetDateTime {
    pub date_time: LocalDateTime,
    pub offset_seconds: i32,
}

/// `java.util.UUID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uuid {
    pub most_sig_bits: i64,
    pub least_sig_bits: i64,
}

/// `java.util.Currency`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Currency {
    pub currency_code: String,
}

/// `java.util.Locale`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale {
    pub language: String,
    pub country: String,
    pub variant: String,
    pub script: String,
    pub extensions: String,
}

/// `java.util.BitSet` (little-endian 64-bit words).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitSet {
    pub words: Vec<u64>,
}

/// `java.lang.StringBuilder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringBuilderValue {
    pub value: String,
}

/// `java.lang.StringBuffer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringBufferValue {
    pub value: String,
}

/// `java.util.regex.Pattern` (flags + pattern source; compiled automaton is not
/// serialized).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub pattern: String,
    pub flags: i32,
}

/// `java.sql.Date`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlDate {
    pub epoch_millis: i64,
}

/// `java.sql.Time`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlTime {
    pub epoch_millis: i64,
}

/// `java.sql.Timestamp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlTimestamp {
    pub epoch_millis: i64,
    pub nanos: i32,
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
    Year(Year),
    YearMonth(YearMonth),
    MonthDay(MonthDay),
    Period(Period),
    ZoneId(ZoneId),
    ZoneOffset(ZoneOffset),
    OffsetTime(OffsetTime),
    OffsetDateTime(OffsetDateTime),
    Uuid(Uuid),
    Currency(Currency),
    Locale(Locale),
    BitSet(BitSet),
    StringBuilder(StringBuilderValue),
    StringBuffer(StringBufferValue),
    Pattern(Pattern),
    SqlDate(SqlDate),
    SqlTime(SqlTime),
    SqlTimestamp(SqlTimestamp),
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
            "java.util.Date"
                | "java.math.BigInteger"
                | "java.math.BigDecimal"
                | "java.time.Ser"
                | "java.util.UUID"
                | "java.util.Currency"
                | "java.util.Locale"
                | "java.util.BitSet"
                | "java.lang.StringBuilder"
                | "java.lang.StringBuffer"
                | "java.util.regex.Pattern"
                | "java.sql.Date"
                | "java.sql.Time"
                | "java.sql.Timestamp"
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
            "java.util.UUID" => BuiltinValue::Uuid(Uuid {
                most_sig_bits: long_field(graph, value, "mostSigBits")?,
                least_sig_bits: long_field(graph, value, "leastSigBits")?,
            }),
            "java.util.Currency" => BuiltinValue::Currency(Currency {
                currency_code: string_field(graph, value, "currencyCode")?,
            }),
            "java.util.Locale" => BuiltinValue::Locale(Locale {
                language: string_field(graph, value, "language")?,
                country: string_field(graph, value, "country")?,
                variant: string_field(graph, value, "variant")?,
                script: string_field(graph, value, "script")?,
                extensions: string_field(graph, value, "extensions")?,
            }),
            "java.util.BitSet" => BuiltinValue::BitSet(decode_bit_set(graph, value)?),
            "java.lang.StringBuilder" => BuiltinValue::StringBuilder(StringBuilderValue {
                value: decode_string_builder(graph, value)?,
            }),
            "java.lang.StringBuffer" => BuiltinValue::StringBuffer(StringBufferValue {
                value: decode_string_buffer(graph, value)?,
            }),
            "java.util.regex.Pattern" => BuiltinValue::Pattern(Pattern {
                pattern: string_field(graph, value, "pattern")?,
                flags: int_field(graph, value, "flags")?,
            }),
            "java.sql.Date" => BuiltinValue::SqlDate(SqlDate {
                epoch_millis: decode_date(graph, value)?.epoch_millis,
            }),
            "java.sql.Time" => BuiltinValue::SqlTime(SqlTime {
                epoch_millis: decode_date(graph, value)?.epoch_millis,
            }),
            "java.sql.Timestamp" => {
                let epoch_millis = decode_date(graph, value)?.epoch_millis;
                BuiltinValue::SqlTimestamp(SqlTimestamp {
                    epoch_millis,
                    nanos: int_field(graph, value, "nanos")?,
                })
            }
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
    let magnitude = graph.require_field(value, "magnitude")?;
    let array = graph
        .as_primitive_array(magnitude)?
        .ok_or_else(|| Error::FieldMismatch {
            message: "BigInteger.magnitude is not a primitive array".to_owned(),
        })?;
    if array.typecode() != FieldType::Byte {
        return Err(Error::FieldMismatch {
            message: format!(
                "BigInteger.magnitude has {:?} elements, expected byte",
                array.typecode()
            ),
        });
    }

    Ok(BigInteger {
        signum,
        magnitude: array.bytes().to_vec(),
    })
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

fn long_field(graph: &Graph, value: &JValue, name: &str) -> Result<i64> {
    graph.as_i64(graph.require_field(value, name)?)
}

fn string_field(graph: &Graph, value: &JValue, name: &str) -> Result<String> {
    Ok(graph.as_str(graph.require_field(value, name)?)?.to_owned())
}

fn decode_bit_set(graph: &Graph, value: &JValue) -> Result<BitSet> {
    let bits = graph.require_field(value, "bits")?;
    let array = graph
        .as_primitive_array(bits)?
        .ok_or_else(|| Error::FieldMismatch {
            message: "BitSet.bits is not a primitive array".to_owned(),
        })?;
    if array.typecode() != FieldType::Long {
        return Err(Error::FieldMismatch {
            message: format!(
                "BitSet.bits has {:?} elements, expected long",
                array.typecode()
            ),
        });
    }
    let mut words = Vec::with_capacity(array.len());
    for index in 0..array.len() {
        match array.get(index) {
            Some(JValue::Long(word)) => words.push(word as u64),
            other => {
                return Err(Error::FieldMismatch {
                    message: format!("BitSet.bits[{index}] is {other:?}, expected long"),
                })
            }
        }
    }
    Ok(BitSet { words })
}

fn decode_string_builder(graph: &Graph, value: &JValue) -> Result<String> {
    let object = graph
        .as_object(value)?
        .ok_or_else(|| Error::FieldMismatch {
            message: "StringBuilder is not an object".to_owned(),
        })?;
    let layer = object
        .classes
        .iter()
        .find(|class| {
            graph
                .descriptor(class.desc)
                .map(|desc| desc.name == "java.lang.StringBuilder")
                .unwrap_or(false)
        })
        .ok_or_else(|| Error::FieldMismatch {
            message: "missing java.lang.StringBuilder layer".to_owned(),
        })?;
    let custom = layer
        .custom
        .as_deref()
        .ok_or_else(|| Error::FieldMismatch {
            message: "StringBuilder has no custom data".to_owned(),
        })?;

    let mut count = None;
    let mut chars = None;
    for item in custom {
        match item {
            JValue::Block(block) if block.bytes.len() == 4 => {
                let mut reader = StreamReader::new(&block.bytes);
                count = Some(reader.read_i32()?);
            }
            JValue::Arr(_) => chars = Some(item),
            other => {
                return Err(Error::FieldMismatch {
                    message: format!("StringBuilder custom data contains {other:?}"),
                })
            }
        }
    }
    let count = count.ok_or_else(|| Error::FieldMismatch {
        message: "StringBuilder custom data has no length block".to_owned(),
    })?;
    let chars = chars.ok_or_else(|| Error::FieldMismatch {
        message: "StringBuilder custom data has no char array".to_owned(),
    })?;
    char_array_string(
        graph,
        chars,
        Some(usize::try_from(count).map_err(|_| Error::FieldMismatch {
            message: "StringBuilder length is negative".to_owned(),
        })?),
    )
}

fn decode_string_buffer(graph: &Graph, value: &JValue) -> Result<String> {
    let count = int_field(graph, value, "count")?;
    let count = usize::try_from(count).map_err(|_| Error::FieldMismatch {
        message: "StringBuffer.count is negative".to_owned(),
    })?;
    char_array_string(graph, graph.require_field(value, "value")?, Some(count))
}

fn char_array_string(graph: &Graph, value: &JValue, count: Option<usize>) -> Result<String> {
    let array = graph
        .as_primitive_array(value)?
        .ok_or_else(|| Error::FieldMismatch {
            message: "expected a char array".to_owned(),
        })?;
    if array.typecode() != FieldType::Char {
        return Err(Error::FieldMismatch {
            message: format!("expected char[], found {:?}", array.typecode()),
        });
    }
    let length = count.map_or(array.len(), |count| count.min(array.len()));
    if count.is_some_and(|count| count > array.len()) {
        return Err(Error::FieldMismatch {
            message: "char count exceeds the backing array".to_owned(),
        });
    }
    let mut units = Vec::with_capacity(length);
    for index in 0..length {
        match array.get(index) {
            Some(JValue::Char(unit)) => units.push(unit),
            other => {
                return Err(Error::FieldMismatch {
                    message: format!("char[{index}] is {other:?}"),
                })
            }
        }
    }
    String::from_utf16(&units).map_err(|_| Error::FieldMismatch {
        message: "unpaired UTF-16 surrogate in char array".to_owned(),
    })
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
        7 => BuiltinValue::ZoneId(ZoneId {
            id: read_zone_region(&mut reader)?,
        }),
        8 => BuiltinValue::ZoneOffset(ZoneOffset {
            total_seconds: read_zone_offset(&mut reader)?,
        }),
        9 => BuiltinValue::OffsetTime(OffsetTime {
            time: read_local_time(&mut reader)?,
            offset_seconds: read_zone_offset(&mut reader)?,
        }),
        10 => BuiltinValue::OffsetDateTime(OffsetDateTime {
            date_time: read_local_date_time(&mut reader)?,
            offset_seconds: read_zone_offset(&mut reader)?,
        }),
        11 => BuiltinValue::Year(Year {
            year: reader.read_i32()?,
        }),
        12 => BuiltinValue::YearMonth(YearMonth {
            year: reader.read_i32()?,
            month: reader.read_u8()?,
        }),
        13 => BuiltinValue::MonthDay(MonthDay {
            month: reader.read_u8()?,
            day: reader.read_u8()?,
        }),
        14 => BuiltinValue::Period(Period {
            years: reader.read_i32()?,
            months: reader.read_i32()?,
            days: reader.read_i32()?,
        }),
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

fn read_zone_region(reader: &mut StreamReader<'_>) -> Result<String> {
    let length = usize::from(reader.read_u16()?);
    let offset = reader.position();
    mutf8::decode_at(reader.read_exact(length)?, offset)
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
