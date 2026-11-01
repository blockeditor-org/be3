use super::*;

const WIT: &str = "
package be3:test@0.1.0;

interface files {
    record scan {
        pages: list<list<u8>>,
        cover: option<list<u8>>,
        pages-read: u32,
    }

    variant change {
        replace(list<u8>),
        clear,
    }

    type entries = list<tuple<string, list<u8>>>;

    read: async func(name: string) -> result<list<u8>, string>;
    chunks: func(name: string) -> stream<list<u8>>;
    nested: async func(entries: entries) -> list<option<list<u8>>>;
    count: async func() -> u32;
}

interface served {
    cover: async func() -> result<list<u8>, string>;
}

world test {
    import files;
    export served;
}
";

fn flat(generated: &str) -> String {
    generated.split_whitespace().collect()
}

#[test]
fn bytes_are_encoded_whole_in_every_position() {
    let guest = flat(&generated(WIT, false).unwrap());
    let bytes = "::be_ops::wire::Bytes";
    let plain = "::be_ops::wire::Plain";
    let read = format!("::be_ops::wire::Fallible<{bytes},{plain}>");
    for expected in [
        format!(
            "::be_ops::guest::Pending<::core::result::Result<::std::vec::Vec<u8>,::std::string::String>,{read}>"
        ),
        format!(
            "::be_ops::Ticket<::core::result::Result<::std::vec::Vec<u8>,::std::string::String>,{read}>"
        ),
        format!("::be_ops::guest::Stream<::std::vec::Vec<u8>,{bytes}>"),
        format!(
            "::be_ops::Reply<::std::vec::Vec<::core::option::Option<::std::vec::Vec<u8>>>,::be_ops::wire::List<::be_ops::wire::Optional<{bytes}>>>"
        ),
        format!("::be_ops::wire::encode::<{read},_>"),
        format!(
            "serialize_with=\"::be_ops::wire::serialize::<::be_ops::wire::List<{bytes}>,_,_>\""
        ),
        format!(
            "serialize_with=\"::be_ops::wire::serialize::<::be_ops::wire::Optional<{bytes}>,_,_>\""
        ),
        format!("deserialize_with=\"::be_ops::wire::deserialize::<{bytes},_,_>\""),
        format!(
            "serialize_with=\"::be_ops::wire::serialize::<::be_ops::wire::List<({plain},{bytes},)>,_,_>\""
        ),
        format!("::be_ops::guest::Pending<u32,{plain}>"),
    ] {
        let expected = flat(&expected);
        assert!(guest.contains(&expected), "{expected} is not in {guest}");
    }
    assert!(!guest.contains("serde_bytes"), "{guest}");

    let network = flat(&generated(&WIT.replace("    export served;\n", ""), true).unwrap());
    for expected in [
        format!(
            "::be_ops::net::Pending<::core::result::Result<::std::vec::Vec<u8>,::std::string::String>,{read}>"
        ),
        format!("::be_ops::net::Stream<::std::vec::Vec<u8>,{bytes}>"),
        format!("::be_ops::Producer<::std::vec::Vec<u8>,{bytes}>"),
    ] {
        let expected = flat(&expected);
        assert!(
            network.contains(&expected),
            "{expected} is not in {network}"
        );
    }
}
