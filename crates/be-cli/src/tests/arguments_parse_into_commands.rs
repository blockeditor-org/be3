use super::*;

#[test]
fn arguments_parse_into_commands() {
    let parse = |text: &str| {
        cli::parse(
            text.split(' ').map(str::to_owned).collect(),
            || Ok("from stdin".to_owned()),
        )
    };

    assert_eq!(
        parse("list Issues Status!=Done Title~login --json").unwrap(),
        Invocation {
            command: Command::List {
                database: "Issues".into(),
                filters: vec![
                    Filter {
                        field: "Status".into(),
                        condition: Condition::IsNot("Done".into()),
                    },
                    Filter {
                        field: "Title".into(),
                        condition: Condition::Contains("login".into()),
                    },
                ],
            },
            json: true,
        }
    );
    assert_eq!(
        parse("set BE-7K3Q2M Status=Done Project= --append -")
            .unwrap()
            .command,
        Command::Set {
            id: "BE-7K3Q2M".into(),
            sets: vec![
                ("Status".into(), "Done".into()),
                ("Project".into(), String::new()),
            ],
            body: Body::Append("from stdin".into()),
        }
    );
    assert!(parse("new Issues").is_err());
    assert!(parse("set BE-7K3Q2M Status").is_err());
    assert!(parse("frobnicate").is_err());
    assert_eq!(
        cli::edited_body(&cli::edit_document(&crate::tracker::RowView {
            id: "BE-1".into(),
            database: "Issues".into(),
            title: "Title".into(),
            fields: Vec::new(),
            page: None,
            body: Some("# Heading\nbody\n".into()),
        })),
        "# Heading\nbody\n"
    );
}
