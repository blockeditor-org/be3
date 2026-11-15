//! The same measurements on wasmtime 48, configured as be-wasm-host's App flavour, to compare
//! with ../v8 (results in ../FINDINGS.md).
//!
//! - `wasm-on-wasmtime`: the per-call cost of a `func_wrap` import.
//! - `wasm-on-wasmtime compile A.wasm B.wasm ...`: Cranelift compile time and `.cwasm` size.
//! - `wasm-on-wasmtime bench GUEST.wasm`: run time of ../guest's `run` export.
use std::time::Instant;
use wasmtime::*;

fn config(winch: bool) -> Config {
    let mut c = Config::new();
    c.wasm_threads(true);
    c.shared_memory(true);
    c.wasm_exceptions(true);
    c.wasm_bulk_memory(true);
    c.epoch_interruption(true);
    if winch { c.strategy(Strategy::Winch); }
    c
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("compile") {
        let engine = Engine::new(&config(false)).unwrap();
        for path in &args[1..] {
            let bytes = std::fs::read(path).unwrap();
            let started = Instant::now();
            let module = Module::new(&engine, &bytes);
            println!("{path} ({:.1} MB) cranelift: {:?} wall ({})", bytes.len() as f64 / 1e6, started.elapsed(),
                if module.is_ok() { "ok" } else { "FAILED" });
            if let Ok(module) = module {
                println!("    serialized .cwasm: {:.1} MB", module.serialize().unwrap().len() as f64 / 1e6);
            }
        }
        return;
    }
    if args.first().map(String::as_str) == Some("bench") {
        // Winch is left out: it refuses to start with wasm exceptions on.
        for winch in [false] {
            let engine = Engine::new(&config(winch)).unwrap();
            let started = Instant::now();
            let module = Module::from_file(&engine, &args[1]).unwrap();
            let compiled = started.elapsed();
            let mut store = Store::new(&engine, 0u64);
            store.set_epoch_deadline(u64::MAX / 2);
            let instance = Instance::new(&mut store, &module, &[]).unwrap();
            let run = instance.get_typed_func::<u32, u64>(&mut store, "run").unwrap();
            let mut times = Vec::new();
            for _ in 0..12 {
                let started = Instant::now();
                run.call(&mut store, 20).unwrap();
                times.push(format!("{:.1}", started.elapsed().as_secs_f64() * 1e3));
            }
            println!("wasmtime {} (compile {compiled:?}) run(20) ms per call: {}", if winch { "winch" } else { "cranelift" }, times.join(" "));
        }
        return;
    }
    let engine = Engine::new(&config(false)).unwrap();
    let wat = r#"(module
      (import "host" "add" (func $add (param i32 i32) (result i32)))
      (func (export "loop_fast") (param $n i32) (result i32) (local $acc i32)
        (block $done (loop $l
          (br_if $done (i32.eqz (local.get $n)))
          (local.set $acc (call $add (local.get $acc) (local.get $n)))
          (local.set $n (i32.sub (local.get $n) (i32.const 1)))
          (br $l)))
        (local.get $acc)))"#;
    let module = Module::new(&engine, wat).unwrap();
    let mut store = Store::new(&engine, 0u64);
    store.set_epoch_deadline(u64::MAX / 2);
    let mut linker = Linker::new(&engine);
    linker.func_wrap("host", "add", |mut caller: Caller<'_, u64>, a: i32, b: i32| -> i32 {
        *caller.data_mut() += 1;
        a.wrapping_add(b)
    }).unwrap();
    let instance = linker.instantiate(&mut store, &module).unwrap();
    let f = instance.get_typed_func::<i32, i32>(&mut store, "loop_fast").unwrap();
    let n = 20_000_000;
    f.call(&mut store, 1000).unwrap();
    let started = Instant::now();
    let r = f.call(&mut store, n).unwrap();
    let took = started.elapsed();
    println!("wasmtime func_wrap: {n} calls in {took:?}, {:.2} ns per call (result {r})", took.as_nanos() as f64 / n as f64);
}
