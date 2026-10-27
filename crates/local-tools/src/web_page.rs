use beui_html::elements as html;
use beui_html::{Children, Node, Script, Stylesheet, render, view};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Block,
    Demo,
    PaintReview,
}

pub fn main(args: Vec<String>) -> std::process::ExitCode {
    let kind = match args.first().map(String::as_str) {
        Some("block") => Page::Block,
        Some("demo") => Page::Demo,
        Some("paint-review") => Page::PaintReview,
        _ => panic!("usage: web-page block|demo|paint-review OUTPUT"),
    };
    let html = page(kind, None);
    std::fs::write(args.get(1).expect("output path"), html).expect("write HTML");
    std::process::ExitCode::SUCCESS
}

const STYLES: &str = "
html, body {
    margin: 0;
    height: 100%;
    overflow: hidden;
    background: #1b1b1b;
    color: #d0d0d0;
    font-family: system-ui, sans-serif;
}
#demo, #review {
    width: 100%;
    height: 100%;
}
#plugins {
    position: fixed;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
}
#plugins canvas {
    position: absolute;
    left: 0;
    top: 0;
    transform-origin: 0 0;
}
#block {
    position: relative;
    z-index: 1;
    display: block;
    width: 100%;
    height: 100%;
}
#status {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1rem;
}
#status[hidden] {
    display: none;
}
.detail {
    max-width: 40rem;
    font-size: .875rem;
    color: #9a9a9a;
    white-space: pre-wrap;
}
";

pub(crate) fn page(kind: Page, base: Option<String>) -> String {
    render(|| {
        let base: Children<Node> = match base {
            Some(href) => view! {
                <html::Base href={href} />
            }
            .into(),
            None => view! {},
        };
        let (title, imports, js, content) = match kind {
            Page::Demo => (
                "beui demo",
                include_str!("web_page/demo.json"),
                include_str!("web_page/demo.js"),
                Children::<Node>::from(view! {
                    <html::Div id="demo" />
                }),
            ),
            Page::PaintReview => (
                "Paintings",
                include_str!("web_page/paint_review.json"),
                include_str!("web_page/paint_review.js"),
                Children::<Node>::from(view! {
                    <html::Div id="review" />
                }),
            ),
            Page::Block => (
                "Block",
                include_str!("web_page/block.json"),
                include_str!("web_page/block.js"),
                view! {
                    <html::Div id="plugins" />
                    <html::Canvas id="block" />
                },
            ),
        };
        let status = if kind != Page::Block {
            view! {
                <html::Div id="status" style="white-space: pre-wrap">"Loading…"</html::Div>
            }
        } else {
            view! {
                <html::Div
                    id="status"
                    style="flex-direction: column; gap: .75rem; text-align: center"
                >
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
                    <Stylesheet css={STYLES.to_string()} />
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
