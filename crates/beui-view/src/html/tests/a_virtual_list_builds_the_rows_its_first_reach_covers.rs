use super::*;

#[test]
fn a_virtual_list_builds_the_rows_its_first_reach_covers() {
    painting([200, 160], || {
        let keys: Vec<i32> = (0..500).collect();
        view! {
            <Root clear_color={PAPER}>
                <Frame align={Align2::START}>
                    <Frame height={140.0} width={160.0} outline={Some(Outline::new(INK, 1.0))}>
                        <Offset offset={0.0}>
                            <VirtualList keys={keys} item_size={24.0}>|key: ReadSignal<i32>|
                                <Frame
                                    height={24.0}
                                    fill={if key.get_untracked() % 2 == 0 { PAPER } else { Color32::from_rgb(230, 232, 240) }}
                                >
                                    <Text color={INK}>
                                        {format!("Row {}", key.get_untracked())}
                                    </Text>
                                </Frame>
                            </VirtualList>
                        </Offset>
                    </Frame>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_virtual_list.browser");
}
