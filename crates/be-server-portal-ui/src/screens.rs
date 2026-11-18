use be_server_portal::{
    Field as Named, IdentityChange, IdentityRow, Page, PasskeyOffer, PasskeyStep, ProviderLink,
    Screen, Session, SessionKind, with_code,
};
use beui::icons::{
    Glyph, ICON_BLOCKS, ICON_CIRCLE_ALERT, ICON_CIRCLE_CHECK, ICON_CIRCLE_X, ICON_GLOBE,
    ICON_KEY_ROUND, ICON_LAPTOP, ICON_LOG_IN, ICON_MAIL, ICON_MAIL_CHECK, ICON_SEARCH_X,
    ICON_SHIELD_CHECK,
};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Align2, Direction, Extent, Justify, NodeId, TextAlign};

use crate::form::{Field, PageLink, PortalForm, Submit, fixed};
use crate::passkey::PasskeyButton;

const PRODUCT: &str = "be3";

pub fn title(page: &Page) -> String {
    let name = match &page.screen {
        Screen::SignIn | Screen::SignInClosed => "Sign in",
        Screen::Register | Screen::RegistrationClosed | Screen::ProviderRegister => {
            "Create an account"
        }
        Screen::Verify => "Verify your email address",
        Screen::LoginLink => "Sign in with a link",
        Screen::Approve => "Approve sign-in",
        Screen::Account => "Your account",
        Screen::Settings => "Account settings",
        Screen::Sessions => "Signed-in sessions",
        Screen::Approved => "Sign-in approved",
        Screen::Denied => "Sign-in denied",
        Screen::CheckEmail => "Check your email",
        Screen::EmailUpdated => "Email verified",
        Screen::EnterCode => "Enter your code",
        Screen::CodeElsewhere { .. } => "Your code",
        Screen::Recover => "Get back into your account",
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

    /// The passkey ceremony the page offers for `step`, if it offers that one.
    fn passkey(self, step: PasskeyStep) -> Option<PasskeyOffer> {
        self.read(|page| page.passkey.clone().filter(|offer| offer.step == step))
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
            <ListItem grow=1.0>
                <Text size=TextSize::Lg weight=Weight::Bold>
                    {PRODUCT}
                    <Span color=TextColor::Muted>" account"</Span>
                </Text>
            </ListItem>
            {elsewhere.then(|| view! {
                <PageLink label="Your account" href="/auth" />
            })}
        </List>
    }
}

#[component]
fn Footer(page: Shown) -> NodeId {
    let account = page.read(|page| page.account.clone());
    let signed_in = account.is_some();
    let who = fixed(
        account
            .map(|account| match account.email.as_str() {
                "" => format!("Signed in as {}", account.display_name),
                email => format!("Signed in as {} ({email})", account.display_name),
            })
            .unwrap_or_default(),
    );
    let shows_sign_out = signed_in && page.read(|page| page.screen != Screen::Approve);
    view! {
        <List spacing=Space::Md align=Align::Center>
            {signed_in.then(|| view! {
                <Text style=TextStyle::CAPTION align=TextAlign::Center>{who}</Text>
            })}
            {shows_sign_out.then(|| view! {
                <PortalForm action="/auth/logout" csrf={page.csrf()} code={page.code()}>
                    <Submit label="Sign out" variant=ButtonVariant::Ghost wide=false />
                </PortalForm>
            })}
        </List>
    }
}

#[component]
fn Heading(
    title: String,
    #[prop(default = String::new())] lead: String,
    glyph: Option<Glyph>,
    #[prop(default = TextColor::Accent)] tone: TextColor,
) -> NodeId {
    let has_lead = !lead.is_empty();
    let title = fixed(title);
    let lead = fixed(lead);
    view! {
        <List spacing=Space::Sm>
            {glyph.map(|glyph| view! {
                <Text size=TextSize::Xl3 color={tone}>
                    <Icon glyph />
                </Text>
            })}
            <Text style=TextStyle::TITLE weight=Weight::Bold>{title}</Text>
            {has_lead.then(|| view! {
                <Text style=TextStyle::PARAGRAPH>{lead}</Text>
            })}
        </List>
    }
}

#[component]
fn Notice(text: String, #[prop(default = false)] good: bool) -> Option<NodeId> {
    if text.is_empty() {
        return None;
    }
    let text = fixed(text);
    let (fill, color, glyph) = match good {
        true => (ColorRole::AccentSoft, TextColor::Success, ICON_CIRCLE_CHECK),
        false => (ColorRole::DangerSoft, TextColor::Danger, ICON_CIRCLE_ALERT),
    };
    Some(view! {
        <Frame fill radius=Radius::Md padding={Padding::symmetric(Space::Md, Space::Sm)}>
            <List direction=Direction::Horizontal align=Align::Start spacing=Space::Sm>
                <Text color>
                    <Icon glyph />
                </Text>
                <ListItem grow=1.0>
                    <Text>{text}</Text>
                </ListItem>
            </List>
        </Frame>
    })
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
        Screen::ProviderRegister => view! {
            <ProviderRegister page />
        },
        Screen::LoginLink => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_KEY_ROUND)}
                        title="Sign in with a link"
                        lead="Your server's operator made this link for you. It works once, and signs out every other session of the account."
                    />
                    <Notice text={page.general()} />
                    <PortalForm action="/auth/link" csrf={page.csrf()} hidden={page.token()}>
                        <Submit label="Sign in" />
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
                text="Return to be-host to unlock your encrypted data. You can close this page."
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
                        <PageLink label="Request another verification link" href="/auth/settings" />
                    </List>
                </Card>
            }
        }
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
        Screen::EnterCode => view! {
            <EnterCode page />
        },
        Screen::CodeElsewhere { code } => view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_MAIL)}
                        title={format!("Your code is {code}")}
                        lead="Enter it in the browser where you asked for it. Opening the link here signs nothing in."
                    />
                    <PageLink label="Sign in on this browser instead" href="/auth/login" />
                </List>
            </Card>
        },
        Screen::Recover => view! {
            <Recover page />
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

#[component]
fn SignIn(page: Shown) -> NodeId {
    let code = create_signal(page.code());
    let (approving, entering, registration) = page.read(|page| {
        let approving = !page.code.is_empty() && page.code_problem.is_empty();
        (approving, !approving, page.allow_registration)
    });
    let lead = match approving {
        true => format!(
            "Sign in to approve the request from be-host with the code {}.",
            page.code()
        ),
        false => "We’ll email you a code to sign in with.".to_owned(),
    };
    let (email, _) = values(page);
    view! {
        <List spacing=Space::Xl>
            <Card>
                <List spacing=Space::Xl>
                    <Heading title="Sign in" lead />
                    <Notice text={page.code_problem()} />
                    <Notice text={page.general()} />
                    <PortalForm action="/auth/code/send" csrf={page.csrf()} code={page.code()}>
                        <Field
                            kind=InputKind::Email
                            name="email"
                            label="Email"
                            autocomplete="username"
                            value={email}
                            problem={page.problem(Named::Email)}
                        />
                        <Submit label="Email me a code" />
                    </PortalForm>
                    {page.passkey(PasskeyStep::SignIn).map(|offer| view! {
                        <PasskeyButton
                            offer
                            csrf={page.csrf()}
                            code={page.code()}
                            label="Sign in with a passkey"
                        />
                    })}
                    <Providers page />
                    <List
                        direction=Direction::Horizontal
                        justify=Justify::SpaceBetween
                        wrap=true
                        spacing=Space::Md
                    >
                        <PageLink label="I can’t sign in" href={page.here("/auth/recover")} />
                        {registration.then(|| view! {
                            <PageLink
                                label="Create an account"
                                href={page.here("/auth/register")}
                            />
                        })}
                    </List>
                </List>
            </Card>
            {entering.then(|| view! {
                <CodeEntry
                    csrf={page.csrf()}
                    code
                    lead="Signing in on another device? Enter the code be-host shows you."
                />
            })}
        </List>
    }
}

/// Links that sign in with each provider the server has, when it has any.
#[component]
fn Providers(page: Shown) -> Option<NodeId> {
    let providers = page.read(|page| page.providers.clone());
    (!providers.is_empty()).then(|| {
        view! {
            <List spacing=Space::Md>
                <Text style=TextStyle::CAPTION>"Or continue with"</Text>
                <ForEach value={providers}>|provider: ProviderLink|
                    <PageLink
                        label={format!("Continue with {}", provider.name)}
                        href={provider.href.clone()}
                    />
                </ForEach>
            </List>
        }
    })
}

/// A provider sign-in with no account here: the invite code makes one.
#[component]
fn ProviderRegister(page: Shown) -> NodeId {
    let (_, name) = values(page);
    let invite = page.read(|page| create_signal(page.invite.clone()));
    let provider = page.read(|page| page.provider.clone());
    let lead = match provider.is_empty() {
        true => "This sign-in expired. Sign in with your provider again.".to_owned(),
        false => format!(
            "No account here signs in with this {provider} account yet. Enter an invite code to create one."
        ),
    };
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading title="Create your account" lead />
                <Notice text={page.general()} />
                <PortalForm action="/auth/oidc/register" csrf={page.csrf()}>
                    <Field
                        kind=InputKind::Text
                        name="invite"
                        label="Invite code"
                        autocomplete="off"
                        hint="From the people who run this server."
                        value={invite}
                        problem={page.problem(Named::Invite)}
                    />
                    <Field
                        kind=InputKind::Text
                        name="display_name"
                        label="Display name"
                        autocomplete="nickname"
                        hint="How other people see you in be3."
                        value={name}
                        problem={page.problem(Named::DisplayName)}
                    />
                    <Submit label="Create account" />
                </PortalForm>
                <PageLink label="Sign in another way" href="/auth/login" />
            </List>
        </Card>
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
                        label="Code shown in be-host"
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
    let invite = page.read(|page| create_signal(page.invite.clone()));
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    title="Create your account"
                    lead="We’ll email you a code to verify your address and finish creating your account."
                />
                <Notice text={page.general()} />
                <PortalForm action="/auth/register" csrf={page.csrf()} code={page.code()}>
                    <Field
                        kind=InputKind::Text
                        name="invite"
                        label="Invite code"
                        autocomplete="off"
                        hint="From the people who run this server."
                        value={invite}
                        problem={page.problem(Named::Invite)}
                    />
                    <Field
                        kind=InputKind::Text
                        name="display_name"
                        label="Display name"
                        autocomplete="nickname"
                        hint="How other people see you in be3."
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
                    <Submit label="Email me a code" />
                </PortalForm>
                <Providers page />
                <PageLink
                    label="Already have an account? Sign in"
                    href={page.here("/auth/login")}
                />
            </List>
        </Card>
    }
}

#[component]
fn Approve(page: Shown) -> NodeId {
    // A browser signed in a while ago signs in again first: a cookie alone approves nothing
    // (guides/blocks/server.md, "Accounts"). Signing out keeps the code, so sign-in returns here.
    if !page.read(|page| page.fresh) {
        return view! {
            <Card>
                <List spacing=Space::Xl>
                    <Heading
                        glyph={Some(ICON_SHIELD_CHECK)}
                        title="Sign in again to approve"
                        lead="You signed in to this browser a while ago. To approve a sign-in to be-host, confirm it is you by signing in again."
                    />
                    <Notice text={page.general()} />
                    <PortalForm action="/auth/logout" csrf={page.csrf()} code={page.code()}>
                        <Submit label="Sign in again" />
                    </PortalForm>
                    <PortalForm action="/auth/deny" csrf={page.csrf()} code={page.code()}>
                        <Submit label="Deny" variant=ButtonVariant::Secondary />
                    </PortalForm>
                </List>
            </Card>
        };
    }
    let (requester, elsewhere) =
        page.read(|page| (page.requester.clone(), page.requester_elsewhere));
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    glyph={Some(ICON_SHIELD_CHECK)}
                    title="Approve sign-in to be-host?"
                    lead="Only approve if you started this sign-in yourself and the code below matches the one be-host shows."
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
                {(!requester.is_empty()).then(|| view! {
                    <Text
                        style=TextStyle::CAPTION
                        color={if elsewhere { TextColor::Warning } else { TextColor::Muted }}
                    >
                        {requester.clone()}
                    </Text>
                })}
                <Notice text={page.general()} />
                <PortalForm action="/auth/approve" csrf={page.csrf()} code={page.code()}>
                    <Submit label="Approve sign-in" />
                </PortalForm>
                <PortalForm action="/auth/deny" csrf={page.csrf()} code={page.code()}>
                    <Submit label="Deny" variant=ButtonVariant::Secondary />
                </PortalForm>
                <Separator />
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <Text style=TextStyle::CAPTION>"Not your account?"</Text>
                    </ListItem>
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
                            detail="Name, email address and ways to sign in"
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
                lead="To sign in to be-host on a device, enter the code it shows you."
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
            <ListItem grow=1.0>
                <List spacing=Space::Xs2>
                    <PageLink label href />
                    <Text style=TextStyle::CAPTION>{detail.clone()}</Text>
                </List>
            </ListItem>
        </List>
    }
}

#[component]
fn Section(title: String, lead: String, children: Children<ListItem>) -> NodeId {
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
    let email_lead = match page.read(|page| {
        page.account
            .as_ref()
            .is_some_and(|account| !account.email.is_empty())
    }) {
        true => "Your address changes once you open the link we send to the new one.",
        false => "Your address is added once you open the link we send to it.",
    }
    .to_owned();
    let (identities, fresh, adding) =
        page.read(|page| (page.identities.clone(), page.fresh, page.providers.clone()));
    let csrf = page.csrf();
    // One way in that isn't an email is one lost device or provider from no way in.
    let alone = match identities.as_slice() {
        [only] if only.kind != "email" => {
            "Add a second way to sign in, so losing this one doesn't lock you out.".to_owned()
        }
        _ => String::new(),
    };
    let add_passkey = page.passkey(PasskeyStep::Add).filter(|_| fresh).map(|offer| {
        view! {
            <PasskeyButton offer csrf={page.csrf()} code={String::new()} label="Add a passkey" />
        }
    });
    let saved = match page.read(|page| page.saved) {
        true => "Your changes were saved.".to_owned(),
        false => String::new(),
    };
    view! {
        <List spacing=Space::Xl>
            <Heading title="Account settings" />
            <Notice text={saved} good=true />
            <Notice text={page.general()} />
            <Proof page fresh doing="change your email address or your ways to sign in" />
            <Section
                title="Ways to sign in"
                lead="Any one of these signs you in. Your last one can’t be removed."
            >
                <Notice text={alone} />
                <ForEach value={identities}>|identity: IdentityRow|
                    <IdentityLine identity csrf={csrf.clone()} fresh />
                </ForEach>
                {add_passkey}
                {(fresh && !adding.is_empty()).then(|| view! {
                    <List spacing=Space::Md>
                        <ForEach value={adding.clone()}>|provider: ProviderLink|
                            <PageLink
                                label={format!("Add {}", provider.name)}
                                href={provider.href.clone()}
                            />
                        </ForEach>
                    </List>
                })}
            </Section>
            <RecentChanges page />
            <Section title="Display name" lead="How other people see you in be3.">
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
            <Section title="Email address" lead={email_lead}>
                <PortalForm action="/auth/settings/email" csrf={page.csrf()}>
                    <Field
                        kind=InputKind::Email
                        name="email"
                        label="New email"
                        autocomplete="email"
                        value={email}
                        problem={page.problem(Named::Email)}
                    />
                    <Submit label="Send verification link" variant=ButtonVariant::Secondary />
                </PortalForm>
            </Section>
        </List>
    }
}

/// Ways to sign in added or removed in the last thirty days, which nobody signed in can hide.
#[component]
fn RecentChanges(page: Shown) -> Option<NodeId> {
    let changes = page.read(|page| page.identity_changes.clone());
    (!changes.is_empty()).then(|| {
        view! {
            <Section
                title="Recent changes"
                lead="Ways to sign in added or removed in the last thirty days. If one wasn’t you, remove it and contact your server operator."
            >
                <ForEach value={changes}>|change: IdentityChange|
                    <List spacing=Space::Xs2>
                        <Text>{change.text.clone()}</Text>
                        <Text style=TextStyle::CAPTION>{change.when.clone()}</Text>
                    </List>
                </ForEach>
            </Section>
        }
    })
}

#[component]
fn Sessions(page: Shown) -> NodeId {
    let sessions = page.read(|page| page.sessions.clone());
    let others = sessions.iter().any(|session| !session.current);
    let fresh = page.read(|page| page.fresh);
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
                    <ForEach value={sessions}>|session: Session|
                        <SessionRow session csrf={csrf.clone()} />
                    </ForEach>
                </List>
            </Card>
            {(others && fresh).then(|| view! {
                <PortalForm action="/auth/sessions/others" csrf={page.csrf()}>
                    <Submit label="Sign out all other sessions" variant=ButtonVariant::Danger />
                </PortalForm>
            })}
            {(others && !fresh).then(|| view! {
                <Proof page fresh doing="sign out every other session" />
            })}
        </List>
    }
}

#[component]
fn SessionRow(session: Session, csrf: String) -> NodeId {
    let (glyph, kind) = match session.kind {
        SessionKind::Browser => (ICON_GLOBE, "Browser"),
        SessionKind::Host => (ICON_LAPTOP, "be-host"),
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
            <ListItem grow=1.0>
                <List spacing=Space::Xs2>
                    <Text weight=Weight::Bold>{kind}</Text>
                    <Text style=TextStyle::CAPTION>{detail.clone()}</Text>
                </List>
            </ListItem>
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

/// Asks for a code to the account's email, which proves for a few minutes that this browser is
/// the owner's, before an account change.
#[component]
fn Proof(page: Shown, fresh: bool, doing: String) -> NodeId {
    let (emailed, providers) = page.read(|page| {
        (
            page.account
                .as_ref()
                .is_some_and(|account| !account.email.is_empty()),
            page.proof_providers.clone(),
        )
    });
    let passkey = page.passkey(PasskeyStep::Proof);
    let ways: Vec<&str> = [
        (emailed, "a code we email you"),
        (passkey.is_some(), "a passkey"),
        (!providers.is_empty(), "a provider you sign in with"),
    ]
    .into_iter()
    .filter_map(|(has, way)| has.then_some(way))
    .collect();
    let ways = match ways.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
        None => "one of your ways to sign in".to_owned(),
    };
    let lead = match fresh {
        true => format!("You confirmed it is you, so you can {doing} for the next few minutes."),
        false => format!("To {doing}, first confirm it is you with {ways}."),
    };
    view! {
        <Frame border={Some(Border::DEFAULT)} radius=Radius::Lg padding={Padding::all(Space::Xl)}>
            <List spacing=Space::Lg>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
                    <Text color=TextColor::Muted>
                        <Icon glyph=ICON_SHIELD_CHECK />
                    </Text>
                    <Text style=TextStyle::HEADING>"Confirm it is you"</Text>
                </List>
                <Text style=TextStyle::PARAGRAPH>{lead.clone()}</Text>
                {(!fresh && emailed).then(|| view! {
                    <PortalForm action="/auth/proof" csrf={page.csrf()}>
                        <Submit label="Email me a code" variant=ButtonVariant::Secondary />
                    </PortalForm>
                })}
                {passkey.filter(|_| !fresh).map(|offer| view! {
                    <PasskeyButton
                        offer
                        csrf={page.csrf()}
                        code={String::new()}
                        label="Confirm with a passkey"
                    />
                })}
                {(!fresh && !providers.is_empty()).then(|| view! {
                    <List spacing=Space::Md>
                        <ForEach value={providers.clone()}>|provider: ProviderLink|
                            <PageLink
                                label={format!("Confirm with {}", provider.name)}
                                href={provider.href.clone()}
                            />
                        </ForEach>
                    </List>
                })}
            </List>
        </Frame>
    }
}

#[component]
fn IdentityLine(identity: IdentityRow, csrf: String, fresh: bool) -> NodeId {
    let detail = match (identity.kind.as_str(), identity.removable) {
        ("email", true) => "Email: a code we send signs you in",
        ("email", false) => "Email: your only way to sign in",
        ("passkey", true) => "Passkey: this device or password manager signs you in",
        ("passkey", false) => "Passkey: your only way to sign in",
        (_, true) => "Provider: signing in with it signs you in",
        (_, false) => "Provider: your only way to sign in",
    }
    .to_owned();
    let glyph = match identity.kind.as_str() {
        "email" => ICON_MAIL,
        "passkey" => ICON_KEY_ROUND,
        _ => ICON_LOG_IN,
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
            <Text size=TextSize::Lg color=TextColor::Muted>
                <Icon glyph />
            </Text>
            <ListItem grow=1.0>
                <List spacing=Space::Xs2>
                    <Text weight=Weight::Bold>{identity.label.clone()}</Text>
                    <Text style=TextStyle::CAPTION>{detail.clone()}</Text>
                </List>
            </ListItem>
            {(identity.removable && fresh).then(|| view! {
                <PortalForm
                    action="/auth/settings/identity/remove"
                    csrf={csrf.clone()}
                    hidden={vec![("identity", identity.id.clone())]}
                >
                    <Submit label="Remove" variant=ButtonVariant::Secondary wide=false />
                </PortalForm>
            })}
        </List>
    }
}

#[component]
fn EnterCode(page: Shown) -> NodeId {
    let typed = page.read(|page| create_signal(page.email_code.clone()));
    let lead = page.read(|page| match (page.provider.is_empty(), page.code_email.is_empty()) {
        (false, _) => format!(
            "{} already has an account here. Enter the code we sent it to sign in and add {} to that account, or sign in another way.",
            match page.code_email.is_empty() {
                true => "Your email",
                false => page.code_email.as_str(),
            },
            page.provider
        ),
        (true, true) => {
            "Enter the code from the email we sent you. Ask for a new one if it has expired."
                .to_owned()
        }
        (true, false) => format!(
            "If {} has an account here, we sent it a code. It works for ten minutes.",
            page.code_email
        ),
    });
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading glyph={Some(ICON_MAIL)} title="Enter your code" lead />
                <Notice text={page.general()} />
                <PortalForm action="/auth/enter" csrf={page.csrf()} code={page.code()}>
                    <Field
                        kind=InputKind::Text
                        name="email_code"
                        label="Code"
                        autocomplete="one-time-code"
                        value={typed}
                        problem={page.problem(Named::EmailCode)}
                    />
                    <Submit label="Continue" />
                </PortalForm>
                <Text style=TextStyle::CAPTION>
                    "Email delivery is currently recorded in the server log; ask your server operator for the message."
                </Text>
                <PageLink label="Ask for a new code" href={page.here("/auth/login")} />
            </List>
        </Card>
    }
}

#[component]
fn Recover(page: Shown) -> NodeId {
    let (email, _) = values(page);
    view! {
        <Card>
            <List spacing=Space::Xl>
                <Heading
                    title="Can’t sign in?"
                    lead="We’ll email a code to your account’s address. Signing in with it signs out every other session."
                />
                <Notice text={page.general()} />
                <PortalForm
                    action="/auth/code/send"
                    csrf={page.csrf()}
                    code={page.code()}
                    hidden={vec![("purpose", "recover".to_owned())]}
                >
                    <Field
                        kind=InputKind::Email
                        name="email"
                        label="Email"
                        autocomplete="username"
                        value={email}
                        problem={page.problem(Named::Email)}
                    />
                    <Submit label="Email me a code" />
                </PortalForm>
                <Text style=TextStyle::CAPTION>
                    "This gets you back into your account only. Encrypted data still needs your recovery phrase or another unlocked device with be-host."
                </Text>
                <PageLink label="Back to sign in" href={page.here("/auth/login")} />
            </List>
        </Card>
    }
}
