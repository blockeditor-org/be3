use super::*;

fn build_shapes(body: &str) -> Result<String, Vec<TokenizationError>> {
    build_file(&format!(
        "{body}
  }}
]
Shape :: std.Enum[\"circle\" .= std.kw.int, \"square\" .= std.kw.int, \"dot\"]
describe :: (s: Shape) => std.kw.string: :return {{
  std.kw.match (s) [
    .circle .= (r) => {{ return: \"circle \" + std.kw.string.from_int(r) }}
    .square .= (side: std.kw.int) => {{ return: \"square \" + std.kw.string.from_int(side * side) }}
    .dot .= () => {{ return: \"dot\" }}
  ]
}}
round :: (s: Shape) => std.kw.string: :return {{
  std.kw.match (s) [
    .circle .= (_) => {{ return: \"round\" }}
    .else .= () => {{ return: \"not round\" }}
  ]
}}
unused :: [
  {{"
    ))
}

#[test]
fn import_file_kw_match() {
    let built = build_shapes(
        "    -> describe(Shape.circle(2)) + \", \" + describe(Shape.square(3)) + \", \" + describe(Shape.dot) + \", \" + round(Shape.dot) + \", \" + round(Shape.circle(1))",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(built, "circle 2, square 9, dot, not round, round");

    let known = build_shapes(
        "    -> std.kw.string: :return {\n      std.kw.match (Shape.dot) [\n        .circle .= (r) => { _ = not_defined }\n        .else .= () => { return: \"skipped the circle arm\" }\n      ]\n    }",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(known, "skipped the circle arm");

    let errors = [
        (
            "    std.kw.match (Shape.dot) [\n      .circle .= (r) => { }\n    ]\n    -> \"x\"",
            "std.kw.match is missing .square, .dot",
        ),
        (
            "    std.kw.match (Shape.dot) [\n      .round .= () => { }\n    ]\n    -> \"x\"",
            "Shape has no case .round",
        ),
        (
            "    std.kw.match (Shape.dot) [\n      .dot .= () => { }\n      .dot .= () => { }\n    ]\n    -> \"x\"",
            "std.kw.match has two .dot arms",
        ),
        (
            "    std.kw.match (Shape.dot) [\n      .else .= () => { }\n      .dot .= () => { }\n    ]\n    -> \"x\"",
            "the .else arm of std.kw.match comes last",
        ),
        (
            "    std.kw.match (std.kw.int: 1) [\n      .else .= () => { }\n    ]\n    -> \"x\"",
            "std.kw.match needs an enum, got KwInt",
        ),
    ];
    for (body, message) in errors {
        assert_eq!(only_error(build_shapes(body)), message, "{body}");
    }
}
