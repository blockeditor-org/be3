use std::time::Duration;

use beui::Vec2;
use block_plugin_api::{EditorCapabilities, EditorRegion, PluginManifest};

use crate::plugin_host::{InstanceStatus, RuntimeStatus, ScreenStatus};
use crate::ui::{Line, LineStyle, PluginsView, RuntimeView};

fn push(lines: &mut Vec<Line>, style: LineStyle, indent: u8, text: impl Into<String>) {
    lines.push(Line {
        text: text.into(),
        style,
        indent,
    });
}

pub(super) fn view() -> PluginsView {
    let mut lines = Vec::new();
    discovered(&mut lines);
    PluginsView {
        lines,
        runtimes: crate::plugin_host::running()
            .iter()
            .map(|runtime| RuntimeView {
                id: runtime.plugin_id.clone(),
                state: runtime.state.clone(),
                lines: runtime_lines(runtime),
            })
            .collect(),
    }
}

fn discovered(lines: &mut Vec<Line>) {
    let discovered = crate::editors::plugin::discovery::plugins();
    push(
        lines,
        LineStyle::Heading,
        0,
        format!("Discovered ({})", discovered.manifests().len()),
    );
    if discovered.manifests().is_empty() {
        push(lines, LineStyle::Muted, 1, "no plugins were found");
    }
    for manifest in discovered.manifests() {
        push(
            lines,
            LineStyle::Body,
            0,
            format!(
                "{} {} ({})",
                manifest.identity.id, manifest.identity.version, manifest.identity.name
            ),
        );
        manifest_lines(lines, manifest);
    }
    for error in discovered.errors() {
        push(lines, LineStyle::Error, 1, error.to_string());
    }
}

fn manifest_lines(lines: &mut Vec<Line>, manifest: &PluginManifest) {
    for editor in &manifest.editors {
        push(
            lines,
            LineStyle::Body,
            1,
            format!(
                "{} — {}",
                editor.display_name,
                uuid::Uuid::from_bytes(editor.block_type)
            ),
        );
        let mut small = |text: String| push(lines, LineStyle::Muted, 2, text);
        small(format!("regions {}", regions(&editor.regions)));
        small(format!(
            "interaction {:?} · resize {:?}",
            editor.interaction, editor.resize
        ));
        small(format!(
            "children add {} · delete {} · replace {}",
            editor.children.add, editor.children.delete, editor.children.replace
        ));
        small(format!(
            "capabilities {}",
            capabilities(&editor.capabilities)
        ));
        for template in &editor.templates {
            small(format!(
                "template {} ({}) · {:?}{}",
                template.id,
                template.name,
                template.category,
                if template.dialog { " · dialog" } else { "" }
            ));
        }
    }
    push(
        lines,
        LineStyle::Muted,
        1,
        format!("entry point {}", manifest.entry_point),
    );
}

fn runtime_lines(runtime: &RuntimeStatus) -> Vec<Line> {
    let mut lines = Vec::new();
    let surface = &runtime.surface;
    push(
        &mut lines,
        LineStyle::Muted,
        1,
        format!(
            "surface {} — {}x{} px, {}, {} placement(s), generation {}",
            surface.index,
            surface.width,
            surface.height,
            bytes(u64::from(surface.width) * u64::from(surface.height) * 4),
            surface.placements,
            surface.generation
        ),
    );
    push(
        &mut lines,
        LineStyle::Muted,
        1,
        format!(
            "pass {}{}",
            runtime.pass,
            runtime
                .uptime
                .map(|uptime| format!(" · up {}", elapsed(uptime)))
                .unwrap_or_default()
        ),
    );
    if runtime.instances.is_empty() {
        push(&mut lines, LineStyle::Muted, 1, "idle");
    }
    for instance in &runtime.instances {
        instance_lines(&mut lines, instance);
    }
    lines
}

fn instance_lines(lines: &mut Vec<Line>, instance: &InstanceStatus) {
    let block = instance
        .block
        .map_or_else(|| "creating a block".to_owned(), |id| id.to_string());
    push(
        lines,
        LineStyle::Body,
        1,
        format!(
            "{} — {} {block}{}",
            instance.instance.0,
            instance.role,
            if instance.opened { "" } else { ", opening" }
        ),
    );
    let mut details = Vec::new();
    if let Some(ratio) = instance.aspect_ratio {
        details.push(format!("aspect {ratio:.3}"));
    }
    if let Some(intrinsic) = instance.intrinsic {
        details.push(format!("intrinsic {}", size(intrinsic)));
    }
    if let Some(view) = instance.view {
        details.push(format!(
            "view {:.0},{:.0} {}",
            view.min.x,
            view.min.y,
            size(view.size())
        ));
    }
    if !details.is_empty() {
        push(lines, LineStyle::Muted, 2, details.join(" · "));
    }
    if let Some(artifact) = &instance.artifact {
        push(
            lines,
            LineStyle::Muted,
            2,
            format!(
                "artifact {}{} — {}",
                bytes(artifact.data as u64),
                artifact
                    .draft
                    .map(|draft| format!(", draft {}", bytes(draft as u64)))
                    .unwrap_or_default(),
                artifact.description.as_deref().unwrap_or("undescribed")
            ),
        );
    }
    if instance.screens.is_empty() {
        push(lines, LineStyle::Muted, 2, "no screens");
    }
    for screen in &instance.screens {
        screen_line(lines, screen);
    }
}

fn screen_line(lines: &mut Vec<Line>, screen: &ScreenStatus) {
    let text = format!(
        "{:?} #{} — {} pt @ {:.2} ({}x{} px){}{}{}{}",
        screen.region,
        screen.screen.0,
        size(screen.logical),
        screen.scale_factor,
        screen.pixels[0],
        screen.pixels[1],
        screen
            .used
            .map(|used| format!(" · used {} pt", size(used)))
            .unwrap_or_default(),
        screen
            .placement
            .map(|[x, y, width, height]| format!(" · at {x},{y} {width}x{height} px"))
            .unwrap_or_else(|| " · unplaced".to_owned()),
        if screen.drawn { "" } else { " · stale" },
        match screen.children {
            0 => String::new(),
            children => format!(
                " · {children} children at generation {}",
                screen.child_generation
            ),
        }
    );
    let style = match screen.drawn {
        true => LineStyle::Code,
        false => LineStyle::Muted,
    };
    push(lines, style, 2, text);
}

fn regions(regions: &[EditorRegion]) -> String {
    list(regions.iter().map(|region| format!("{region:?}")))
}

fn capabilities(capabilities: &EditorCapabilities) -> String {
    let named = [
        ("rotation", capabilities.rotation),
        ("preserve aspect ratio", capabilities.preserve_aspect_ratio),
        ("pan and zoom", capabilities.pan_and_zoom),
    ];
    list(
        named
            .iter()
            .filter(|(_, held)| *held)
            .map(|(name, _)| (*name).to_owned()),
    )
}

fn list(items: impl Iterator<Item = String>) -> String {
    let items: Vec<_> = items.collect();
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

fn size(size: Vec2) -> String {
    format!("{:.0}x{:.0}", size.x, size.y)
}

fn bytes(count: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut amount = count as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit + 1 < UNITS.len() {
        amount /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{count} B")
    } else {
        format!("{amount:.1} {}", UNITS[unit])
    }
}

fn elapsed(uptime: Duration) -> String {
    let seconds = uptime.as_secs();
    match (seconds / 3600, (seconds / 60) % 60, seconds % 60) {
        (0, 0, seconds) => format!("{seconds}s"),
        (0, minutes, seconds) => format!("{minutes}m {seconds:02}s"),
        (hours, minutes, _) => format!("{hours}h {minutes:02}m"),
    }
}
