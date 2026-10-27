use be_portal::{Field as Named, Page, Screen, Session, SessionKind, with_code};
use beui::icons::{
    Glyph, ICON_BLOCKS, ICON_CIRCLE_ALERT, ICON_CIRCLE_CHECK, ICON_CIRCLE_X, ICON_GLOBE,
    ICON_KEY_ROUND, ICON_LAPTOP, ICON_MAIL, ICON_MAIL_CHECK, ICON_SEARCH_X, ICON_SHIELD_CHECK,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Align2, Direction, Extent, ItemSize, Justify, NodeId, TextAlign};

use crate::form::{Field, PageLink, PortalForm, Submit, fixed};

const PRODUCT: &str = "Block";

pub fn title(page: &Page) -> String {
    let name = match &page.screen {
        Screen::SignIn | Screen::SignInClosed => "Sign in",
        Screen::Register | Screen::RegistrationClosed => "Create an account",
        Screen::Forgot => "Reset your password",
        Screen::Reset => "Choose a new password",
        Screen::Verify => "Verify your email address",
        Screen::Approve => "Approve sign-in",
        Screen::Account => "Your account",
        Screen::Settings => "Account settings",
        Screen::Sessions => "Signed-in sessions",
        Screen::Approved => "Sign-in approved",
        Screen::Denied => "Sign-in denied",
        Screen::CheckEmail | Screen::ResetRequested => "Check your email",
        Screen::PasswordReset => "Password reset",
        Screen::EmailUpdated => "Email verified",
        Screen::NotFound => "Page not found",
        Screen::Message { title, .. } => title.as_str(),
    };
    format!("{name} · {PRODUCT} account")
}

#[derive(Clone, Copy)]
struct Shown(Stored<Page>);

impl Shown {
    fn read<R>(self, read: impl FnOnce(&Page) -> R) -> R {
        self.0.with(read)
    }

    fn csrf(self) -> String {
        self.read(|page| page.csrf.clone())
    }

    fn code(self) -> String {
        self.read(|page| page.code.clone())
    }

    fn code_problem(self) -> String {
        self.read(|page| page.code_problem.clone())
    }

    fn general(self) -> String {
        self.read(Page::general_problem)
    }

    fn problem(self, field: Named) -> String {
        self.read(|page| page.problem_for(field))
    }

    fn token(self) -> Vec<(&'static str, String)> {
        vec![("token", self.read(|page| page.mail_token.clone()))]
    }

    fn here(self, path: &str) -> String {
        self.read(|page| match page.code.is_empty() {
            true => path.to_owned(),
            false => with_code(path, &page.code),
        })
    }
}

fn column() -> Extent {
    Extent::at_most(Size::Dialog.px() + 2.0 * Space::Xl.px())
}

#[component]
pub fn PortalPage(page: Page) -> Root {
    let heading = title(&page);
    let page = Shown(store(page));
    view! {
        <Root title={heading}>
            <Scroll>
                <List spacing=Space::None align=Align::Center>
                    <Frame width={column()} padding={Padding::symmetric(Space::Lg, Space::Xl3)}>
                        <List spacing=Space::Xl>
                            <Brand page />
                            <Body page />
                            <Footer page />
                        </List>
                    </Frame>
                </List>
            </Scroll>
        </Root>
    }
}

#[component]
fn Brand(page: Shown) -> NodeId {
    let elsewhere = page.read(|page| page.account.is_some() && page.screen != Screen::Account);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
            <Text size=TextSize::Xl2 color=TextColor::Accent>
                <Icon glyph=ICON_BLOCKS />
            </Text>
            <Text size=TextSize::Lg weight=Weight::Bold @sizing=ItemSize::Percent(100.0)>
                {PRODUCT}
                <Span color=TextColor::Muted>" account"</Span>
            </Text>
            <Show condition={elsewhere}>
                <PageLink label="Your account" href="/auth" />
            </Show>
        </List>
    }
}

#[component]
fn Footer(page: Shown) -> NodeId {
    let account = page.read(|page| page.account.clone());
    let signed_in = account.is_some();
    let who = fixed(
        account
            .map(|account| format!("Signed in as {} ({})", account.display_name, account.email))
            .unwrap_or_default(),
    );
    let shows_sign_out = signed_in && page.read(|page| page.screen != Screen::Approve);
    view! {
        <List spacing=Space::Md align=Align::Center>
            <Show condition={signed_in}>
                <Text style=TextStyle::CAPTION align=TextAlign::Center>{who}</Text>
            </Show>
            <Show condition={shows_sign_out}>
                <PortalForm action="/auth/logout" csrf={page.csrf()} code={page.code()}>
                    <Submit label="Sign out" variant=ButtonVariant::Ghost wide=false />
                </PortalForm>
            </Show>
        </List>
    }
}

#[component]
fn Heading(
    title: String,
    #[prop(default = String::new())] lead: String,
    #[prop(default = None)] glyph: Option<Glyph>,
    #[prop(default = TextColor::Accent)] tone: TextColor,
) -> NodeId {
    let has_lead = !lead.is_empty();
    let has_glyph = glyph.is_some();
    let title = fixed(title);
    let lead = fixed(lead);
    view! {
        <List spacing=Space::Sm>
            <Show condition={has_glyph}>
                <Text size=TextSize::Xl3 color={tone}>
                    <Icon glyph={glyph} />
                </Text>
            </Show>
            <Text style=TextStyle::TITLE weight=Weight::Bold>{title}</Text>
            <Show condition={has_lead}>
                <Text style=TextStyle::PARAGRAPH>{lead}</Text>
            </Show>
        </List>
    }
}

#[component]
fn Notice(text: String, #[prop(default = false)] good: bool) -> DynamicSegment<ListChild> {
    let shown = !text.is_empty();
    let text = fixed(text);
    let (fill, color, glyph) = match good {
        true => (ColorRole::AccentSoft, TextColor::Success, ICON_CIRCLE_CHECK),
        false => (ColorRole::DangerSoft, TextColor::Danger, ICON_CIRCLE_ALERT),
    };
    view! {
        <Show condition={shown}>
            <Frame fill radius=Radius::Md padding={Padding::symmetric(Space::Md, Space::Sm)}>
                <List direction=Direction::Horizontal align=Align::Start spacing=Space::Sm>
                    <Text color>
                        <Icon glyph />
                    </Text>
                    <Text @sizing=ItemSize::Percent(100.0)>{text}</Text>
                </List>
            </Frame>
        </Show>
    }
}

#[component]
fn Body(page: Shown) -> NodeId {
    let screen = page.read(|page| page.screen.clone());
    let content = match &screen {
        Screen::SignIn => view! {
            <SignIn page />
        },
        Screen::SignInClosed => view! {
            <Message
                glyph=ICON_SHIELD_CHECK
                title="Sign-ins are paused"
                text="This server is not accepting new sign-ins. Devices that are already signed in keep working."
            />
        },
        Screen::Register => view! {
            <Register page />
        },
        Screen::RegistrationClosed => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        title="Registration is closed"
                        lead="This server is not accepting new accounts."
                    />
                    <PageLink label="Sign in instead" href={page.here("/auth/login")} />
                </List>
            </Card>
        },
        Screen::Forgot => view! {
            <Forgot page />
        },
        Screen::Reset => view! {
            <Reset page />
        },
        Screen::Verify => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_MAIL_CHECK)}
                        title="Verify your email address"
                        lead="Confirm that this address is yours to continue."
                    />
                    <Notice text={page.general()} />
                    <PortalForm
                        action="/auth/verify"
                        csrf={page.csrf()}
                        code={page.code()}
                        hidden={page.token()}
                    >
                        <Submit label="Verify email" />
                    </PortalForm>
                </List>
            </Card>
        },
        Screen::Approve => view! {
            <Approve page />
        },
        Screen::Account => view! {
            <AccountHome page />
        },
        Screen::Settings => view! {
            <Settings page />
        },
        Screen::Sessions => view! {
            <Sessions page />
        },
        Screen::Approved => view! {
            <Message
                glyph=ICON_CIRCLE_CHECK
                tone=TextColor::Success
                title="Sign-in approved"
                text="Return to Block to unlock your encrypted data. You can close this page."
            />
        },
        Screen::Denied => view! {
            <Message
                glyph=ICON_CIRCLE_X
                tone=TextColor::Danger
                title="Sign-in denied"
                text="The requesting device has not been signed in."
            />
        },
        Screen::CheckEmail => {
            let again = match page.read(|page| page.account.is_some()) {
                true => "/auth/settings".to_owned(),
                false => page.here("/auth/register"),
            };
            view! {
                <Card>
                    <List spacing=Space::Xl>
                        <Heading
                            glyph={Some(ICON_MAIL)}
                            title="Check your email"
                            lead="Follow the verification link to continue."
                        />
                        <Text style=TextStyle::PARAGRAPH>
                            "Email delivery is currently recorded in the server log; ask your server operator for the message. If the link expires, submit the form again to request another."
                        </Text>
                        <PageLink label="Request another verification link" href={again} />
                    </List>
                </Card>
            }
        }
        Screen::ResetRequested => view! {
            <Message
                glyph=ICON_MAIL
                title="Check your email"
                text="If an account matches that address, a reset link has been requested. Email delivery is currently recorded in the server log; ask your server operator for the message."
            />
        },
        Screen::PasswordReset => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_CIRCLE_CHECK)}
                        tone=TextColor::Success
                        title="Password reset"
                        lead="Every session was signed out. Your encryption keys are unchanged."
                    />
                    <PageLink
                        label="Sign in with your new password"
                        href={page.here("/auth/login")}
                    />
                </List>
            </Card>
        },
        Screen::EmailUpdated => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_MAIL_CHECK)}
                        tone=TextColor::Success
                        title="Email verified"
                        lead="Your email address has been updated."
                    />
                    <PageLink label="Back to your account" href="/auth" />
                </List>
            </Card>
        },
        Screen::NotFound => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_SEARCH_X)}
                        title="Page not found"
                        lead="This address is not part of the account portal."
                    />
                    <Notice text={page.general()} />
                    <PageLink label="Open the account portal" href="/auth" />
                </List>
            </Card>
        },
        Screen::Message { title, text } => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_CIRCLE_ALERT)}
                        title={title.clone()}
                        lead={text.clone()}
                    />
                    <PageLink label="Back to your account" href="/auth" />
                </List>
            </Card>
        },
    };
    content
}

#[component]
fn Message(
    glyph: Glyph,
    #[prop(default = TextColor::Accent)] tone: TextColor,
    title: String,
    text: String,
) -> NodeId {
    view! {
        <Card>
            <Heading glyph={Some(glyph)} tone title={title.clone()} lead={text.clone()} />
        </Card>
    }
}

fn values(page: Shown) -> (Signal<String>, Signal<String>) {
    page.read(|page| {
        (
            create_signal(page.email.clone()),
            create_signal(page.display_name.clone()),
        )
    })
}

fn confirm(
    password: Signal<String>,
    confirmation: Signal<String>,
    problem: String,
) -> Memo<String> {
    create_memo(move || {
        let typed = confirmation.get();
        match !typed.is_empty() && typed != password.get() {
            true => "The passwords do not match.".to_owned(),
            false => problem.clone(),
        }
    })
}

#[component]
fn SignIn(page: Shown) -> NodeId {
    let (email, _) = values(page);
    let password = create_signal(String::new());
    let code = create_signal(page.code());
    let (approving, entering, registration) = page.read(|page| {
        let approving = !page.code.is_empty() && page.code_problem.is_empty();
        (approving, !approving, page.allow_registration)
    });
    let lead = match approving {
        true => format!(
            "Sign in to approve the request from the Block app with the code {}.",
            page.code()
        ),
        false => "Use the email address and password of your Block account.".to_owned(),
    };
    view! {
        <List spacing=Space::Xl>
            <Card>
                <List spacing=Space::Xl>
                    <Heading title="Sign in" lead />
                    <Notice text={page.code_problem()} />
                    <Notice text={page.general()} />
                    <PortalForm action="/auth/login" csrf={page.csrf()} code={page.code()}>
                        <Field
                            kind=InputKind::Email
                            name="email"
                            label="Email"
                            autocomplete="username"
                            value={email}
                            problem={page.problem(Named::Email)}
                        />
                        <Field
                            kind=InputKind::Password
                            name="password"
                            label="Password"
                            autocomplete="current-password"
                            value={password}
                            problem={page.problem(Named::Password)}
                        />
                        <Submit label="Sign in" />
                    </PortalForm>
                    <List
                        direction=Direction::Horizontal
                        justify=Justify::SpaceBetween
                        spacing=Space::Md
                    >
                        <PageLink label="Forgot your password?" href={page.here("/auth/forgot")} />
                        <Show condition={registration}>
                            <PageLink
                                label="Create an account"
                                href={page.here("/auth/register")}
                            />
                        </Show>
                    </List>
                </List>
            </Card>
            <Show condition={entering}>
                <CodeEntry
                    csrf={page.csrf()}
                    code
                    lead="Signing in on another device? Enter the code Block shows you."
                />
            </Show>
        </List>
    }
}

#[component]
fn CodeEntry(csrf: String, code: Signal<String>, lead: String) -> NodeId {
    view! {
        <Frame border={Some(Border::DEFAULT)} radius=Radius::Lg padding={Padding::all(Space::Xl)}>
            <List spacing=Space::Lg>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                    <Text color=TextColor::Muted>
                        <Icon glyph=ICON_KEY_ROUND />
                    </Text>
                    <Text style=TextStyle::HEADING>"Have a sign-in code?"</Text>
                </List>
                <Text style=TextStyle::PARAGRAPH>{lead.clone()}</Text>
                <PortalForm action="/auth/code" csrf={csrf.clone()}>
                    <Field
                        kind=InputKind::Text
                        name="code"
                        label="Code shown in Block"
                        autocomplete="off"
                        value={code}
                    />
                    <Submit label="Continue with code" variant=ButtonVariant::Secondary />
                </PortalForm>
            </List>
        </Frame>
    }
}

#[component]
fn Register(page: Shown) -> NodeId {
    let (email, name) = values(page);
    let password = create_signal(String::new());
    let confirmation = create_signal(String::new());
    let mismatch = confirm(password, confirmation, page.problem(Named::Confirmation));
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    title="Create your account"
                    lead="We’ll email you a link to verify your address and finish creating your account."
                />
                <Notice text={page.general()} />
                <PortalForm action="/auth/register" csrf={page.csrf()} code={page.code()}>
                    <Field
                        kind=InputKind::Text
                        name="display_name"
                        label="Display name"
                        autocomplete="nickname"
                        hint="How other people see you in Block."
                        value={name}
                        problem={page.problem(Named::DisplayName)}
                    />
                    <Field
                        kind=InputKind::Email
                        name="email"
                        label="Email"
                        autocomplete="username"
                        value={email}
                        problem={page.problem(Named::Email)}
                    />
                    <Field
                        kind=InputKind::Password
                        name="password"
                        label="Password"
                        autocomplete="new-password"
                        hint="At least 8 characters."
                        value={password}
                        problem={page.problem(Named::Password)}
                    />
                    <ConfirmField value={confirmation} problem={mismatch} />
                    <Submit label="Create account" />
                </PortalForm>
                <PageLink
                    label="Already have an account? Sign in"
                    href={page.here("/auth/login")}
                />
            </List>
        </Card>
    }
}

#[component]
fn ConfirmField(value: Signal<String>, problem: Memo<String>) -> NodeId {
    let shown = create_memo(move || !problem.get().is_empty());
    view! {
        <List spacing=Space::Xs>
            <Field
                kind=InputKind::Password
                name="confirmation"
                label="Confirm password"
                autocomplete="new-password"
                value
            />
            <Show condition={shown}>
                <Text size=TextSize::Sm color=TextColor::Danger>{problem}</Text>
            </Show>
        </List>
    }
}

#[component]
fn Forgot(page: Shown) -> NodeId {
    let (email, _) = values(page);
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    title="Reset your password"
                    lead="Enter your account’s email address and we’ll send you a link to choose a new password."
                />
                <Notice text={page.general()} />
                <PortalForm action="/auth/forgot" csrf={page.csrf()} code={page.code()}>
                    <Field
                        kind=InputKind::Email
                        name="email"
                        label="Email"
                        autocomplete="username"
                        value={email}
                        problem={page.problem(Named::Email)}
                    />
                    <Submit label="Send reset link" />
                </PortalForm>
                <Text style=TextStyle::CAPTION>
                    "This resets your sign-in password only. Encrypted data still needs your recovery phrase or another unlocked device in Block."
                </Text>
                <PageLink label="Back to sign in" href={page.here("/auth/login")} />
            </List>
        </Card>
    }
}

#[component]
fn Reset(page: Shown) -> NodeId {
    let password = create_signal(String::new());
    let confirmation = create_signal(String::new());
    let mismatch = confirm(password, confirmation, page.problem(Named::Confirmation));
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    glyph={Some(ICON_KEY_ROUND)}
                    title="Choose a new password"
                    lead="Every session will be signed out. Your encryption keys stay unchanged."
                />
                <Notice text={page.general()} />
                <PortalForm
                    action="/auth/reset"
                    csrf={page.csrf()}
                    code={page.code()}
                    hidden={page.token()}
                >
                    <Field
                        kind=InputKind::Password
                        name="password"
                        label="New password"
                        autocomplete="new-password"
                        hint="At least 8 characters."
                        value={password}
                        problem={page.problem(Named::Password)}
                    />
                    <ConfirmField value={confirmation} problem={mismatch} />
                    <Submit label="Reset password" />
                </PortalForm>
            </List>
        </Card>
    }
}

#[component]
fn Approve(page: Shown) -> NodeId {
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    glyph={Some(ICON_SHIELD_CHECK)}
                    title="Approve sign-in to Block?"
                    lead="Only approve if you started this sign-in yourself and the code below matches the one Block shows."
                />
                <Frame
                    fill=ColorRole::Background
                    radius=Radius::Md
                    padding={Padding::all(Space::Lg)}
                    align={Align2::CENTER}
                >
                    <Text size=TextSize::Xl2 family=Family::Mono align=TextAlign::Center>
                        {page.code()}
                    </Text>
                </Frame>
                <Notice text={page.general()} />
                <PortalForm action="/auth/approve" csrf={page.csrf()} code={page.code()}>
                    <Submit label="Approve sign-in" />
                </PortalForm>
                <PortalForm action="/auth/deny" csrf={page.csrf()} code={page.code()}>
                    <Submit label="Deny" variant=ButtonVariant::Secondary />
                </PortalForm>
                <Separator />
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>
                        "Not your account?"
                    </Text>
                    <PortalForm action="/auth/logout" csrf={page.csrf()} code={page.code()}>
                        <Submit
                            label="Use another account"
                            variant=ButtonVariant::Ghost
                            wide=false
                        />
                    </PortalForm>
                </List>
            </List>
        </Card>
    }
}

#[component]
fn AccountHome(page: Shown) -> NodeId {
    let account = page.read(|page| page.account.clone()).unwrap_or_default();
    let code = create_signal(page.code());
    view! {
        <List spacing=Space::Xl>
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        title={format!("Hello, {}", account.display_name)}
                        lead={account.email.clone()}
                    />
                    <Notice text={page.code_problem()} />
                    <Notice text={page.general()} />
                    <List spacing=Space::Lg>
                        <Destination
                            glyph=ICON_KEY_ROUND
                            label="Account settings"
                            detail="Name, email address and password"
                            href="/auth/settings"
                        />
                        <Destination
                            glyph=ICON_LAPTOP
                            label="Signed-in sessions"
                            detail="See and sign out browsers and devices"
                            href="/auth/sessions"
                        />
                    </List>
                </List>
            </Card>
            <CodeEntry
                csrf={page.csrf()}
                code
                lead="To sign in to Block on a device, enter the code it shows you."
            />
        </List>
    }
}

#[component]
fn Destination(glyph: Glyph, label: String, detail: String, href: String) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Text size=TextSize::Lg color=TextColor::Muted>
                <Icon glyph />
            </Text>
            <List spacing=Space::Xs2 @sizing=ItemSize::Percent(100.0)>
                <PageLink label href />
                <Text style=TextStyle::CAPTION>{detail.clone()}</Text>
            </List>
        </List>
    }
}

#[component]
fn Section(title: String, lead: String, children: Children<ListChild>) -> NodeId {
    view! {
        <Card>
            <List spacing=Space::Lg>
                <List spacing=Space::Xs>
                    <Text style=TextStyle::HEADING weight=Weight::Bold>{title.clone()}</Text>
                    <Text style=TextStyle::CAPTION>{lead.clone()}</Text>
                </List>
                {children}
            </List>
        </Card>
    }
}

#[component]
fn Settings(page: Shown) -> NodeId {
    let (email, name) = values(page);
    let current_for_email = create_signal(String::new());
    let current = create_signal(String::new());
    let password = create_signal(String::new());
    let confirmation = create_signal(String::new());
    let mismatch = confirm(password, confirmation, page.problem(Named::Confirmation));
    let saved = match page.read(|page| page.saved) {
        true => "Your changes were saved.".to_owned(),
        false => String::new(),
    };
    view! {
        <List spacing=Space::Xl>
            <Heading title="Account settings" />
            <Notice text={saved} good=true />
            <Notice text={page.general()} />
            <Section title="Display name" lead="How other people see you in Block.">
                <PortalForm action="/auth/settings/name" csrf={page.csrf()}>
                    <Field
                        kind=InputKind::Text
                        name="display_name"
                        label="Display name"
                        autocomplete="nickname"
                        value={name}
                        problem={page.problem(Named::DisplayName)}
                    />
                    <Submit label="Save name" variant=ButtonVariant::Secondary />
                </PortalForm>
            </Section>
            <Section
                title="Email address"
                lead="Your address changes once you open the link we send to the new one."
            >
                <PortalForm action="/auth/settings/email" csrf={page.csrf()}>
                    <Field
                        kind=InputKind::Email
                        name="email"
                        label="New email"
                        autocomplete="email"
                        value={email}
                        problem={page.problem(Named::Email)}
                    />
                    <Field
                        kind=InputKind::Password
                        name="current_password"
                        label="Current password"
                        autocomplete="current-password"
                        value={current_for_email}
                        problem={page.problem(Named::CurrentPassword)}
                    />
                    <Submit label="Send verification link" variant=ButtonVariant::Secondary />
                </PortalForm>
            </Section>
            <Section title="Password" lead="Changing your password signs out every other session.">
                <PortalForm action="/auth/settings/password" csrf={page.csrf()}>
                    <Field
                        kind=InputKind::Password
                        name="current_password"
                        label="Current password"
                        autocomplete="current-password"
                        value={current}
                        problem={page.problem(Named::CurrentPassword)}
                    />
                    <Field
                        kind=InputKind::Password
                        name="password"
                        label="New password"
                        autocomplete="new-password"
                        hint="At least 8 characters."
                        value={password}
                        problem={page.problem(Named::Password)}
                    />
                    <ConfirmField value={confirmation} problem={mismatch} />
                    <Submit label="Change password" variant=ButtonVariant::Secondary />
                </PortalForm>
            </Section>
        </List>
    }
}

#[component]
fn Sessions(page: Shown) -> NodeId {
    let sessions = page.read(|page| page.sessions.clone());
    let others = sessions.iter().any(|session| !session.current);
    let csrf = page.csrf();
    view! {
        <List spacing=Space::Xl>
            <Heading
                title="Signed-in sessions"
                lead="Signing out a session disconnects it from this server. It does not erase data already saved on that device."
            />
            <Notice text={page.general()} />
            <Card>
                <List spacing=Space::Lg>
                    <ForEach keys={sessions}>|session: Session|
                        <SessionRow session csrf={csrf.clone()} />
                    </ForEach>
                </List>
            </Card>
            <Show condition={others}>
                <PortalForm action="/auth/sessions/others" csrf={page.csrf()}>
                    <Submit label="Sign out all other sessions" variant=ButtonVariant::Danger />
                </PortalForm>
            </Show>
        </List>
    }
}

#[component]
fn SessionRow(session: Session, csrf: String) -> NodeId {
    let (glyph, kind) = match session.kind {
        SessionKind::Browser => (ICON_GLOBE, "Browser"),
        SessionKind::App => (ICON_LAPTOP, "Block app"),
    };
    let detail = match session.current {
        true => format!("Created {} · this browser", session.age),
        false => format!("Created {}", session.age),
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Text size=TextSize::Lg color=TextColor::Muted>
                <Icon glyph />
            </Text>
            <List spacing=Space::Xs2 @sizing=ItemSize::Percent(100.0)>
                <Text weight=Weight::Bold>{kind}</Text>
                <Text style=TextStyle::CAPTION>{detail.clone()}</Text>
            </List>
            <PortalForm
                action="/auth/sessions/revoke"
                csrf={csrf.clone()}
                hidden={vec![("session", session.id.clone())]}
            >
                <Submit label="Sign out" variant=ButtonVariant::Secondary wide=false />
            </PortalForm>
        </List>
    }
}
