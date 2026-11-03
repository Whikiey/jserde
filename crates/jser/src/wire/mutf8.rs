//! Java "modified UTF-8" (CESU-8 style) encoding and decoding.
//!
//! Differences from standard UTF-8:
//! - `U+0000` is encoded as `C0 80`;
//! - supplementary characters are encoded as two 3-byte surrogate units;
//! - 4-byte sequences and unpaired surrogates are rejected.

use crate::error::{Error, Result};

fn malformed(offset: usize) -> Error {
    Error::MalformedString { offset }
}

fn continuation(bytes: &[u8], index: usize, sequence_start: usize) -> Result<u8> {
    match bytes.get(index).copied() {
        Some(byte) if byte & 0xC0 == 0x80 => Ok(byte),
        _ => Err(malformed(sequence_start)),
    }
}

/// Decode a complete modified UTF-8 byte sequence.
pub fn decode(bytes: &[u8]) -> Result<String> {
    decode_at(bytes, 0)
}

/// Decode modified UTF-8, reporting offsets relative to `base`.
pub fn decode_at(bytes: &[u8], base: usize) -> Result<String> {
    let mut output = String::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte < 0x80 {
            if byte == 0 {
                return Err(malformed(base + index));
            }
            output.push(byte as char);
            index += 1;
            continue;
        }

        if byte & 0xE0 == 0xC0 {
            let second = continuation(bytes, index + 1, base + index)?;
            let code = ((u32::from(byte) & 0x1F) << 6) | (u32::from(second) & 0x3F);

            if code == 0 {
                if byte != 0xC0 || second != 0x80 {
                    return Err(malformed(base + index));
                }
            } else if code < 0x80 {
                return Err(malformed(base + index));
            }

            output.push(char::from_u32(code).ok_or_else(|| malformed(base + index))?);
            index += 2;
            continue;
        }

        if byte & 0xF0 == 0xE0 {
            let second = continuation(bytes, index + 1, base + index)?;
            let third = continuation(bytes, index + 2, base + index)?;
            let code = ((u32::from(byte) & 0x0F) << 12)
                | ((u32::from(second) & 0x3F) << 6)
                | (u32::from(third) & 0x3F);

            if code < 0x800 {
                return Err(malformed(base + index));
            }

            if (0xD800..=0xDFFF).contains(&code) {
                if !(0xD800..=0xDBFF).contains(&code) {
                    return Err(malformed(base + index));
                }

                let low_start = index + 3;
                if low_start + 3 > bytes.len() {
                    return Err(malformed(base + index));
                }
                if bytes[low_start] & 0xF0 != 0xE0 {
                    return Err(malformed(base + index));
                }

                let low_second = continuation(bytes, low_start + 1, base + low_start)?;
                let low_third = continuation(bytes, low_start + 2, base + low_start)?;
                let low = ((u32::from(bytes[low_start]) & 0x0F) << 12)
                    | ((u32::from(low_second) & 0x3F) << 6)
                    | (u32::from(low_third) & 0x3F);

                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(malformed(base + index));
                }

                let scalar = 0x1_0000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                output.push(char::from_u32(scalar).ok_or_else(|| malformed(base + index))?);
                index = low_start + 3;
                continue;
            }

            output.push(char::from_u32(code).ok_or_else(|| malformed(base + index))?);
            index += 3;
            continue;
        }

        // 4-byte UTF-8 (F0..F4) and stray leads are not valid modified UTF-8.
        return Err(malformed(base + index));
    }

    Ok(output)
}

fn push_three_bytes(output: &mut Vec<u8>, code: u32) {
    output.push(0xE0 | ((code >> 12) as u8 & 0x0F));
    output.push(0x80 | ((code >> 6) as u8 & 0x3F));
    output.push(0x80 | (code as u8 & 0x3F));
}

/// Encode a Rust string as Java modified UTF-8.
pub fn encode(text: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(encoded_len(text));

    for character in text.chars() {
        let code = character as u32;

        if code == 0 {
            output.extend_from_slice(&[0xC0, 0x80]);
        } else if code < 0x80 {
            output.push(code as u8);
        } else if code < 0x800 {
            output.push(0xC0 | ((code >> 6) as u8));
            output.push(0x80 | (code as u8 & 0x3F));
        } else if code < 0x1_0000 {
            push_three_bytes(&mut output, code);
        } else {
            let supplementary = code - 0x1_0000;
            let high = 0xD800 + (supplementary >> 10);
            let low = 0xDC00 + (supplementary & 0x3FF);
            push_three_bytes(&mut output, high);
            push_three_bytes(&mut output, low);
        }
    }

    output
}

/// Number of modified-UTF-8 bytes `encode` would produce.
pub fn encoded_len(text: &str) -> usize {
    text.chars()
        .map(|character| {
            let code = character as u32;
            if code == 0 {
                2
            } else if code < 0x80 {
                1
            } else if code < 0x800 {
                2
            } else if code < 0x1_0000 {
                3
            } else {
                6
            }
        })
        .sum()
}
