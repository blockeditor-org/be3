use std::ffi::CString;

use pulseaudio::protocol::{SinkInfo, SourceInfo};

use super::*;
use audio::{Unreachable, default_sink, default_source};

fn sink(name: &str) -> SinkInfo {
    SinkInfo {
        name: CString::new(name).expect("a name"),
        ..SinkInfo::default()
    }
}

fn source(name: &str, monitor: Option<u32>) -> SourceInfo {
    SourceInfo {
        name: CString::new(name).expect("a name"),
        monitor_of_sink_index: monitor,
        ..SourceInfo::default()
    }
}

fn named(name: &str) -> CString {
    CString::new(name).expect("a name")
}

#[test]
fn the_default_output_is_the_one_the_server_names_or_else_the_first() {
    let sinks = || vec![sink("hdmi"), sink("speakers")];
    let chosen = |name: Option<&str>| {
        let name = name.map(named);
        default_sink(sinks(), name.as_deref()).map(|sink| sink.name)
    };
    assert_eq!(chosen(Some("speakers")), Some(named("speakers")));
    assert_eq!(
        chosen(Some("gone")),
        Some(named("hdmi")),
        "a default the server has not caught up on still leaves an output"
    );
    assert_eq!(chosen(None), Some(named("hdmi")));
    assert_eq!(default_sink(Vec::new(), Some(&named("hdmi"))), None);

    let sources = || vec![source("hdmi.monitor", Some(0)), source("mic", None)];
    let heard = |name: Option<&str>| {
        let name = name.map(named);
        default_source(sources(), name.as_deref()).map(|source| source.name)
    };
    assert_eq!(heard(Some("hdmi.monitor")), Some(named("hdmi.monitor")));
    assert_eq!(
        heard(Some("gone")),
        Some(named("mic")),
        "without the named default, a monitor is no microphone"
    );

    let mut unreachable = Unreachable::new();
    assert!(unreachable.first("there is no server socket"));
    assert!(
        !unreachable.first("there is no server socket"),
        "the same failure is told once, not on every key"
    );
    assert!(unreachable.first("connection refused"));
    unreachable.reached();
    assert!(unreachable.first("connection refused"));
}
