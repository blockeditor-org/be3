use beui_html::elements as html;
use beui_html::{Children, Node, Script, Stylesheet, render, view};

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
                <html::Base href={href} />
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
                    <html::Div id="demo" />
                }),
            )
        } else {
            (
                "Block",
                include_str!("web_page/block.css"),
                include_str!("web_page/block.json"),
                include_str!("web_page/block.js"),
                view! {
                    <html::Div id="plugins" />
                    <html::Canvas id="block" />
                },
            )
        };
        let status = if demo {
            view! {
                <html::Div id="status">"Loading…"</html::Div>
            }
        } else {
            view! {
                <html::Div id="status">
                    <html::Div class="detail">"Loading Block…"</html::Div>
                </html::Div>
            }
        };
        view! {
            <html::Html lang="en">
                <html::Head>
                    {base}
                    <html::Meta charset="utf-8" />
                    <html::Meta
                        name="viewport"
                        content="width=device-width, initial-scale=1, user-scalable=no, interactive-widget=resizes-content"
                    />
                    <html::Title>{title}</html::Title>
                    <Stylesheet css={css} />
                    <Script kind="importmap" source={imports} />
                </html::Head>
                <html::Body>
                    {content}
                    {status}
                    <Script kind="module" source={js} />
                </html::Body>
            </html::Html>
        }
    })
}
