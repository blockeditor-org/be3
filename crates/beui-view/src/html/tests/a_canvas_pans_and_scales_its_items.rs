use super::*;

#[test]
fn a_canvas_pans_and_scales_its_items() {
    painting([240, 160], || {
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START}>
                    <Frame width={240.0} height={160.0} outline={Some(Outline::new(INK, 1.0))}>
                        <Canvas view={Some(CanvasView::new(pos2(20.0, 10.0), 2.0))}>
                            <CanvasItem x={0.0} y={0.0} width={40.0} height={20.0} scales={true}>
                                <Frame fill={BLUE}>
                                    <Text color={PAPER}>"Scaled"</Text>
                                </Frame>
                            </CanvasItem>
                            <CanvasItem x={50.0} y={0.0} width={30.0} height={20.0}>
                                <Frame fill={GREEN}>
                                    <Text color={PAPER}>"Kept"</Text>
                                </Frame>
                            </CanvasItem>
                            <CanvasItem x={100.0} y={60.0} width={40.0} height={40.0}>
                                {swatch(AMBER, 80.0, 80.0)}
                            </CanvasItem>
                        </Canvas>
                    </Frame>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_canvas.browser");
}
