fn main() {
    let mut arguments = std::env::args().skip(1);
    let Some(linker) = arguments.next() else {
        eprintln!("usage: wasm-link LINKER [ARGUMENT]...");
        std::process::exit(2);
    };
    if let Err(error) = wasm_link::run(&linker, arguments.collect()) {
        if !error.is_empty() {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}
