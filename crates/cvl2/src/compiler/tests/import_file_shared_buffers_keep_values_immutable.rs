use super::*;

#[test]
fn import_file_shared_buffers_keep_values_immutable() {
    let list = "    a := std.kw.list(std.kw.int): (1)
    b := a.push(2)
    c := a.push(3)
    -> std.kw.string.from_int(a.len) + \",\" + std.kw.string.from_int(b.get(1)) + \",\" + std.kw.string.from_int(c.get(1)) + \",\" + std.kw.string.from_int(c.len)";
    assert_eq!(build_file(list), Ok("1,2,3,2".to_string()));

    let string = "    a := std.kw.string: \"x\"
    b := a + \"y\"
    c := a + \"z\"
    -> a + \",\" + b + \",\" + c";
    assert_eq!(build_file(string), Ok("x,xy,xz".to_string()));
}
