use be_portal::{PAGE_ID, Page, ROOT_ID};
use beui::reactive::*;

mod form;
mod screens;

#[cfg(target_arch = "wasm32")]
mod web;

pub use screens::PortalPage;

pub const PORTAL: be_portal::Portal = be_portal::Portal {
    render,
    import_map,
    assets: &[],
};

pub fn import_map(base: &str) -> String {
    format!(
        r#"{{ "imports": {{ "wasi_snapshot_preview1": "{base}wasi.js", "wasi": "{base}threads.js" }} }}"#
    )
}

pub fn markup(page: &Page) -> String {
    let page = page.clone();
    beui::html::render_to_string(move || {
        view! {
            <PortalPage page />
        }
    })
}

pub fn render(page: &Page) -> String {
    let markup = markup(page);
    let hydrated = !page.assets.is_empty();
    let fonts = if hydrated { page.assets.as_str() } else { "/" };
    let scripts = if hydrated {
        format!(
            "<script type=\"importmap\">{}</script>\n<script type=\"module\" src=\"{}start.js\"></script>\n<script type=\"application/json\" id=\"{PAGE_ID}\">{}</script>\n",
            import_map(&page.assets),
            page.assets,
            page.to_json(),
        )
    } else {
        String::new()
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"color-scheme\" content=\"light dark\">\n<title>{}</title>\n<style id=\"{}\">{}</style>\n{scripts}</head>\n<body><div id=\"{ROOT_ID}\">{markup}</div></body>\n</html>\n",
        escape(&screens::title(page)),
        beui::html::STYLES_ID,
        beui::html::stylesheet(fonts),
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests;
