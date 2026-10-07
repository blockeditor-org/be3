use super::*;

#[test]
fn import_file_kw_for() {
    let built = build_file(
        "    out := std.kw.mut(std.kw.string).new: \"\"
    std.kw.for (w := std.kw.list(std.kw.string): (\"a\", \"b\", \"c\")) {
      out += w
    }
    total := std.kw.mut(std.kw.int).new: 0
    :break std.kw.for (i := std.kw.range(0, 100)): :continue {
      std.kw.if (i % 2 == 0) {
        continue:
      }
      std.kw.if (i > 9) {
        break:
      }
      total += i
    }
    m := std.kw.map(std.kw.string, std.kw.int): [\"x\" .= 1, \"y\" .= 2]
    std.kw.for ((k, v) := m) {
      out += k + std.kw.string.from_int(v)
    }
    std.kw.for (_ := std.kw.range(0, 3)) {
      out += \"!\"
    }
    -> out.* + \" \" + std.kw.string.from_int(total.*)",
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(built, "abcx1y2!!! 25");

    assert_eq!(
        only_error(build_file(
            "    std.kw.for (x := std.kw.int: 3) { }\n    -> \"x\""
        )),
        "std.kw.for needs a std.kw.list or std.kw.map, got KwInt"
    );
    assert_eq!(
        only_error(build_file(
            "    std.kw.for (x := std.kw.map(std.kw.string, std.kw.int): []) { }\n    -> \"x\""
        )),
        "std.kw.for takes (item := list) or ((key, value) := map)"
    );
}
