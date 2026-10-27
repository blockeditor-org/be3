use wasm_bindgen::JsCast;

pub fn text_agent(document: &web_sys::Document) -> Option<web_sys::HtmlTextAreaElement> {
    let agent = document
        .create_element("textarea")
        .ok()?
        .dyn_into::<web_sys::HtmlTextAreaElement>()
        .ok()?;
    for (name, value) in [
        ("autocapitalize", "off"),
        ("autocomplete", "off"),
        ("autocorrect", "on"),
        ("spellcheck", "true"),
        ("aria-hidden", "true"),
    ] {
        agent.set_attribute(name, value).ok();
    }
    let style = agent.style();
    for (property, value) in [
        ("position", "absolute"),
        ("left", "0px"),
        ("top", "0px"),
        ("width", "1px"),
        ("height", "1px"),
        ("opacity", "0"),
        ("border", "none"),
        ("padding", "0"),
        ("resize", "none"),
        ("outline", "none"),
        ("background", "transparent"),
        ("color", "transparent"),
        ("caret-color", "transparent"),
        ("pointer-events", "none"),
        ("z-index", "-1"),
    ] {
        let _ = style.set_property(property, value);
    }
    Some(agent)
}
