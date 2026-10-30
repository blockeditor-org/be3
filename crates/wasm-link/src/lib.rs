pub mod archive;
pub mod inputs;
pub mod map;
pub mod module;
pub mod object;
pub mod tls;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
};

use wasm_dylink::{
    Declared, ENV, GOT_FUNCTION, GOT_MEMORY, GOT_THREAD_LOCAL, MEMORY_BASE, STACK_POINTER, TABLE,
    TABLE_BASE, TLS_ALIGN, TLS_SIZE,
};

pub const NEEDS_FLAG: &str = "--be3-needs=";
pub const LIBRARY_FLAG: &str = "--be3-library=";
pub const LIBRARY_SECTION: &str = "be3.lib";

const LINKED_BY_LOADER: [&str; 5] = ["memory", TABLE, STACK_POINTER, MEMORY_BASE, TABLE_BASE];
const SYNTHESIZED: [&str; 12] = [
    "__dso_handle",
    "__data_end",
    "__global_base",
    "__heap_base",
    "__heap_end",
    "__stack_low",
    "__stack_high",
    "__tls_base",
    "__tls_size",
    "__tls_align",
    "__wasm_call_ctors",
    "__wasm_apply_data_relocs",
];
const DROPPED_FROM_PLUGINS: [&str; 5] = [
    "--import-memory",
    "--export-memory",
    "--stack-first",
    "--no-entry",
    "--export=__heap_base",
];
const PLUGIN_FLAGS: [&str; 7] = [
    "-shared",
    "--experimental-pic",
    "--unresolved-symbols=import-dynamic",
    "--export=__wasm_init_tls",
    "--export=__tls_size",
    "--export=__tls_align",
    "--export=__tls_base",
];
const ROOT_FLAGS: [&str; 3] = ["--export-all", "--export-table", "--growable-table"];
const UNUSED_SECTIONS: [&str; 3] = [".rmeta", ".rmeta-link", "__wasm_bindgen_unstable"];
const WHOLE_LIBRARIES: [&str; 2] = ["-lc++", "-lc++abi"];
const LIBC: [&str; 63] = [
    "__assert_fail",
    "atof",
    "atoi",
    "atol",
    "fclose",
    "fdopen",
    "feof",
    "ferror",
    "fflush",
    "fopen",
    "fprintf",
    "fputc",
    "fread",
    "fseek",
    "ftell",
    "gettimeofday",
    "gmtime",
    "isalnum",
    "isalpha",
    "isdigit",
    "islower",
    "isprint",
    "isspace",
    "isupper",
    "iswalnum",
    "iswalpha",
    "iswdigit",
    "iswlower",
    "iswspace",
    "iswupper",
    "isxdigit",
    "pthread_cond_broadcast",
    "pthread_cond_destroy",
    "pthread_cond_init",
    "pthread_cond_signal",
    "pthread_cond_wait",
    "pthread_mutex_destroy",
    "pthread_mutex_init",
    "pthread_mutex_lock",
    "pthread_mutex_trylock",
    "pthread_mutex_unlock",
    "pthread_once",
    "setlocale",
    "snprintf",
    "strcat",
    "strchr",
    "strcmp",
    "strncpy",
    "strrchr",
    "strstr",
    "strtod",
    "strtol",
    "strtoul",
    "time",
    "tolower",
    "toupper",
    "towlower",
    "towupper",
    "vsnprintf",
    "wcslen",
    "wmemcmp",
    "wmemcpy",
    "wmemset",
];
const MATH: [&str; 44] = [
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atan2",
    "atanh",
    "cbrt",
    "ceil",
    "copysign",
    "cos",
    "cosh",
    "erf",
    "erfc",
    "exp",
    "exp2",
    "expm1",
    "fdim",
    "floor",
    "fma",
    "fmax",
    "fmin",
    "fmod",
    "frexp",
    "hypot",
    "ldexp",
    "lgamma",
    "log",
    "log10",
    "log1p",
    "log2",
    "llrint",
    "llround",
    "lrint",
    "lround",
    "modf",
    "pow",
    "remainder",
    "round",
    "sin",
    "sinh",
    "sqrt",
    "tan",
    "tanh",
];

#[derive(Debug, PartialEq, Eq)]
pub enum Mode {
    Plain,
    Linked {
        needs: Vec<PathBuf>,
        library: Option<String>,
    },
}

pub fn run(linker: &str, arguments: Vec<String>) -> Result<(), String> {
    let arguments = expand(arguments)?;
    let (mode, arguments) = mode(arguments);
    match mode {
        Mode::Plain => link(linker, &arguments),
        Mode::Linked {
            needs,
            library: Some(name),
        } if needs.is_empty() => link_root(linker, arguments, &name),
        Mode::Linked { needs, library } => link_linked(linker, arguments, &needs, library),
    }
}

pub fn expand(arguments: Vec<String>) -> Result<Vec<String>, String> {
    let mut expanded = Vec::with_capacity(arguments.len());
    for argument in arguments {
        match argument.strip_prefix('@') {
            Some(path) => {
                let contents = std::fs::read_to_string(path).map_err(|error| {
                    format!("the response file {path} could not be read: {error}")
                })?;
                expanded.extend(expand(tokenize(&contents))?);
            }
            None => expanded.push(argument),
        }
    }
    Ok(expanded)
}

pub fn tokenize(contents: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote = None;
    let mut characters = contents.chars();
    while let Some(character) = characters.next() {
        match (quote, character) {
            (_, '\\') => {
                if let Some(next) = characters.next() {
                    current.push(next);
                }
                started = true;
            }
            (Some(open), _) if character == open => quote = None,
            (Some(_), _) => current.push(character),
            (None, '"' | '\'') => {
                quote = Some(character);
                started = true;
            }
            (None, _) if character.is_whitespace() => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, _) => {
                current.push(character);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

pub fn mode(arguments: Vec<String>) -> (Mode, Vec<String>) {
    let mut needs = Vec::new();
    let mut library = None;
    let mut linked = false;
    let mut kept = Vec::with_capacity(arguments.len());
    for argument in arguments {
        if let Some(needed) = argument.strip_prefix(NEEDS_FLAG) {
            needs.push(PathBuf::from(needed));
            linked = true;
        } else if let Some(name) = argument.strip_prefix(LIBRARY_FLAG) {
            library = Some(name.to_owned());
            linked = true;
        } else if !argument.starts_with("-fuse-ld=") {
            kept.push(argument);
        }
    }
    let mode = match linked {
        true => Mode::Linked { needs, library },
        false => Mode::Plain,
    };
    (mode, kept)
}

fn link(linker: &str, arguments: &[String]) -> Result<(), String> {
    let status = Command::new(linker)
        .args(arguments)
        .status()
        .map_err(|error| format!("{linker} could not run: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(String::new())
    }
}

fn output(arguments: &[String]) -> Result<PathBuf, String> {
    arguments
        .iter()
        .position(|argument| argument == "-o")
        .and_then(|at| arguments.get(at + 1))
        .map(PathBuf::from)
        .ok_or_else(|| "the link names no output".to_owned())
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn is_rust(argument: &str) -> bool {
    argument.ends_with(".rlib") || argument.ends_with(".o")
}

fn is_archive(argument: &str) -> bool {
    argument.ends_with(".rlib") || argument.ends_with(".a")
}

pub fn root_arguments(arguments: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut linked = Vec::new();
    let mut rewritten = Vec::with_capacity(arguments.len() + ROOT_FLAGS.len());
    let mut arguments = arguments.into_iter();
    let mut whole = HashSet::new();
    while let Some(argument) = arguments.next() {
        let library = if argument == "-l" {
            let Some(library) = arguments.next() else {
                continue;
            };
            Some((format!("-l{library}"), vec![argument.clone(), library]))
        } else if argument.starts_with("-l") {
            Some((argument.clone(), vec![argument.clone()]))
        } else {
            None
        };
        if let Some((library, given)) = library {
            linked.push(library.clone());
            if WHOLE_LIBRARIES.contains(&library.as_str()) && whole.insert(library.clone()) {
                rewritten.push("--whole-archive".to_owned());
                rewritten.extend(given);
                rewritten.push("--no-whole-archive".to_owned());
            } else {
                rewritten.extend(given);
            }
            continue;
        }
        if is_archive(&argument) {
            linked.push(basename(&argument).to_owned());
        }
        if is_rust(&argument) {
            rewritten.extend([
                "--whole-archive".to_owned(),
                argument,
                "--no-whole-archive".to_owned(),
            ]);
        } else {
            rewritten.push(argument);
        }
    }
    rewritten.extend(ROOT_FLAGS.map(str::to_owned));
    for function in MATH {
        rewritten.push(format!("--undefined={function}"));
        rewritten.push(format!("--undefined={function}f"));
    }
    rewritten.extend([
        "--undefined=tgamma".to_owned(),
        "--undefined=tgammaf".to_owned(),
    ]);
    for function in LIBC {
        rewritten.push(format!("--undefined={function}"));
    }
    (rewritten, linked)
}

fn scratch(out: &Path, suffix: &str) -> PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn link_mapped(
    linker: &str,
    mut arguments: Vec<String>,
    map: &Path,
) -> Result<map::ThreadLocals, String> {
    arguments.push(format!("-Map={}", map.display()));
    let linked = link(linker, &arguments);
    let contents = std::fs::read_to_string(map);
    let _ = std::fs::remove_file(map);
    linked?;
    map::thread_locals(
        &contents.map_err(|error| format!("the link map could not be read: {error}"))?,
    )
}

pub fn listing(
    name: &str,
    needs: &[&str],
    linked: &[String],
    thread_locals: &map::ThreadLocals,
) -> String {
    let mut listing = format!("name {name}\n");
    for identity in needs {
        listing.push_str(&format!("need {identity}\n"));
    }
    for archive in linked {
        listing.push_str(&format!("link {archive}\n"));
    }
    let mut seen = HashSet::new();
    for symbol in &thread_locals.symbols {
        if seen.insert(&symbol.name) {
            listing.push_str(&format!("tls {}\n", symbol.name));
        }
    }
    listing
}

fn link_root(linker: &str, arguments: Vec<String>, name: &str) -> Result<(), String> {
    let out = output(&arguments)?;
    let (arguments, linked) = root_arguments(arguments);
    let thread_locals = link_mapped(linker, arguments, &scratch(&out, ".map"))?;
    let bytes = without_unused_sections(&read(&out)?)?;
    let listing = listing(name, &[], &linked, &thread_locals);
    write(
        &out,
        &module::with_custom(&bytes, LIBRARY_SECTION, listing.as_bytes()),
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub name: String,
    pub identity: String,
    pub needs: Vec<String>,
    pub linked: Vec<String>,
    pub thread_locals: HashSet<String>,
    pub functions: HashSet<String>,
    pub globals: HashSet<String>,
}

impl Library {
    fn read(path: &Path) -> Result<Self, String> {
        let bytes = read(path)?;
        Self::parse(&bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let listing = module::custom(bytes, LIBRARY_SECTION)?
            .ok_or("it is not a library: it has no be3.lib section")?;
        let mut library = Library {
            identity: wasm_dylink::identity(bytes),
            ..Library::default()
        };
        for line in String::from_utf8_lossy(listing).lines() {
            let Some((kind, value)) = line.split_once(' ') else {
                continue;
            };
            match kind {
                "name" => library.name = value.to_owned(),
                "need" => library.needs.push(value.to_owned()),
                "link" => library.linked.push(value.to_owned()),
                "tls" => {
                    library.thread_locals.insert(value.to_owned());
                }
                _ => {}
            }
        }
        for export in module::exports(bytes)? {
            match export.kind {
                module::FUNCTION | module::TAG => library.functions.insert(export.name),
                module::GLOBAL_KIND => library.globals.insert(export.name),
                _ => false,
            };
        }
        Ok(library)
    }
}

pub fn crate_of(archive: &str) -> String {
    if let Some(library) = archive.strip_prefix("-l") {
        return format!("the C library {library}");
    }
    let Some(stem) = archive.strip_suffix(".rlib") else {
        return format!("the archive {archive}");
    };
    let stem = stem.strip_prefix("lib").unwrap_or(stem);
    let name = match stem.rsplit_once('-') {
        Some((name, _)) => name,
        None => stem,
    };
    format!("the crate {name}")
}

pub fn closure(given: Vec<Library>) -> Result<Vec<Library>, String> {
    let named: HashSet<String> = given
        .iter()
        .map(|library| library.identity.clone())
        .collect();
    for library in &given {
        if let Some(missing) = library.needs.iter().find(|need| !named.contains(*need)) {
            return Err(format!(
                "{} needs the library {missing}, which this link does not name: name every \
                 library under a module in its libraries",
                library.name
            ));
        }
    }
    let mut ordered: Vec<Library> = Vec::with_capacity(given.len());
    let mut waiting = given;
    while !waiting.is_empty() {
        let placed: HashSet<&str> = ordered
            .iter()
            .map(|library| library.identity.as_str())
            .collect();
        let Some(next) = waiting.iter().position(|library| {
            library
                .needs
                .iter()
                .all(|need| placed.contains(need.as_str()))
        }) else {
            return Err("the libraries need each other in a cycle".to_owned());
        };
        let library = waiting.remove(next);
        if !placed.contains(library.identity.as_str()) {
            ordered.push(library);
        }
    }
    let mut owners: HashMap<&str, &str> = HashMap::new();
    for library in &ordered {
        for archive in &library.linked {
            if let Some(owner) = owners.insert(archive, &library.name)
                && owner != library.name
            {
                return Err(format!(
                    "{} is in both {owner} and {}: a crate or C library may be in only one module \
                     of a plugin's closure",
                    crate_of(archive),
                    library.name
                ));
            }
        }
    }
    Ok(ordered)
}

pub fn plugin_arguments(arguments: Vec<String>, linked: &HashSet<String>) -> Vec<String> {
    let mut rewritten = Vec::with_capacity(arguments.len() + PLUGIN_FLAGS.len());
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "-l" || argument == "-z" {
            if let Some(value) = arguments.next() {
                let dropped = (argument == "-l" && linked.contains(&format!("-l{value}")))
                    || (argument == "-z" && value.starts_with("stack-size="));
                if !dropped {
                    rewritten.extend([argument, value]);
                }
            }
            continue;
        }
        let dropped = DROPPED_FROM_PLUGINS.contains(&argument.as_str())
            || (argument.starts_with("-l") && linked.contains(&argument))
            || (is_archive(&argument) && linked.contains(basename(&argument)));
        if !dropped {
            rewritten.push(argument);
        }
    }
    rewritten.extend(PLUGIN_FLAGS.map(str::to_owned));
    rewritten
}

pub fn kept(arguments: &[String]) -> Vec<String> {
    let mut kept = Vec::new();
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        if argument == "-l" {
            if let Some(library) = arguments.next() {
                kept.push(format!("-l{library}"));
            }
        } else if argument.starts_with("-l") {
            kept.push(argument.clone());
        } else if argument == "-o" || argument == "-L" || argument == "-z" {
            arguments.next();
        } else if is_archive(argument) {
            kept.push(basename(argument).to_owned());
        }
    }
    kept
}

pub fn unresolved(bytes: &[u8], name: &str, closure: &[Library]) -> Result<(), String> {
    let own: HashSet<String> = module::exports(bytes)?
        .into_iter()
        .map(|export| export.name)
        .collect();
    let mut missing = Vec::new();
    let mut twice = Vec::new();
    for import in module::imports(bytes)? {
        let wanted = import.name.as_str();
        let definers: Vec<&str> = closure
            .iter()
            .filter(|library| match import.module.as_str() {
                ENV | GOT_FUNCTION => library.functions.contains(wanted),
                GOT_MEMORY => library.globals.contains(wanted),
                GOT_THREAD_LOCAL => library.thread_locals.contains(wanted),
                _ => false,
            })
            .map(|library| library.name.as_str())
            .collect();
        let checked = match import.module.as_str() {
            ENV => !LINKED_BY_LOADER.contains(&wanted),
            GOT_MEMORY | GOT_FUNCTION => !own.contains(wanted),
            GOT_THREAD_LOCAL => true,
            _ => false,
        };
        if !checked {
            continue;
        }
        let symbol = format!("{}.{wanted}", import.module);
        match definers[..] {
            [] => missing.push(symbol),
            [_] => {}
            _ if SYNTHESIZED.contains(&wanted) => {}
            _ => twice.push(format!("{symbol} (in {})", definers.join(" and "))),
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "{name} imports {} symbols that no library it is linked with exports; add the crate \
             or C library that defines them to one of its libraries:\n  {}",
            missing.len(),
            missing.join("\n  ")
        ));
    }
    if !twice.is_empty() {
        return Err(format!(
            "{name} imports {} symbols that more than one library defines:\n  {}",
            twice.len(),
            twice.join("\n  ")
        ));
    }
    Ok(())
}

pub fn declared(bytes: &[u8]) -> Result<Declared, String> {
    let info = module::memory_info(bytes)?.ok_or("the module has no dylink.0 section")?;
    let constant = |name: &str, default: u32| -> Result<u32, String> {
        Ok(module::exported_constant(bytes, name)?.map_or(default, |value| value as u32))
    };
    Ok(Declared {
        memory_size: info.memory_size,
        memory_align: info.memory_align,
        table_size: info.table_size,
        table_align: info.table_align,
        tls_size: constant(TLS_SIZE, 0)?,
        tls_align: constant(TLS_ALIGN, 1)?,
    })
}

pub fn described(bytes: &[u8], closure: &[Library]) -> Result<Vec<u8>, String> {
    let mut globals = vec![(declared(bytes)?.export(), 0)];
    for (position, library) in closure.iter().enumerate() {
        globals.push((wasm_dylink::library_export(position, &library.identity), 0));
    }
    module::with_exported_globals(bytes, &globals)
}

pub fn library_arguments(arguments: Vec<String>) -> Vec<String> {
    let mut rewritten = Vec::with_capacity(arguments.len() + 1);
    for argument in arguments {
        match is_rust(&argument) {
            true => rewritten.extend([
                "--whole-archive".to_owned(),
                argument,
                "--no-whole-archive".to_owned(),
            ]),
            false => rewritten.push(argument),
        }
    }
    rewritten.push("--export-all".to_owned());
    rewritten
}

fn link_linked(
    linker: &str,
    arguments: Vec<String>,
    needs: &[PathBuf],
    library: Option<String>,
) -> Result<(), String> {
    let out = output(&arguments)?;
    let name = library.clone().unwrap_or_else(|| {
        out.file_stem().map_or_else(
            || out.display().to_string(),
            |stem| stem.to_string_lossy().into_owned(),
        )
    });
    let closure = closure(
        needs
            .iter()
            .map(|path| Library::read(path))
            .collect::<Result<_, _>>()?,
    )?;
    let dropped: HashSet<String> = closure
        .iter()
        .flat_map(|library| library.linked.iter().cloned())
        .collect();
    let arguments = plugin_arguments(arguments, &dropped);
    let kept = kept(&arguments);
    let arguments = match library {
        Some(_) => library_arguments(arguments),
        None => arguments,
    };
    let (arguments, rewritten) = inputs::through_got(arguments, &out)?;
    let thread_locals = link_mapped(linker, arguments, &scratch(&out, ".map"));
    for path in rewritten {
        let _ = std::fs::remove_file(path);
    }
    let thread_locals = thread_locals?;
    let bytes = without_unused_sections(&read(&out)?)?;
    unresolved(&bytes, &name, &closure)?;
    let needs: Vec<&str> = closure
        .iter()
        .map(|library| library.identity.as_str())
        .collect();
    let listing = listing(&name, &needs, &kept, &thread_locals);
    write(
        &out,
        &module::with_custom(
            &described(&bytes, &closure)?,
            LIBRARY_SECTION,
            listing.as_bytes(),
        ),
    )
}

fn without_unused_sections(bytes: &[u8]) -> Result<Vec<u8>, String> {
    module::without_custom(bytes, |name| UNUSED_SECTIONS.contains(&name))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{} could not be read: {error}", path.display()))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes)
        .map_err(|error| format!("{} could not be written: {error}", path.display()))
}

#[cfg(test)]
mod tests;
