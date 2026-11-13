use super::*;

// A module like wasm-ld's, exporting a __wasm_apply_data_relocs of 20,000
// stores (about 360 KB), comes back with that function calling functions of at
// most relocations::CHUNK bytes that hold the same stores in the same order:
// with 32-bit addresses, and with 64-bit ones (i64.const, i64.add, i64.store).
#[test]
fn a_long_relocation_function_is_split_into_calls() {
    split_and_check(false);
    split_and_check(true);
}

fn split_and_check(wide: bool) {
    // (the globals' type, const, add, store)
    let (global, constant, add, store) = if wide {
        (0x7e, 0x42, 0x7c, 0x37)
    } else {
        (0x7f, 0x41, 0x6a, 0x36)
    };
    let mut stores = Vec::new();
    for at in 0..20_000u64 {
        stores.extend([0x23, 0]);
        stores.push(constant);
        stores.extend(signed(at as i64 * 4));
        stores.extend([add, 0x23, 1, constant]);
        stores.extend(signed(at as i64));
        stores.extend([add, store, 2, 0]);
    }
    let mut body = vec![0];
    body.extend(&stores);
    body.push(0x0b);
    let mut code = vec![2];
    code.extend(leb(body.len() as u64));
    code.extend(&body);
    code.extend([2, 0, 0x0b]);

    let mut imports = vec![2];
    for name in ["__memory_base", "__table_base"] {
        imports.extend(named("env"));
        imports.extend(named(name));
        imports.extend([3, global, 0]);
    }
    let mut exports = vec![1];
    exports.extend(named("__wasm_apply_data_relocs"));
    exports.extend([0, 0]);
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    bytes.extend(section(1, &[1, 0x60, 0, 0]));
    bytes.extend(section(2, &imports));
    bytes.extend(section(3, &[2, 0, 0]));
    bytes.extend(section(5, &[1, 0, 1]));
    bytes.extend(section(7, &exports));
    bytes.extend(section(10, &code));
    let mut names = vec![2];
    names.extend(named("__wasm_apply_data_relocs"));
    names.push(1);
    names.extend(named("other"));
    let mut subsection = vec![1];
    subsection.extend(leb(names.len() as u64));
    subsection.extend(names);
    bytes.extend(custom("name", &subsection));

    let split = relocations::with_split_relocations(&bytes).unwrap();
    let bodies = bodies(&split);
    let added = &bodies[2..];
    assert!(added.len() > 1);
    let mut caller = vec![0];
    for index in 2..bodies.len() {
        caller.extend([0x10, index as u8]);
    }
    caller.push(0x0b);
    assert_eq!(bodies[0], caller);
    assert_eq!(bodies[1], [0, 0x0b]);
    let mut rejoined: Vec<u8> = Vec::new();
    for body in added {
        assert!(body.len() <= relocations::CHUNK);
        assert_eq!((body[0], body[body.len() - 1]), (0, 0x0b));
        rejoined.extend(&body[1..body.len() - 1]);
    }
    assert_eq!(rejoined, stores);
    let names = module::custom(&split, "name").unwrap().unwrap();
    let last = format!("__wasm_apply_data_relocs.{}", added.len() - 1);
    assert!(names.ends_with(last.as_bytes()));
    assert_eq!(relocations::with_split_relocations(&split).unwrap(), split);
}

fn signed(value: i64) -> Vec<u8> {
    let mut out = Vec::new();
    module::signed(value, &mut out);
    out
}

fn bodies(bytes: &[u8]) -> Vec<Vec<u8>> {
    let code = module::sections(bytes)
        .unwrap()
        .into_iter()
        .find(|section| section.id == 10)
        .unwrap();
    let mut reader = module::Reader::new(code.contents);
    let count = reader.u32().unwrap();
    (0..count)
        .map(|_| {
            let length = reader.u32().unwrap() as usize;
            reader.take(length).unwrap().to_vec()
        })
        .collect()
}
