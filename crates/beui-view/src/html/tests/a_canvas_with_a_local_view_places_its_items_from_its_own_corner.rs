use super::*;

/// The HTML core's side of beui-host's test of the same name: a local view's origin is in
/// the canvas's own points, so its items sit where the view puts them from the canvas's
/// corner however far down and in its parent places it.
#[test]
fn a_canvas_with_a_local_view_places_its_items_from_its_own_corner() {
    let view = Some(CanvasView::new(pos2(10.0, 5.0), 2.0));
    let painting = painting([240, 200], || {
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START}>
                    <List spacing={0.0}>
                        {swatch(AMBER, 240.0, 60.0)}
                        <Frame padding={Sides::all(24.0)}>
                            <Frame
                                width={180.0}
                                height={90.0}
                                outline={Some(Outline::new(INK, 1.0))}
                            >
                                <Canvas view={view} local_view={true}>
                                    <CanvasItem
                                        x={3.0}
                                        y={4.0}
                                        width={40.0}
                                        height={20.0}
                                        scales={true}
                                    >
                                        <Frame fill={BLUE} />
                                    </CanvasItem>
                                    <CanvasItem x={50.0} y={10.0} width={30.0} height={20.0}>
                                        <Frame fill={GREEN} />
                                    </CanvasItem>
                                </Canvas>
                            </Frame>
                        </Frame>
                    </List>
                </Frame>
            </Root>
        }
    });
    painting.snapshot("html_canvas_local_view.browser");
}
