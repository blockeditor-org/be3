//! Reading values without their schema: every value's wire type says what it is, so a message,
//! list or bytes a newer build wrote can still be walked.

use std::fmt::Write as _;

use crate::Error;
use crate::wire::{ItemsReader, Key, Reader, Reference, Value, Wire};

/// Every block reference in `message` (an encoded message, as `encode` wrote it), in the order
/// they are written, wherever they are: in fields and lists, in fields no type knows.
pub fn references(message: &[u8]) -> Result<Vec<Reference>, Error> {
    let mut found = Vec::new();
    walk(Value::outermost(Wire::Message, message)?, &mut |value| {
        if let Value::Reference(reference) = value {
            found.push(reference);
        }
    })?;
    Ok(found)
}

fn walk<'de>(value: Value<'de>, visit: &mut impl FnMut(Value<'de>)) -> Result<(), Error> {
    visit(value);
    match value {
        Value::Message(_) => {
            let mut reader = Reader::of(value)?;
            while !reader.is_empty() {
                walk(reader.field()?.1, visit)?;
            }
        }
        Value::List(items) => {
            let mut items = ItemsReader::any(items)?;
            while let Some(item) = items.next() {
                walk(item?, visit)?;
            }
            items.finish()?;
        }
        _ => {}
    }
    Ok(())
}

/// `message` (an encoded message) as text, one field a line: its id, its flags, and its value,
/// for looking at stored or sent bytes with no schema at hand.
pub fn dump(message: &[u8]) -> Result<String, Error> {
    let mut out = String::new();
    dump_fields(Value::outermost(Wire::Message, message)?, 0, &mut out)?;
    Ok(out)
}

fn dump_fields(message: Value<'_>, indent: usize, out: &mut String) -> Result<(), Error> {
    let mut reader = Reader::of(message)?;
    while !reader.is_empty() {
        let (key, value) = reader.field()?;
        let _ = write!(out, "{:indent$}{}", "", key_text(key));
        dump_value(value, indent, out)?;
    }
    Ok(())
}

fn key_text(key: Key) -> String {
    let mut text = key.id.to_string();
    if key.critical {
        text.push_str(" critical");
    }
    if key.no_copy {
        text.push_str(" no_copy");
    }
    text
}

fn dump_value(value: Value<'_>, indent: usize, out: &mut String) -> Result<(), Error> {
    match value {
        Value::Empty => out.push_str(": empty\n"),
        Value::Fixed8(value) => {
            let _ = writeln!(out, ": {value}");
        }
        Value::Fixed16(value) => {
            let _ = writeln!(out, ": {value}");
        }
        Value::Fixed32(value) => {
            let _ = writeln!(out, ": {value} ({})", f32::from_bits(value));
        }
        Value::Fixed64(value) => {
            let _ = writeln!(out, ": {value} ({})", f64::from_bits(value));
        }
        Value::Fixed128(value) => {
            let _ = writeln!(out, ": {value:#034x}");
        }
        Value::Reference(reference) => {
            let _ = writeln!(out, ": reference {}", hex(&reference.0));
        }
        Value::Bytes(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => {
                let _ = writeln!(out, ": {text:?}");
            }
            Err(_) => {
                let _ = writeln!(
                    out,
                    ": {} bytes {}",
                    bytes.len(),
                    hex(&bytes[..bytes.len().min(32)])
                );
            }
        },
        Value::Message(_) => {
            out.push_str(": {\n");
            dump_fields(value, indent + 2, out)?;
            let _ = writeln!(out, "{:indent$}}}", "");
        }
        Value::List(items) => {
            let _ = writeln!(out, ": [{} {:?}", items.count, items.wire);
            let mut items = ItemsReader::any(items)?;
            while let Some(item) = items.next() {
                let _ = write!(out, "{:width$}-", "", width = indent + 2);
                dump_value(item?, indent + 2, out)?;
            }
            items.finish()?;
            let _ = writeln!(out, "{:indent$}]", "");
        }
        Value::Packed(packed) => {
            let _ = writeln!(
                out,
                ": {} {:?} {}",
                packed.bytes.len() / packed.number.size(),
                packed.number,
                hex(&packed.bytes[..packed.bytes.len().min(32)])
            );
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
