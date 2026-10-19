use beui_html::{Children, Element, Node, Script, Stylesheet, attr, render, view};

pub fn main(args: Vec<String>) -> std::process::ExitCode {
    let demo = match args.first().map(String::as_str) {
        Some("demo") => true,
        Some("block") => false,
        _ => panic!("usage: web-page block|demo OUTPUT"),
    };
    let html = page(demo, None);
    std::fs::write(args.get(1).expect("output path"), html).expect("write HTML");
    std::process::ExitCode::SUCCESS
}

pub(crate) fn page(demo: bool, base: Option<String>) -> String {
    render(|| {
        let base: Children<Node> = match base {
            Some(href) => view! {
                <Element tag="base" @attr={("href", href)} />
            }
            .into(),
            None => view! {},
        };
        let (title, css, imports, js, content) = if demo {
            (
                "beui demo",
                include_str!("web_page/demo.css"),
                include_str!("web_page/demo.json"),
                include_str!("web_page/demo.js"),
                Children::<Node>::from(view! {
                    <Element tag="div" @attr={("id", "demo")} />
                }),
            )
        } else {
            (
                "Block",
                include_str!("web_page/block.css"),
                include_str!("web_page/block.json"),
                include_str!("web_page/block.js"),
                view! {
                    <Element tag="div" @attr={("id", "plugins")} />
                    <Element tag="canvas" @attr={("id", "block")} />
                },
            )
        };
        let status = if demo {
            view! {
                <Element tag="div" @attr={("id", "status")}>"Loading…"</Element>
            }
        } else {
            view! {
                <Element tag="div" @attr={("id", "status")}>
                    <Element tag="div" @attr={("class", "detail")}>"Loading Block…"</Element>
                </Element>
            }
        };
        view! {
            <Element tag="html" @attr={("lang", "en")}>
                <Element tag="head">
                    {base}
                    <Element tag="meta" @attr={("charset", "utf-8")} />
                    <Element
                        tag="meta"
                        @attr={("name", "viewport")}
                        @attr={("content", "width=device-width, initial-scale=1, user-scalable=no, interactive-widget=resizes-content")}
                    />
                    <Element tag="title">{title}</Element>
                    <Stylesheet css={css} />
                    <Script kind="importmap" source={imports} />
                </Element>
                <Element tag="body">
                    {content}
                    {status}
                    <Script kind="module" source={js} />
                </Element>
            </Element>
        }
    })
}
