//! The panel of its host's statistics and the graph of its memory over time.

use super::*;

/// The host's statistics as its debug panels show them: a row a measurement, under headings,
/// its performance first and then its memory, scrolling.
#[component]
pub(super) fn StatisticsPanel(
    statistics: Signal<HostStatistics>,
    history: Signal<MemoryMinutes>,
) -> NodeId {
    let rows = create_memo(move || {
        statistics.with(|statistics| {
            let mut rows = statistics.performance.clone();
            rows.extend(statistics.memory.iter().cloned());
            rows
        })
    });
    let keys = create_memo(move || (0..rows.with(Vec::len)).collect::<Vec<_>>());
    let file = create_memo(move || {
        statistics.with(|statistics| {
            statistics
                .memory_file
                .as_ref()
                .map(|file| format!("Memory is written to {file}"))
                .unwrap_or_default()
        })
    });
    view! {
        <Scroll>
            <Frame padding=Space::Md>
                <List spacing=Space::Xs>
                    <Text style=TextStyle::TITLE>{STATISTICS_TITLE}</Text>
                    <MemoryGraph history={history} />
                    <StatisticLine
                        row={create_memo(|| Some(StatisticRow {
                            heading: true,
                            name: "Measurement".to_owned(),
                            current: "Latest".to_owned(),
                            average: "Average".to_owned(),
                            peak: "Peak".to_owned(),
                        }))}
                    />
                    <ForEach value={keys}>|index: usize| {
                        let row = create_memo(move || rows.with(|rows| rows.get(index).cloned()));
                        view! {
                            <StatisticLine row />
                        }
                    }</ForEach>
                    <Text size=TextSize::Xs color=TextColor::Muted>{file}</Text>
                </List>
            </Frame>
        </Scroll>
    }
}

/// The host's memory over time, as be-host's own panel graphs it: resident, anonymous,
/// shared and wasm memory a point a minute, each line in its own colour role, against a top of
/// the peak resident size rounded up to 64 MiB, a gap of more than two minutes breaking a line,
/// each line's name below it in its colour. Nothing shows until a minute is measured.
#[component]
pub(super) fn MemoryGraph(history: Signal<MemoryMinutes>) -> NodeId {
    let measured = create_memo(move || history.with(|history| !history.points.is_empty()));
    view! {
        <Show value={measured}>
            <MemoryDrawing history={history} />
        </Show>
    }
}

/// The graph itself, drawn each time the minutes or the theme change.
#[component]
pub(super) fn MemoryDrawing(history: Signal<MemoryMinutes>) -> NodeId {
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::dynamic(move || {
        let history = history.get();
        let colors = [
            theme.accent.get(),
            theme.success.get(),
            theme.warning.get(),
            theme.danger.get(),
        ];
        let border = theme.border.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            draw_memory(painter, rect, &history, colors, border);
        }) as Draw
    });
    view! {
        <List spacing=Space::Xs2>
            <Frame height={Size::Custom(MEMORY_GRAPH_HEIGHT)}>
                <Drawing draw />
            </Frame>
            <List direction=Direction::Horizontal spacing=Space::Md>
                <Text size=TextSize::Xs color=TextColor::Accent>"Resident"</Text>
                <Text size=TextSize::Xs color=TextColor::Success>"Anonymous"</Text>
                <Text size=TextSize::Xs color=TextColor::Warning>"Shared"</Text>
                <Text size=TextSize::Xs color=TextColor::Danger>"Wasm"</Text>
            </List>
        </List>
    }
}

/// The memory graph's height, in points.
pub(super) const MEMORY_GRAPH_HEIGHT: f32 = 60.0;

pub(super) fn draw_memory(
    painter: &Painter,
    rect: Rect,
    history: &MemoryMinutes,
    colors: [beui::Color; 4],
    border: beui::Color,
) {
    painter.rect_stroke(rect, 0.0, 1.0, border);
    let Some(first) = history.points.first() else {
        return;
    };
    const STEP: u64 = 64 << 20;
    let peak = history
        .points
        .iter()
        .map(|point| point.resident)
        .max()
        .unwrap_or(0);
    let top = peak.div_ceil(STEP).max(1) * STEP;
    let span = history.now.saturating_sub(first.at).max(60) as f32;
    let at = |point: &be_wasm_root_api::MemoryPoint, value: u64| {
        pos2(
            rect.left() + rect.width() * point.at.saturating_sub(first.at) as f32 / span,
            rect.bottom() - rect.height() * value as f32 / top as f32,
        )
    };
    let lines: [fn(&be_wasm_root_api::MemoryPoint) -> u64; 4] = [
        |point| point.resident,
        |point| point.anonymous,
        |point| point.shared,
        |point| point.wasm,
    ];
    for (line, color) in lines.into_iter().zip(colors).rev() {
        let mut path = beui::Path::new();
        let mut previous: Option<u64> = None;
        for point in &history.points {
            let spot = at(point, line(point));
            path = match previous {
                Some(before) if point.at.saturating_sub(before) <= 120 => path.line_to(spot),
                _ => path.move_to(spot),
            };
            previous = Some(point.at);
        }
        if let [only] = history.points.as_slice() {
            path = path.line_to(pos2(rect.right(), at(only, line(only)).y));
        }
        painter.stroke_path(
            &path,
            &beui::Stroke::new(1.5).join(beui::LineJoin::Round),
            color,
        );
    }
}

/// One row of the host's statistics: its name, and its latest value, average and peak, each
/// right-aligned in a column of its own.
#[component]
pub(super) fn StatisticLine(row: Memo<Option<StatisticRow>>) -> NodeId {
    let cell = |read: fn(&StatisticRow) -> String| {
        create_memo(move || row.with(|row| row.as_ref().map(read).unwrap_or_default()))
    };
    let name = cell(|row| row.name.clone());
    let current = cell(|row| row.current.clone());
    let average = cell(|row| row.average.clone());
    let peak = cell(|row| row.peak.clone());
    let weight =
        create_memo(
            move || match row.with(|row| row.as_ref().is_some_and(|row| row.heading)) {
                true => Weight::Bold,
                false => Weight::Normal,
            },
        );
    view! {
        <List direction=Direction::Horizontal spacing=Space::Sm>
            <ListItem grow=1.0>
                <Text overflow=Overflow::Ellipsis size=TextSize::Xs weight={weight}>{name}</Text>
            </ListItem>
            <StatisticCell value={current} weight={weight} />
            <StatisticCell value={average} weight={weight} />
            <StatisticCell value={peak} weight={weight} />
        </List>
    }
}

/// One value of a row, right-aligned in its column.
#[component]
pub(super) fn StatisticCell(value: Memo<String>, weight: Memo<Weight>) -> NodeId {
    view! {
        <Frame width=Size::Xl3>
            <Text size=TextSize::Xs family=Family::Mono weight={weight} align=TextAlign::End>
                {value}
            </Text>
        </Frame>
    }
}
