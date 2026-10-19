use beui_html::{Element, Node, attr, render, script, stylesheet, view};

pub fn main(args: Vec<String>) -> std::process::ExitCode {
    let demo = match args.first().map(String::as_str) {
        Some("demo") => true,
        Some("block") => false,
        _ => panic!("usage: web-page block|demo OUTPUT"),
    };
    let html = render(|| {
        let (title, css, imports, js, content) = if demo {
            (
                "beui demo",
                include_str!("web_page/demo.css"),
                include_str!("web_page/demo.json"),
                include_str!("web_page/demo.js"),
                Node::element("div").attr("id", "demo"),
            )
        } else {
            (
                "Block",
                include_str!("web_page/block.css"),
                include_str!("web_page/block.json"),
                include_str!("web_page/block.js"),
                beui_html::fragment([
                    Node::element("div").attr("id", "plugins"),
                    Node::element("canvas").attr("id", "block"),
                ]),
            )
        };
        let status = if demo {
            Node::element("div").attr("id", "status").child("Loading…")
        } else {
            Node::element("div").attr("id", "status").child(
                Node::element("div")
                    .attr("class", "detail")
                    .child("Loading Block…"),
            )
        };
        view! {
            <Element tag="html" @attr={("lang", "en")}>
                <Element tag="head">
                    {Node::element("meta").attr("charset", "utf-8")}
                    {Node::element("meta").attr("name", "viewport").attr("content", "width=device-width, initial-scale=1, user-scalable=no, interactive-widget=resizes-content")}
                    <Element tag="title">{title}</Element>
                    {stylesheet(css)}
                    {script("importmap", imports)}
                </Element>
                <Element tag="body">{content}{status}{script("module", js)}</Element>
            </Element>
        }
    });
    std::fs::write(args.get(1).expect("output path"), html).expect("write HTML");
    std::process::ExitCode::SUCCESS
}
