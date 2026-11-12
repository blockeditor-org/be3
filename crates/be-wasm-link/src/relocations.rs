//! wasm-ld writes a position-independent module's data relocations as one
//! function, `__wasm_apply_data_relocs`: a store of a module-relative address
//! for every pointer in the module's data. In beui's library that is 84,000
//! stores, 1.5 MB of wasm and 2 MB of ARM64 code, and Windows on ARM64 cannot
//! describe a function of more than 1 MiB of code, so Cranelift refuses the
//! module there. [`with_split_relocations`] moves the stores into functions of
//! at most [`CHUNK`] bytes each, appended to the module, and leaves
//! `__wasm_apply_data_relocs` calling them in order.

use be_wasm_dylink::APPLY_DATA_RELOCS;

use crate::module::{
    self, CUSTOM, FUNCTION, Reader, Section, exported_index, imports, name, section, sections,
    unsigned,
};

/// The most wasm a split function holds: about 3,600 stores, under 128 KB of
/// code on any target.
pub const CHUNK: usize = 64 * 1024;

const FUNCTIONS: u8 = 3;
const CODE: u8 = 10;
const CALL: u8 = 0x10;
const END: u8 = 0x0b;

/// The module with `__wasm_apply_data_relocs` split into functions of at most
/// [`CHUNK`] bytes, or unchanged when it has none or it is small. Its body must
/// be what wasm-ld writes, a run of stores whose operands are globals,
/// constants and their sums; anything else is refused rather than split
/// wrongly.
pub fn with_split_relocations(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let sections = sections(bytes)?;
    let Some(index) = exported_index(&sections, APPLY_DATA_RELOCS, FUNCTION)? else {
        return Ok(bytes.to_vec());
    };
    let imported = imports(bytes)?
        .iter()
        .filter(|import| import.kind == FUNCTION)
        .count() as u32;
    let defined = index
        .checked_sub(imported)
        .ok_or_else(|| format!("{APPLY_DATA_RELOCS} is imported, not defined"))? as usize;
    let functions = find(&sections, FUNCTIONS)?;
    let code = find(&sections, CODE)?;

    let mut reader = Reader::new(functions);
    let count = reader.u32()?;
    let mut types = Vec::with_capacity(count as usize);
    for _ in 0..count {
        types.push(reader.u32()?);
    }
    let mut reader = Reader::new(code);
    let mut bodies = Vec::with_capacity(count as usize);
    for _ in 0..reader.u32()? {
        let length = reader.u32()? as usize;
        bodies.push(reader.take(length)?);
    }
    let body = *bodies
        .get(defined)
        .ok_or_else(|| format!("{APPLY_DATA_RELOCS} has no body"))?;
    if body.len() <= CHUNK {
        return Ok(bytes.to_vec());
    }
    let ty = types[defined];
    let chunks = chunks(body)?;

    let first = imported + count;
    let mut caller = vec![0];
    for at in 0..chunks.len() as u32 {
        caller.push(CALL);
        unsigned(u64::from(first + at), &mut caller);
    }
    caller.push(END);

    let mut function_contents = Vec::new();
    unsigned(u64::from(count) + chunks.len() as u64, &mut function_contents);
    for ty in types.iter().copied().chain(chunks.iter().map(|_| ty)) {
        unsigned(u64::from(ty), &mut function_contents);
    }
    let mut code_contents = Vec::new();
    unsigned(u64::from(count) + chunks.len() as u64, &mut code_contents);
    let mut add_body = |body: &[u8]| {
        unsigned(body.len() as u64, &mut code_contents);
        code_contents.extend_from_slice(body);
    };
    for (at, body) in bodies.iter().enumerate() {
        add_body(if at == defined { &caller } else { body });
    }
    for chunk in &chunks {
        add_body(chunk);
    }

    let mut out = bytes[..module::HEADER].to_vec();
    for current in &sections {
        match current.id {
            FUNCTIONS => section(FUNCTIONS, &function_contents, &mut out),
            CODE => section(CODE, &code_contents, &mut out),
            CUSTOM if is_name_section(current)? => {
                section(CUSTOM, &named(current.contents, first, chunks.len())?, &mut out)
            }
            id => section(id, current.contents, &mut out),
        }
    }
    Ok(out)
}

fn find<'a>(sections: &[Section<'a>], id: u8) -> Result<&'a [u8], String> {
    sections
        .iter()
        .find(|section| section.id == id)
        .map(|section| section.contents)
        .ok_or_else(|| format!("a module that exports {APPLY_DATA_RELOCS} has no section {id}"))
}

/// The body's stores in bodies of at most [`CHUNK`] bytes, each a whole store,
/// split where nothing is left on the stack.
fn chunks(body: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut reader = Reader::new(body);
    if reader.u32()? != 0 {
        return Err(format!("{APPLY_DATA_RELOCS} has locals"));
    }
    let mut chunks = Vec::new();
    let mut chunk = vec![0];
    let mut statement = reader.at;
    let mut depth = 0u32;
    loop {
        let opcode = reader.byte()?;
        match opcode {
            // global.get
            0x23 => {
                reader.u32()?;
                depth += 1;
            }
            // i32.const, i64.const
            0x41 | 0x42 => {
                reader.signed()?;
                depth += 1;
            }
            // i32.add, i32.sub, i64.add, i64.sub
            0x6a | 0x6b | 0x7c | 0x7d => depth = pop(depth, 2)? + 1,
            // i32.store, i64.store
            0x36 | 0x37 => {
                reader.u32()?;
                reader.u32()?;
                depth = pop(depth, 2)?;
            }
            END if depth == 0 && reader.done() => break,
            other => {
                return Err(format!(
                    "{APPLY_DATA_RELOCS} has the opcode {other:#x}, which it is not split across"
                ));
            }
        }
        if depth == 0 {
            let stored = &body[statement..reader.at];
            if chunk.len() + stored.len() + 1 > CHUNK {
                chunk.push(END);
                chunks.push(std::mem::replace(&mut chunk, vec![0]));
            }
            chunk.extend_from_slice(stored);
            statement = reader.at;
        }
    }
    chunk.push(END);
    chunks.push(chunk);
    Ok(chunks)
}

fn pop(depth: u32, operands: u32) -> Result<u32, String> {
    depth
        .checked_sub(operands)
        .ok_or_else(|| format!("{APPLY_DATA_RELOCS} pops a value it never pushed"))
}

fn is_name_section(current: &Section<'_>) -> Result<bool, String> {
    Ok(Reader::new(current.contents).name()? == "name")
}

/// The name section with `__wasm_apply_data_relocs.N` added for each split
/// function, which come after every other, so their names go last.
fn named(contents: &[u8], first: u32, added: usize) -> Result<Vec<u8>, String> {
    let mut reader = Reader::new(contents);
    let mut out = Vec::with_capacity(contents.len() + added * 32);
    name(&reader.name()?, &mut out);
    while !reader.done() {
        let kind = reader.byte()?;
        let length = reader.u32()? as usize;
        let subsection = reader.take(length)?;
        let mut rewritten = subsection.to_vec();
        if kind == 1 {
            let (count, existing) = {
                let mut names = Reader::new(subsection);
                (names.u32()?, &subsection[names.at..])
            };
            rewritten.clear();
            unsigned(u64::from(count) + added as u64, &mut rewritten);
            rewritten.extend_from_slice(existing);
            for at in 0..added {
                unsigned(u64::from(first) + at as u64, &mut rewritten);
                name(&format!("{APPLY_DATA_RELOCS}.{at}"), &mut rewritten);
            }
        }
        out.push(kind);
        unsigned(rewritten.len() as u64, &mut out);
        out.extend_from_slice(&rewritten);
    }
    Ok(out)
}
