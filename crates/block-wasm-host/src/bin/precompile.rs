use std::path::PathBuf;

use block_wasm_host::precompile;

fn main() {
    let mut target = None;
    let mut modules: Vec<PathBuf> = Vec::new();
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--target" => match arguments.next() {
                Some(triple) => target = Some(triple),
                None => usage(),
            },
            _ => modules.push(PathBuf::from(argument)),
        }
    }
    if modules.is_empty() {
        usage();
    }
    if let Err(error) = precompile(&modules, target.as_deref()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    let described = target.as_deref().unwrap_or("this machine");
    println!("Compiled {} plugins for {described}", modules.len());
}

fn usage() -> ! {
    eprintln!("usage: precompile [--target TRIPLE] PLUGIN.wasm...");
    std::process::exit(2);
}
