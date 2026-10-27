use super::*;

#[test]
fn a_list_lays_out_its_children_in_a_line_by_their_sizing() {
    painting([320, 260], || {
        view! {
            <Root clear_color={PAPER}>
                <List spacing={10.0}>
                    <List direction={Direction::Horizontal} spacing={8.0}>
                        {swatch(BLUE, 40.0, 30.0)}
                        <Spacer @sizing={ItemSize::Percent(1.0)} />
                        {swatch(GREEN, 40.0, 30.0)}
                    </List>
                    <List direction={Direction::Horizontal} spacing={8.0}>
                        <Frame @sizing={ItemSize::Percent(1.0)} height={30.0} fill={BLUE} />
                        <Frame @sizing={ItemSize::Percent(2.0)} height={30.0} fill={GREEN} />
                        <Frame @sizing={ItemSize::Fixed(50.0)} height={30.0} fill={AMBER} />
                    </List>
                    <List
                        direction={Direction::Horizontal}
                        justify={Justify::SpaceBetween}
                        align={Align::End}
                        spacing={0.0}
                    >
                        {swatch(INK, 30.0, 50.0)}
                        {swatch(BLUE, 30.0, 20.0)}
                        {swatch(GREEN, 30.0, 35.0)}
                    </List>
                    <List direction={Direction::Horizontal} wrap={true} spacing={6.0}>
                        {swatch(AMBER, 90.0, 20.0)}
                        {swatch(BLUE, 90.0, 20.0)}
                        {swatch(GREEN, 90.0, 20.0)}
                        {swatch(INK, 90.0, 20.0)}
                    </List>
                </List>
            </Root>
        }
    })
    .snapshot("html_list.browser");
}
