use crate::{Error, Result};

use super::Mapping;

pub(crate) fn base_offset(mappings: &str, line: u32) -> u32 {
    let mut offset = 0u32;
    for (current_line, part) in mappings.split(';').enumerate() {
        if u32::try_from(current_line).is_ok_and(|l| l == line) {
            return offset;
        }
        for seg in part.split(',') {
            if let Some((first, _)) = seg.split_once(':') {
                let v = decode_vlq(first);
                offset = offset.saturating_add(narrow(v));
            } else {
                offset = offset.saturating_add(narrow(decode_vlq(seg)));
            }
        }
        offset = offset.saturating_add(1);
    }
    offset
}

pub(crate) fn narrow(v: i64) -> u32 {
    u32::try_from(v.max(0)).unwrap_or(u32::MAX)
}

pub fn decode(mappings: &str, sources_len: usize) -> Result<Vec<Mapping>> {
    let mut out: Vec<Mapping> = Vec::with_capacity(mappings.len() / 8 + 8);
    let mut generated_offset = 0u32;
    let mut source_index = 0i64;
    let mut original_line = 0i64;
    let mut original_column = 0i64;
    let mut name_index = 0i64;

    let mut values = [0i64; 5];
    for line in mappings.split(';') {
        for segment in line.split(',') {
            if segment.is_empty() {
                continue;
            }
            let count = decode_segment(segment, &mut values);
            if count == 0 {
                continue;
            }

            generated_offset = generated_offset.saturating_add(narrow(values[0]));
            if count < 4 {
                continue;
            }

            source_index += values[1];
            original_line += values[2];
            original_column += values[3];
            let name = if count >= 5 {
                name_index += values[4];
                Some(name_index)
            } else {
                None
            };

            if source_index < 0 || original_line < 0 || original_column < 0 {
                return Err(Error::Malformed(
                    "source map has negative deltas (corrupt mappings)".into(),
                ));
            }

            if source_index >= i64::try_from(sources_len).unwrap_or(i64::MAX) {
                return Err(Error::Malformed(format!(
                    "source map mapping references source {source_index} but only {sources_len} are declared"
                )));
            }

            out.push(Mapping {
                generated_offset,

                source_index: narrow(source_index),
                original_line: narrow(original_line),
                original_column: narrow(original_column),
                name_index: name.map(narrow),
            });
        }
        generated_offset = generated_offset.saturating_add(1);
    }

    Ok(out)
}

fn decode_segment(segment: &str, out: &mut [i64; 5]) -> usize {
    let mut shift = 0u32;
    let mut result = 0i64;
    let mut written = 0usize;

    for byte in segment.bytes() {
        let Some(digit) = base64_digit(byte) else { break };
        result |= i64::from(digit & 31) << shift;
        if digit & 32 == 0 {
            let negative = result & 1 == 1;
            let value = result >> 1;
            let value = if negative { -value } else { value };
            if written < out.len() {
                out[written] = value;
            }
            written += 1;
            result = 0;
            shift = 0;
        } else {
            shift += 5;
        }
    }
    written
}

fn base64_digit(byte: u8) -> Option<u8> {
    const TABLE: [u8; 128] = {
        let mut t = [255u8; 128];
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0u8;
        while (i as usize) < alphabet.len() {
            t[alphabet[i as usize] as usize] = i;
            i += 1;
        }
        t
    };
    if byte < 128 { Some(TABLE[byte as usize]) } else { None }
}

pub fn decode_vlq(input: &str) -> i64 {
    let mut values = [0i64; 5];
    if decode_segment(input, &mut values) == 0 { 0 } else { values[0] }
}
