use super::*;
use crate::tracker::Change;

#[test]
fn arguments_parse_into_commands() {
    let parse = |words: &[&str]| {
        cli::parse(
            words.iter().map(|word| (*word).to_owned()).collect(),
            || Ok("from stdin".to_owned()),
        )
    };

    let listed = parse(&["Issues", "rows", "Status!=Done", "--json"]).unwrap();
    assert_eq!(listed.target.as_deref(), Some("Issues"));
    assert_eq!(listed.verb, "rows");
    assert_eq!(listed.arguments, ["Status!=Done"]);
    assert!(listed.json);

    let changed = parse(&[
        "BE-7K3Q2M",
        "set",
        "Status=Done",
        "Blocked by+=BE-ABCDEF",
        "--append",
        "-",
    ])
    .unwrap();
    assert_eq!(
        changed.options.body,
        Some(Body::Append("from stdin".into()))
    );
    let assignments: Vec<Assignment> = changed
        .arguments
        .iter()
        .map(|text| Assignment::parse(text).unwrap())
        .collect();
    assert_eq!(assignments[1].field, "Blocked by");
    assert_eq!(assignments[1].change, Change::Add);
    assert_eq!(assignments[1].value, "BE-ABCDEF");
    assert_eq!(Assignment::parse("Project=").unwrap().value, "");
    assert_eq!(
        Assignment::parse("Blocked by-=X").unwrap().change,
        Change::Remove
    );
    assert!(Assignment::parse("Status").is_err());

    let listing = parse(&["ls"]).unwrap();
    assert_eq!((listing.target, listing.verb.as_str()), (None, "ls"));
    let page = parse(&["BE-7K3Q2M", "page", "write", "-"]).unwrap();
    assert_eq!(page.arguments, ["write", "from stdin"]);
    let made = parse(&["new-database", "Issues", "--prefix", "BE", "--in", "a1f3"]).unwrap();
    assert_eq!(made.options.prefix.as_deref(), Some("BE"));
    assert_eq!(made.options.within.as_deref(), Some("a1f3"));
    let column = parse(&[
        "Issues",
        "add-column",
        "Blocked by",
        "relation",
        "--links",
        "Issues",
        "--multiple",
        "--inverse",
        "Blocking",
    ])
    .unwrap();
    assert!(column.options.column.multiple);
    assert_eq!(column.options.column.inverse, "Blocking");
    assert!(parse(&["Issues", "rows", "--frobnicate"]).is_err());

    assert_eq!(
        cli::edited_body(&cli::edit_document("BE-1", "# Heading\nbody\n")),
        "# Heading\nbody\n"
    );
}
