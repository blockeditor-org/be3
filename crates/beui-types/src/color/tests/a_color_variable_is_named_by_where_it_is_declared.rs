use super::*;

mod first {
    crate::color_var!(pub SURFACE);
}

mod second {
    crate::color_var!(pub SURFACE, pub TEXT);
}

#[test]
fn a_color_variable_is_named_by_where_it_is_declared() {
    assert_ne!(first::SURFACE, second::SURFACE);
    assert_ne!(first::SURFACE.css_name(), second::SURFACE.css_name());
    assert_ne!(second::SURFACE.css_name(), second::TEXT.css_name());
    assert_eq!(
        first::SURFACE,
        ColorVar::__declared_at(concat!(module_path!(), "::first"), "SURFACE")
    );
    assert_eq!(second::TEXT.css_name(), "v-15a037d4");
    assert_eq!(first::SURFACE.color().describe(), "var(SURFACE)");
}
