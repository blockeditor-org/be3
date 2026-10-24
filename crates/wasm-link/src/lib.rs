pub mod map;
pub mod module;
pub mod object;

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

use sha2::{Digest, Sha256};
use wasm_dylink::{ENV, GOT_FUNCTION, GOT_MEMORY, MEMORY_BASE, STACK_POINTER, TABLE, TABLE_BASE};

pub const SDK_FLAG: &str = "--be3-sdk";
pub const PLUGIN_FLAG: &str = "--be3-plugin=";
pub const SDK_SECTION: &str = "be3.sdk";
pub use wasm_dylink::{LAYOUT_EXPORT_PREFIX, PLUGIN_TLS, PLUGIN_TLS_SIZE, SDK_EXPORT_PREFIX};
const PLUGIN_TLS_ALIGN_LOG2: u32 = 4;
const SDK_TLS_SEGMENT: &str = ".tbss.__be3_sdk_tls";
const PLUGIN_TLS_SEGMENT: &str = ".tbss.__be3_plugin_tls";

const LINKED_BY_LOADER: [&str; 5] = ["memory", TABLE, STACK_POINTER, MEMORY_BASE, TABLE_BASE];
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
const SDK_FLAGS: [&str; 3] = ["--export-all", "--export-table", "--growable-table"];
const UNUSED_SECTIONS: [&str; 3] = [".rmeta", ".rmeta-link", "__wasm_bindgen_unstable"];
const WHOLE_LIBRARIES: [&str; 2] = ["-lc++", "-lc++abi"];
const LIBC: [&str; 41] = [
    "atof",
    "atoi",
    "atol",
    "gettimeofday",
    "isalnum",
    "isalpha",
    "isdigit",
    "islower",
    "isprint",
    "isspace",
    "isupper",
    "isxdigit",
    "iswalnum",
    "iswalpha",
    "iswdigit",
    "iswlower",
    "iswspace",
    "iswupper",
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
    "strtod",
    "strtol",
    "strtoul",
    "time",
    "tolower",
    "toupper",
    "towlower",
    "towupper",
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
    Sdk,
    Plugin(PathBuf),
}

pub fn run(linker: &str, arguments: Vec<String>) -> Result<(), String> {
    let arguments = expand(arguments)?;
    let (mode, arguments) = mode(arguments);
    match mode {
        Mode::Plain => link(linker, &arguments),
        Mode::Sdk => link_sdk(linker, arguments),
        Mode::Plugin(sdk) => link_plugin(linker, arguments, &sdk),
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
    let mut mode = Mode::Plain;
    let mut kept = Vec::with_capacity(arguments.len());
    for argument in arguments {
        if argument == SDK_FLAG {
            mode = Mode::Sdk;
        } else if let Some(sdk) = argument.strip_prefix(PLUGIN_FLAG) {
            mode = Mode::Plugin(PathBuf::from(sdk));
        } else if !argument.starts_with("-fuse-ld=") {
            kept.push(argument);
        }
    }
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

pub fn sdk_arguments(arguments: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut linked = Vec::new();
    let mut rewritten = Vec::with_capacity(arguments.len() + SDK_FLAGS.len());
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
    rewritten.extend(SDK_FLAGS.map(str::to_owned));
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

pub fn sdk_listing(linked: &[String], thread_locals: &map::ThreadLocals) -> Result<String, String> {
    let Some((reserve, shared)) = thread_locals.symbols.split_last() else {
        return Err(
            "the SDK has no thread-local storage, not even the plugins' reserve".to_owned(),
        );
    };
    if reserve.name != PLUGIN_TLS {
        return Err(format!(
            "{PLUGIN_TLS} is not the last of the SDK's thread-locals ({} is), so the plugins' \
             thread-locals would not be laid out after the SDK's",
            reserve.name
        ));
    }
    let mut listing = String::new();
    for library in linked {
        listing.push_str(&format!("link {library}\n"));
    }
    let mut seen = HashSet::new();
    for symbol in shared {
        if seen.insert(&symbol.name) {
            listing.push_str(&format!(
                "tls {} {} {}\n",
                symbol.offset, symbol.size, symbol.name
            ));
        }
    }
    listing.push_str(&format!("reserve {} {}\n", reserve.offset, reserve.size));
    Ok(listing)
}

fn link_sdk(linker: &str, arguments: Vec<String>) -> Result<(), String> {
    let out = output(&arguments)?;
    let reserve = scratch(&out, ".reserve.o");
    write(
        &reserve,
        &object::thread_local_segment(
            PLUGIN_TLS_SEGMENT,
            PLUGIN_TLS_SIZE,
            PLUGIN_TLS_ALIGN_LOG2,
            &[object::Symbol {
                name: PLUGIN_TLS,
                offset: 0,
                size: PLUGIN_TLS_SIZE,
            }],
            object::SYMBOL_NO_STRIP,
        ),
    )?;
    let (mut arguments, linked) = sdk_arguments(arguments);
    arguments.push(reserve.display().to_string());
    let thread_locals = link_mapped(linker, arguments, &scratch(&out, ".map"));
    let _ = std::fs::remove_file(&reserve);
    let listing = sdk_listing(&linked, &thread_locals?)?;
    let bytes = without_unused_sections(&read(&out)?)?;
    write(
        &out,
        &module::with_custom(&bytes, SDK_SECTION, listing.as_bytes()),
    )
}

pub struct ThreadLocal {
    pub name: String,
    pub offset: u32,
    pub size: u32,
}

pub struct Sdk {
    pub identity: String,
    pub linked: HashSet<String>,
    pub importable: HashSet<String>,
    pub exports: HashSet<String>,
    pub thread_locals: Vec<ThreadLocal>,
    pub reserve: (u32, u32),
}

fn number(token: Option<&str>, line: &str) -> Result<u32, String> {
    token
        .and_then(|token| token.parse().ok())
        .ok_or_else(|| format!("the be3.sdk line {line:?} is not one this reads"))
}

impl Sdk {
    pub fn without_unused_sections(bytes: &[u8]) -> Result<Vec<u8>, String> {
        module::without_custom(bytes, |name| UNUSED_SECTIONS.contains(&name))
    }

    fn read(path: &Path) -> Result<Self, String> {
        let bytes = read(path)?;
        Self::parse(&bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let listing = module::custom(bytes, SDK_SECTION)?
            .ok_or("it is not a plugin SDK: it has no be3.sdk section")?;
        let mut linked = HashSet::new();
        let mut thread_locals = Vec::new();
        let mut reserve = None;
        for line in String::from_utf8_lossy(listing).lines() {
            let mut tokens = line.split(' ');
            match tokens.next() {
                Some("link") => {
                    linked.insert(tokens.collect::<Vec<_>>().join(" "));
                }
                Some("tls") => {
                    let offset = number(tokens.next(), line)?;
                    let size = number(tokens.next(), line)?;
                    let name = tokens
                        .next()
                        .ok_or_else(|| format!("{line:?} names no symbol"))?;
                    thread_locals.push(ThreadLocal {
                        name: name.to_owned(),
                        offset,
                        size,
                    });
                }
                Some("reserve") => {
                    reserve = Some((number(tokens.next(), line)?, number(tokens.next(), line)?));
                }
                _ => {}
            }
        }
        let exports = module::exports(bytes)?;
        Ok(Self {
            identity: identity(bytes),
            linked,
            importable: exports
                .iter()
                .filter(|export| matches!(export.kind, module::FUNCTION | module::TAG))
                .map(|export| export.name.clone())
                .collect(),
            exports: exports.into_iter().map(|export| export.name).collect(),
            thread_locals,
            reserve: reserve.ok_or("the SDK reserves no thread-local storage for plugins")?,
        })
    }

    pub fn mirror(&self) -> Vec<u8> {
        let symbols: Vec<object::Symbol<'_>> = self
            .thread_locals
            .iter()
            .map(|local| object::Symbol {
                name: &local.name,
                offset: local.offset,
                size: local.size,
            })
            .collect();
        object::thread_local_segment(
            SDK_TLS_SEGMENT,
            self.reserve.0,
            PLUGIN_TLS_ALIGN_LOG2,
            &symbols,
            object::SYMBOL_HIDDEN,
        )
    }
}

pub fn identity(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn plugin_arguments(arguments: Vec<String>, sdk: &Sdk) -> Vec<String> {
    let mut rewritten = Vec::with_capacity(arguments.len() + PLUGIN_FLAGS.len());
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "-l" || argument == "-z" {
            if let Some(value) = arguments.next() {
                let dropped = (argument == "-l" && sdk.linked.contains(&format!("-l{value}")))
                    || (argument == "-z" && value.starts_with("stack-size="));
                if !dropped {
                    rewritten.extend([argument, value]);
                }
            }
            continue;
        }
        let dropped = DROPPED_FROM_PLUGINS.contains(&argument.as_str())
            || (argument.starts_with("-l") && sdk.linked.contains(&argument))
            || (is_archive(&argument) && sdk.linked.contains(basename(&argument)));
        if !dropped {
            rewritten.push(argument);
        }
    }
    rewritten.extend(PLUGIN_FLAGS.map(str::to_owned));
    rewritten
}

pub fn unresolved(bytes: &[u8], sdk: &Sdk) -> Result<Vec<String>, String> {
    let own: HashSet<String> = module::exports(bytes)?
        .into_iter()
        .map(|export| export.name)
        .collect();
    Ok(module::imports(bytes)?
        .into_iter()
        .filter(|import| match import.module.as_str() {
            ENV => {
                !LINKED_BY_LOADER.contains(&import.name.as_str())
                    && !sdk.importable.contains(&import.name)
            }
            GOT_MEMORY | GOT_FUNCTION => {
                !sdk.exports.contains(&import.name) && !own.contains(&import.name)
            }
            _ => false,
        })
        .map(|import| format!("{}.{}", import.module, import.name))
        .collect())
}

pub fn describe(bytes: &[u8], sdk: &Sdk) -> Result<Vec<u8>, String> {
    let info = module::memory_info(bytes)?.ok_or("the plugin has no dylink.0 section")?;
    let layout = format!(
        "{LAYOUT_EXPORT_PREFIX}{}_{}_{}_{}",
        info.memory_size, info.memory_align, info.table_size, info.table_align
    );
    let globals = [
        (format!("{SDK_EXPORT_PREFIX}{}", sdk.identity), 0),
        (layout, 0),
    ];
    module::with_exported_globals(bytes, &globals)
}

pub fn check_thread_locals(
    thread_locals: &map::ThreadLocals,
    mirror: &Path,
    sdk: &Sdk,
) -> Result<(), String> {
    let mirror = mirror.display().to_string();
    if thread_locals
        .inputs
        .first()
        .is_some_and(|first| !first.starts_with(&mirror))
    {
        return Err(format!(
            "the SDK's thread-locals were not laid out first in the plugin's ({})",
            thread_locals.inputs[0]
        ));
    }
    let (start, size) = sdk.reserve;
    let mirrored: HashSet<&str> = sdk
        .thread_locals
        .iter()
        .map(|local| local.name.as_str())
        .collect();
    let own: Vec<&map::Placed> = thread_locals
        .symbols
        .iter()
        .filter(|symbol| !mirrored.contains(symbol.name.as_str()))
        .collect();
    if let Some(low) = own.iter().find(|symbol| symbol.offset < start) {
        return Err(format!(
            "the plugin's thread-local {} was laid out among the SDK's",
            low.name
        ));
    }
    let needed = own
        .iter()
        .map(|symbol| u64::from(symbol.offset) + u64::from(symbol.size))
        .max()
        .unwrap_or(u64::from(start));
    if needed > u64::from(start) + u64::from(size) {
        return Err(format!(
            "the plugin's thread-locals take {} bytes, more than the {size} the SDK reserves \
             for them (PLUGIN_TLS_SIZE in crates/wasm-link)",
            needed - u64::from(start)
        ));
    }
    Ok(())
}

pub fn with_mirror(mut arguments: Vec<String>, mirror: &Path) -> Vec<String> {
    let first = arguments
        .iter()
        .position(|argument| is_rust(argument) || is_archive(argument))
        .unwrap_or(arguments.len());
    arguments.insert(first, mirror.display().to_string());
    arguments
}

fn link_plugin(linker: &str, arguments: Vec<String>, sdk: &Path) -> Result<(), String> {
    let out = output(&arguments)?;
    let sdk = Sdk::read(sdk)?;
    let mirror = scratch(&out, ".sdk-tls.o");
    write(&mirror, &sdk.mirror())?;
    let arguments = with_mirror(plugin_arguments(arguments, &sdk), &mirror);
    let thread_locals = link_mapped(linker, arguments, &scratch(&out, ".map"));
    let _ = std::fs::remove_file(&mirror);
    check_thread_locals(&thread_locals?, &mirror, &sdk)?;
    let bytes = without_unused_sections(&read(&out)?)?;
    let missing = unresolved(&bytes, &sdk)?;
    if !missing.is_empty() {
        return Err(format!(
            "the plugin needs {} symbols the plugin SDK does not export; add the crate or \
             C library that defines them to crates/block-plugin-sdk:\n  {}",
            missing.len(),
            missing.join("\n  ")
        ));
    }
    write(&out, &describe(&bytes, &sdk)?)
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
