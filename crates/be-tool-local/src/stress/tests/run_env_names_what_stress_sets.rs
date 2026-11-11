use super::*;

/// `run_env` of the host tests' `cargo_test` in crates/be-host-tests/BUCK is what the harness
/// needs; `ENVIRONMENT` is what `stress` sets, so a variable added to one and not the other
/// would make stress run a harness `./scripts/test` does not.
#[test]
fn run_env_names_what_stress_sets() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let buck = fs::read_to_string(Path::new(manifest).join("../be-host-tests/BUCK"))
        .expect("the host tests' BUCK");
    let start = buck.find("run_env = {").expect("run_env");
    let block = &buck[start..buck[start..].find("},").map_or(buck.len(), |end| start + end)];
    let mut named: Vec<&str> = block
        .lines()
        .filter_map(|line| line.trim().strip_prefix('"'))
        .filter_map(|line| line.split('"').next())
        .collect();
    // The manifest directory is set by the test runner there and by stress here.
    named.push("CARGO_MANIFEST_DIR");
    named.sort_unstable();
    let mut set = ENVIRONMENT.to_vec();
    set.sort_unstable();
    assert_eq!(named, set);
}
