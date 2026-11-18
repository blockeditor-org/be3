use super::*;

const WIT: &str = "
package be3:test@0.1.0;

interface host {
    type password = list<u8>;

    record login {
        user: string,
        password: password,
    }

    unlock: async func(password: password);
    sign-in: async func(logins: list<login>);
    count: async func(n: u32) -> u32;
}

world test {
    import host;
}
";

#[test]
fn only_a_call_taking_a_secret_zeroes_its_arguments() {
    let mut resolve = Resolve::new();
    let package = resolve
        .push_str("test.wit", WIT)
        .expect("the test's WIT parses");
    let options = Options {
        world: "test".to_owned(),
        secret: ["password".to_owned()].into_iter().collect(),
        ..Options::default()
    };
    let generated = Generator::generate(options, resolve, package)
        .expect("it generates")
        .to_string();
    let flat: String = generated.split_whitespace().collect();
    assert_eq!(
        flat.matches("::be_wasm_ops::scrub(").count(),
        2,
        "the call taking a password and the one taking it inside a list of records zero their \
         arguments, and the one taking none does not"
    );
    assert!(flat.contains("letmutstarted=started;"));
    assert!(
        flat.contains("#[derive(Debug,::serde::Serialize,::serde::Deserialize)]pubstructLogin"),
        "a record holding a secret is neither copied nor compared"
    );

    // With no secret named, no call pays for it.
    let mut resolve = Resolve::new();
    let package = resolve
        .push_str("test.wit", WIT)
        .expect("the test's WIT parses");
    let options = Options {
        world: "test".to_owned(),
        ..Options::default()
    };
    let generated = Generator::generate(options, resolve, package)
        .expect("it generates")
        .to_string();
    assert!(!generated.contains("scrub"));
    assert!(!generated.contains("let mut started"));
}
