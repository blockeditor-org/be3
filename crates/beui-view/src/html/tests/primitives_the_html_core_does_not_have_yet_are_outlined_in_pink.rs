use super::*;

#[test]
fn primitives_the_html_core_does_not_have_yet_are_outlined_in_pink() {
    painting([320, 260], || {
        view! {
            <Root clear_color={PAPER}>
                <List spacing={12.0} align={Align::Start}>
                    <Canvas width={200.0} height={60.0}>
                        <CanvasItem x={10.0} y={10.0} width={40.0} height={40.0}>
                            {swatch(BLUE, 40.0, 40.0)}
                        </CanvasItem>
                        <CanvasItem x={60.0} y={20.0} width={40.0} height={30.0}>
                            {swatch(GREEN, 40.0, 30.0)}
                        </CanvasItem>
                    </Canvas>
                    <VirtualList keys={vec![1, 2, 3]} item_size={20.0}>|key: ReadSignal<i32>|
                        <Text color={INK}>{format!("Row {}", key.get_untracked())}</Text>
                    </VirtualList>
                    <Portal />
                    <Overlay anchor={OverlayAnchor::default()} open={true}>
                        <Frame width={60.0} height={20.0} fill={AMBER} />
                    </Overlay>
                </List>
            </Root>
        }
    })
    .snapshot("html_stand_ins.browser");
}
