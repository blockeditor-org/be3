use super::*;

#[test]
fn a_grid_shares_its_columns_between_its_cells() {
    painting([320, 160], || {
        view! {
            <Root clear_color={PAPER}>
                <Grid
                    columns={vec![Track::Fixed(60.0), Track::Intrinsic, Track::Fraction(1.0)]}
                    column_spacing={8.0}
                    row_spacing={8.0}
                >
                    {swatch(BLUE, 30.0, 30.0)}
                    {swatch(GREEN, 90.0, 30.0)}
                    {swatch(AMBER, 30.0, 30.0)}
                    <GridCell span={2}>{swatch(INK, 40.0, 30.0)}</GridCell>
                    {swatch(BLUE, 30.0, 30.0)}
                </Grid>
            </Root>
        }
    })
    .snapshot("html_grid.browser");
}
