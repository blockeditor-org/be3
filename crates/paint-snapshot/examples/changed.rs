fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let base = match arguments.as_slice() {
        [] => "HEAD",
        [base] => base.as_str(),
        _ => usage(),
    };
    let root = paint_snapshot::root().unwrap_or_else(|error| fail(&error));
    let changes = paint_snapshot::changes(&root, base).unwrap_or_else(|error| fail(&error));
    if changes.is_empty() {
        println!("no painting in snapshots/ differs from {base}");
        return;
    }
    for change in &changes {
        for reason in change.reasons() {
            println!("{}: {reason}", change.name);
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: changed [BASE]");
    std::process::exit(2)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
