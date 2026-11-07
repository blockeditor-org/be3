use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

use super::arguments;

#[test]
fn flags_come_before_the_command() {
    let options = arguments(&["--library-path", "a:b", "--time-limit", "20m", "--passed", "out", "--", "test", "--passed"]).unwrap();
    assert_eq!(options.library_path, Some(OsString::from("a:b")));
    assert_eq!(options.time_limit, Some(("20m".to_owned(), Duration::from_secs(1200))));
    assert_eq!(options.passed, Some(PathBuf::from("out")));
    assert_eq!(options.command, [OsString::from("test"), OsString::from("--passed")]);
    assert!(arguments(&["--passed", "out"]).is_err());
    assert!(arguments(&["--unknown", "--", "test"]).is_err());
    assert!(arguments(&["--passed"]).is_err());
}
