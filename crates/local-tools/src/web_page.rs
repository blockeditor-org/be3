use beui_html::{Children, Node, Script, render, view};
use beui_html::{elements as html, tw};

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
        let (title, imports, js, content) = if demo {
            (
                "beui demo",
                include_str!("web_page/demo.json"),
                include_str!("web_page/demo.js"),
                Children::<Node>::from(view! {
                    <html::Div
                        id="demo"
                        css=<>
                            <tw::WFull />
                            <tw::HFull />
                        </>
                    />
                }),
            )
        } else {
            (
                "Block",
                include_str!("web_page/block.json"),
                include_str!("web_page/block.js"),
                view! {
                    <html::Div
                        id="plugins"
                        css=<>
                            <tw::Fixed />
                            <tw::Inset0 />
                            <tw::OverflowHidden />
                            <tw::PointerEventsNone />
                            <tw::Descendants target=tw::Target::Canvas>
                                <tw::Absolute />
                                <tw::Left0 />
                                <tw::Top0 />
                                <tw::OriginTopLeft />
                            </tw::Descendants>
                        </>
                    />
                    <html::Canvas
                        id="block"
                        css=<>
                            <tw::Relative />
                            <tw::Z1 />
                            <tw::Block />
                            <tw::WFull />
                            <tw::HFull />
                        </>
                    />
                },
            )
        };
        let status = if demo {
            view! {
                <html::Div
                    id="status"
                    css=<>
                        <StatusStyles />
                        <tw::WhitespacePreWrap />
                    </>
                >
                    "Loading…"
                </html::Div>
            }
        } else {
            view! {
                <html::Div
                    id="status"
                    css=<>
                        <StatusStyles />
                        <tw::FlexCol />
                        <tw::Gap3 />
                        <tw::TextCenter />
                    </>
                >
                    <html::Div
                        class="detail"
                        css=<>
                            <tw::MaxW160 />
                            <tw::TextSm />
                            <tw::TextMuted />
                            <tw::WhitespacePreWrap />
                        </>
                    >
                        "Loading Block…"
                    </html::Div>
                </html::Div>
            }
        };
        view! {
            <html::Html lang="en" css=<PageStyles />>
                <html::Head>
                    {base}
                    <html::Meta charset="utf-8" />
                    <html::Meta
                        name="viewport"
                        content="width=device-width, initial-scale=1, user-scalable=no, interactive-widget=resizes-content"
                    />
                    <html::Title>{title}</html::Title>
                    <tw::Styles />
                    <Script kind="importmap" source={imports} />
                </html::Head>
                <html::Body css=<PageStyles />>
                    {content}
                    {status}
                    <Script kind="module" source={js} />
                </html::Body>
            </html::Html>
        }
    })
}

#[beui_html::component]
fn PageStyles() -> Children<tw::Style> {
    view! {
        <tw::M0 />
        <tw::HFull />
        <tw::OverflowHidden />
        <tw::BgSurface />
        <tw::TextForeground />
        <tw::FontSans />
    }
}

#[beui_html::component]
fn StatusStyles() -> Children<tw::Style> {
    view! {
        <tw::Absolute />
        <tw::Inset0 />
        <tw::Flex />
        <tw::ItemsCenter />
        <tw::JustifyCenter />
        <tw::P4 />
        <tw::WhenHidden>
            <tw::Hidden />
        </tw::WhenHidden>
    }
}
