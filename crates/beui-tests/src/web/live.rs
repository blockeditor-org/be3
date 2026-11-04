use beui_html::elements as html;
use beui_html::{
    Element, Event, ForEach, Get, Node, RootScope, Script, Set, Show, Signal, Stylesheet, Text,
    attr, component, create_memo, create_signal, flush, render_fragment, view,
};
use wasm_bindgen::JsCast;

type Checked = Result<(), String>;

pub fn run() -> String {
    let checks: [(&str, fn() -> Checked); 8] = [
        (
            "mounting_keeps_nodes_while_bindings_update",
            mounting_keeps_nodes,
        ),
        (
            "hydrating_claims_every_server_node",
            hydrating_claims_every_node,
        ),
        (
            "hydrating_a_mismatch_builds_what_the_view_says",
            hydrating_a_mismatch,
        ),
        ("hydrating_splits_merged_text", hydrating_splits_merged_text),
        ("raw_text_elements_keep_their_text", raw_text_elements),
        ("a_handler_is_flushed_after_it_runs", handlers_are_flushed),
        ("hydrated_nodes_take_their_listeners", hydrated_listeners),
        ("hydrating_a_form_keeps_its_inputs", hydrating_a_form),
    ];
    let mut report = String::new();
    for (name, check) in checks {
        match check() {
            Ok(()) => report.push_str(&format!("ok {name}\n")),
            Err(why) => report.push_str(&format!("FAILED {name}: {why}\n")),
        }
    }
    report
}

fn host() -> web_sys::Element {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("checks run in a page");
    let host = document.create_element("div").expect("a div");
    document
        .body()
        .expect("the page has a body")
        .append_child(&host)
        .expect("the host joins the page");
    host
}

fn every_node(under: &web_sys::Node) -> Vec<web_sys::Node> {
    let mut found = Vec::new();
    let mut child = under.first_child();
    while let Some(node) = child {
        found.push(node.clone());
        found.extend(every_node(&node));
        child = node.next_sibling();
    }
    found
}

fn expect(holds: bool, why: impl FnOnce() -> String) -> Checked {
    if holds { Ok(()) } else { Err(why()) }
}

fn same(a: &web_sys::Node, b: &web_sys::Node) -> bool {
    a.is_same_node(Some(b))
}

fn first_element(host: &web_sys::Element) -> Result<web_sys::Element, String> {
    host.first_element_child()
        .ok_or_else(|| format!("the host holds no element: {}", host.inner_html()))
}

fn items(list: &web_sys::Element) -> Vec<web_sys::Element> {
    let mut found = Vec::new();
    let mut child = list.first_element_child();
    while let Some(element) = child {
        child = element.next_element_sibling();
        found.push(element);
    }
    found
}

#[component]
fn Listed(title: Signal<String>, rows: Signal<Vec<u32>>) -> Node {
    view! {
        <Element tag="main" @attr={("title", title)}>
            <Text value={title} />
            <html::Ul>
                <ForEach keys={rows}>|row: u32|
                    <html::Li>{row.to_string()}</html::Li>
                </ForEach>
            </html::Ul>
        </Element>
    }
}

fn mounting_keeps_nodes() -> Checked {
    let _root = RootScope::new();
    let title = create_signal("first".to_owned());
    let rows = create_signal(vec![1_u32, 2, 3]);
    let host = host();
    let _mounted = beui_html::dom::mount(&host, move || {
        view! {
            <Listed title={title} rows={rows} />
        }
    });
    let main = first_element(&host)?;
    let text = main
        .first_child()
        .ok_or_else(|| "main holds nothing".to_owned())?;
    let list = main
        .last_element_child()
        .ok_or_else(|| "main holds no list".to_owned())?;
    let before = items(&list);
    title.set("second".to_owned());
    rows.set(vec![3, 1, 2, 4]);
    flush();
    let now = first_element(&host)?;
    expect(same(&now, &main), || {
        "the main element was replaced".to_owned()
    })?;
    expect(
        main.get_attribute("title").as_deref() == Some("second"),
        || format!("the title is {:?}", main.get_attribute("title")),
    )?;
    expect(
        main.first_child().is_some_and(|now| same(&now, &text))
            && text.text_content().as_deref() == Some("second"),
        || format!("the text node changed or reads {:?}", text.text_content()),
    )?;
    let after = items(&list);
    let texts: Vec<String> = after
        .iter()
        .map(|item| item.text_content().unwrap_or_default())
        .collect();
    expect(texts == ["3", "1", "2", "4"], || {
        format!("the rows read {texts:?}")
    })?;
    expect(
        same(&after[0], &before[2]) && same(&after[1], &before[0]) && same(&after[2], &before[1]),
        || "a row that moved was rebuilt rather than moved".to_owned(),
    )
}

#[component]
fn Sectioned(title: Signal<String>, rows: Signal<Vec<u32>>, shown: Signal<bool>) -> Node {
    view! {
        <Element tag="section">
            <Listed title={title} rows={rows} />
            <Show condition={shown}>
                <html::P>"shown"</html::P>
            </Show>
        </Element>
    }
}

fn hydrating_claims_every_node() -> Checked {
    let _root = RootScope::new();
    let title = create_signal("server".to_owned());
    let rows = create_signal(vec![1_u32, 2]);
    let shown = create_signal(true);
    let host = host();
    host.set_inner_html(&render_fragment(move || {
        view! {
            <Sectioned title={title} rows={rows} shown={shown} />
        }
    }));
    let before = every_node(&host);
    let _mounted = beui_html::dom::hydrate(&host, move || {
        view! {
            <Sectioned title={title} rows={rows} shown={shown} />
        }
    });
    let after = every_node(&host);
    expect(
        before.len() == after.len() && before.iter().zip(&after).all(|(a, b)| same(a, b)),
        || {
            format!(
                "hydration replaced nodes: {} before, {} after: {}",
                before.len(),
                after.len(),
                host.inner_html()
            )
        },
    )?;
    title.set("client".to_owned());
    rows.set(vec![2, 1]);
    shown.set(false);
    flush();
    let html = host.inner_html();
    expect(
        html == "<section><main title=\"client\">client<ul><!--[--><li>2</li><li>1</li><!--]--></ul></main><!--[--><!--]--></section>",
        || format!("the hydrated page did not follow its signals: {html}"),
    )
}

fn hydrating_a_mismatch() -> Checked {
    let _root = RootScope::new();
    let title = create_signal("right".to_owned());
    let rows = create_signal(vec![1_u32]);
    let host = host();
    host.set_inner_html("<section>stale</section><p>extra</p>");
    let _mounted = beui_html::dom::hydrate(&host, move || {
        view! {
            <Listed title={title} rows={rows} />
        }
    });
    let wanted = render_fragment(move || {
        view! {
            <Listed title={title} rows={rows} />
        }
    });
    let html = host.inner_html();
    expect(html == wanted, || {
        format!("after a mismatch the page holds {html}, not {wanted}")
    })?;
    rows.set(vec![1, 2]);
    flush();
    let html = host.inner_html();
    expect(html.contains("<li>1</li><li>2</li>"), || {
        format!("the rebuilt part does not follow its signals: {html}")
    })
}

#[component]
fn Greeting(name: Signal<String>) -> Node {
    view! {
        <html::P>
            "Hello, "
            <Text value={name} />
        </html::P>
    }
}

fn hydrating_splits_merged_text() -> Checked {
    let _root = RootScope::new();
    let name = create_signal("world".to_owned());
    let host = host();
    host.set_inner_html(&render_fragment(move || {
        view! {
            <Greeting name={name} />
        }
    }));
    let paragraph = first_element(&host)?;
    let merged = paragraph
        .first_child()
        .ok_or_else(|| "the paragraph is empty".to_owned())?;
    expect(paragraph.child_nodes().length() == 1, || {
        format!(
            "the server's text was not one node: {}",
            paragraph.child_nodes().length()
        )
    })?;
    let _mounted = beui_html::dom::hydrate(&host, move || {
        view! {
            <Greeting name={name} />
        }
    });
    expect(paragraph.child_nodes().length() == 2, || {
        format!(
            "hydration left {} text nodes, not 2",
            paragraph.child_nodes().length()
        )
    })?;
    expect(
        paragraph
            .first_child()
            .is_some_and(|first| same(&first, &merged)),
        || "the server's text node was replaced".to_owned(),
    )?;
    name.set("there".to_owned());
    flush();
    let text = paragraph.text_content().unwrap_or_default();
    expect(text == "Hello, there", || {
        format!("the paragraph reads {text:?}")
    })
}

const CSS: &str = "p > b { color: red }";
const DATA: &str = "{\"a\": [1, 2]}";

#[component]
fn RawText() -> Node {
    view! {
        <Element tag="div">
            <Stylesheet css={CSS.to_owned()} />
            <Script kind="application/json" source={DATA.to_owned()} />
        </Element>
    }
}

fn raw_text_elements() -> Checked {
    let _root = RootScope::new();
    let host = host();
    host.set_inner_html(&render_fragment(|| {
        view! {
            <RawText />
        }
    }));
    let style = first_element(&first_element(&host)?)?;
    let before = every_node(&host);
    let _hydrated = beui_html::dom::hydrate(&host, || {
        view! {
            <RawText />
        }
    });
    let after = every_node(&host);
    expect(
        before.len() == after.len() && before.iter().zip(&after).all(|(a, b)| same(a, b)),
        || format!("hydration replaced raw text: {}", host.inner_html()),
    )?;
    let script = style
        .next_element_sibling()
        .ok_or_else(|| "no script".to_owned())?;
    expect(
        style.text_content().as_deref() == Some(CSS)
            && script.text_content().as_deref() == Some(DATA),
        || format!("the raw text reads {}", host.inner_html()),
    )?;
    let mounted_host = self::host();
    let _mounted = beui_html::dom::mount(&mounted_host, || {
        view! {
            <RawText />
        }
    });
    let html = mounted_host.inner_html();
    expect(html == host.inner_html(), || {
        format!(
            "mounting built {html}, hydrating kept {}",
            host.inner_html()
        )
    })
}

#[component]
fn Counter(count: Signal<u32>) -> Node {
    let shown = create_memo(move || count.get().to_string());
    view! {
        <html::Button on_click={move |_: Event| count.set(count.get_untracked() + 1)}>
            <Text value={shown} />
        </html::Button>
    }
}

fn click_first(host: &web_sys::Element) -> Result<web_sys::HtmlElement, String> {
    let button: web_sys::HtmlElement = first_element(host)?
        .dyn_into()
        .map_err(|_| "the button is no HTML element".to_owned())?;
    button.click();
    Ok(button)
}

fn handlers_are_flushed() -> Checked {
    let _root = RootScope::new();
    let count = create_signal(0_u32);
    let host = host();
    let _mounted = beui_html::dom::mount(&host, move || {
        view! {
            <Counter count={count} />
        }
    });
    let button = click_first(&host)?;
    let text = button.text_content().unwrap_or_default();
    expect(text == "1", || {
        format!("right after the click the button reads {text:?}, not 1")
    })
}

fn hydrated_listeners() -> Checked {
    let _root = RootScope::new();
    let count = create_signal(0_u32);
    let host = host();
    host.set_inner_html(&render_fragment(move || {
        view! {
            <Counter count={count} />
        }
    }));
    let _mounted = beui_html::dom::hydrate(&host, move || {
        view! {
            <Counter count={count} />
        }
    });
    let button = click_first(&host)?;
    button.click();
    let text = button.text_content().unwrap_or_default();
    expect(text == "2", || {
        format!("after two clicks the hydrated button reads {text:?}, not 2")
    })
}

fn sign_in() -> beui::primitives::Root {
    use beui::reactive::{Get, create_memo as memo, create_signal as signal, view as beui_view};
    use beui::styled::*;
    let email = signal("ada@example.com".to_owned());
    let password = signal(String::new());
    let report = memo(move || format!("Password {}", password.get()));
    beui_view! {
        <Root>
            <Form action="/auth/login" hidden={vec![("csrf", "token".to_owned())]}>
                <List spacing=Space::Md>
                    <TextInput value={email} label="Email" kind=InputKind::Email name="email" />
                    <TextInput
                        value={password}
                        label="Password"
                        kind=InputKind::Password
                        name="password"
                    />
                    <Button label="Sign in" submit=true />
                    <Text>{report}</Text>
                </List>
            </Form>
        </Root>
    }
}

fn inputs(host: &web_sys::Element) -> Vec<web_sys::HtmlInputElement> {
    let found = host.query_selector_all("input").expect("a valid selector");
    (0..found.length())
        .filter_map(|index| found.item(index))
        .filter_map(|node| node.dyn_into().ok())
        .collect()
}

fn hydrating_a_form() -> Checked {
    let host = host();
    host.set_id("hydrated-form");
    host.set_inner_html(&beui::html::render_to_string(sign_in));
    let before = inputs(&host);
    expect(before.len() == 3, || {
        format!("the server page holds {} inputs, not 3", before.len())
    })?;
    before[2].set_value("typed before hydration");
    beui::html::hydrate("hydrated-form", sign_in)?;
    let after = inputs(&host);
    expect(
        after.len() == 3 && before.iter().zip(&after).all(|(a, b)| same(a, b)),
        || "hydration replaced the server's inputs".to_owned(),
    )?;
    let password = &after[2];
    expect(password.type_() == "password", || {
        format!("the password field is a {:?} input", password.type_())
    })?;
    expect(password.value() == "typed before hydration", || {
        format!("hydration overwrote what was typed: {:?}", password.value())
    })?;
    password.set_value("hunter2");
    let event = web_sys::Event::new("input").map_err(|_| "an input event".to_owned())?;
    password
        .dispatch_event(&event)
        .map_err(|_| "dispatching input".to_owned())?;
    let text = host.text_content().unwrap_or_default();
    expect(text.contains("Password hunter2"), || {
        format!("typing did not reach the signal: {text:?}")
    })
}
