use super::*;

#[test]
fn an_sdk_link_takes_libcxx_whole_once() {
    let (rewritten, linked) = sdk_arguments(arguments("-flavor wasm -lc++ -l c++abi -lc -lc++"));
    assert_eq!(
        &rewritten[..11],
        arguments(
            "-flavor wasm --whole-archive -lc++ --no-whole-archive \
             --whole-archive -l c++abi --no-whole-archive -lc -lc++"
        )
    );
    assert!(rewritten.contains(&"--undefined=wmemcmp".to_owned()));
    assert_eq!(linked, ["-lc++", "-lc++abi", "-lc", "-lc++"]);
}
