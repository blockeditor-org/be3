use super::*;
use beui_macros::view;

#[test]
fn styled_list_rows_on_the_html_core() {
    painting([320, 160], || {
        view! {
            <Root>
                <Frame padding={Padding::all(Space::Md)}>
                    <List spacing=Space::None>
                        <ListRow label="Inbox" detail="12 unread" />
                        <ListRow label="Archive" selected=true />
                        <ListRow label="Trash" disabled=true />
                    </List>
                </Frame>
            </Root>
        }
    })
    .snapshot("html_styled_list_rows.browser");
}
