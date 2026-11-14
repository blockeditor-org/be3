//! Nothing in the repository listens on every interface unless whoever runs
//! it asks (guides/hosting.md, "Listening"). Caddy is the trap: it listens on
//! every interface whatever host a site address names, unless its global
//! options set `default_bind`. So every Caddyfile run on a developer's machine
//! or in a test binds loopback unless told otherwise, and no other file writes
//! a wildcard address outside a comment, except the one place a deployment
//! asks for it.

/// The files that write wildcard addresses to look for them: this check and
/// its test.
const ALLOWED: &[&str] = &[
    "crates/be-tool-verify/src/binds.rs",
    "crates/be-tool-verify/src/tests/a_wildcard_bind_outside_a_deployment_is_reported.rs",
];

/// The lines, as a path and the line trimmed, that may write a wildcard
/// address: `--domain`, a public deployment.
const ALLOWED_LINES: &[(&str, &str)] = &[("crates/be-host/web/serve.sh", "bind=\"[::]\"")];

/// The Caddyfiles of servers deployed by hand, which face the internet on
/// purpose (guides/build_server.md).
const DEPLOYED: &str = "scripts/internal/";

const WILDCARDS: &[&str] = &[
    "0.0.0.0",
    "[::]:",
    "\"[::]\"",
    "'[::]'",
    "=[::]",
    "Addr::UNSPECIFIED",
];

/// What a Node `listen` must name, or it listens on every interface.
const LOOPBACK: &[&str] = &["127.0.0.1", "localhost", "::1"];

const SCRIPTS: &[&str] = &[".js", ".mjs", ".cjs", ".ts"];

/// What is wrong with the tracked file at `path`, holding `text`, as a line each.
pub fn wildcard_binds(path: &str, text: &str) -> Vec<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let mut problems = Vec::new();
    if (name == "Caddyfile" || name.ends_with(".Caddyfile"))
        && !path.starts_with(DEPLOYED)
        && !text.lines().any(binds_loopback)
    {
        problems.push(format!(
            "{path}: a Caddyfile needs `default_bind {{$NAME_BIND:127.0.0.1}}` in its global options, or Caddy listens on every interface"
        ));
    }
    if ALLOWED.contains(&path) || path.ends_with(".md") || path.starts_with(DEPLOYED) {
        return problems;
    }
    for (index, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if ["#", "//", "*", "/*"]
            .iter()
            .any(|comment| code.starts_with(comment))
        {
            continue;
        }
        if ALLOWED_LINES.contains(&(path, code.trim_end())) {
            continue;
        }
        if SCRIPTS.iter().any(|extension| path.ends_with(extension))
            && code.contains(".listen(")
            && !LOOPBACK.iter().any(|host| code.contains(host))
        {
            problems.push(format!(
                "{path}:{}: a `listen` that names no host listens on every interface; pass \"127.0.0.1\"",
                index + 1
            ));
        }
        if let Some(wildcard) = WILDCARDS.iter().find(|wildcard| code.contains(*wildcard)) {
            problems.push(format!(
                "{path}:{}: `{wildcard}` listens on every interface; default to loopback and let whoever runs it pass the address",
                index + 1
            ));
        }
    }
    problems
}

/// A `default_bind` that falls back to, or names, loopback alone.
fn binds_loopback(line: &str) -> bool {
    let mut words = line.split_whitespace();
    if words.next() != Some("default_bind") {
        return false;
    }
    let hosts: Vec<&str> = words.collect();
    !hosts.is_empty()
        && hosts.iter().all(|host| {
            let host = match host.strip_prefix("{$") {
                Some(variable) => variable
                    .strip_suffix('}')
                    .and_then(|variable| variable.split_once(':'))
                    .map_or("", |(_, fallback)| fallback),
                None => host,
            };
            matches!(host, "127.0.0.1" | "[::1]" | "localhost")
        })
}
