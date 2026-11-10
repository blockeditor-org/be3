//! Sending pushes: Web Push (RFC 8030) with its payloads encrypted to the device (RFC 8291,
//! `be_lib_web_push`) and
//! the server identified by VAPID (RFC 8292), and FCM's HTTP v1 API for Android devices without
//! a UnifiedPush distributor. Every push carries the same bytes, encrypted the same way, so a
//! device decrypts what either sends alike. guides/blocks/server.md, "Notifications".

use std::{
    collections::{HashMap, VecDeque},
    io,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use be_block_protocol::{PushEndpoint, PushService};
use ring::{
    rand::SystemRandom,
    signature::{self, KeyPair},
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{ServerError, store::ServerStore};

/// How long a push service keeps a push for a device that is away.
const TIME_TO_LIVE: Duration = Duration::from_secs(24 * 60 * 60);
/// How long one request to a push service may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Pushes sent at once over the whole server. Each holds a blocking thread while it waits on
/// its service, so a service that never answers holds at most these, never the pool the
/// database shares.
const SENDERS: usize = 32;
/// Pushes waiting over the whole server, past which more are dropped.
const MAX_QUEUED: usize = 16_384;
/// Pushes waiting for one account's devices, past which more are dropped.
const MAX_QUEUED_PER_ACCOUNT: usize = 256;
/// What each push's plaintext is padded to: the largest `PushMessage`, so its length says
/// nothing of the notification's.
pub const PADDED_BYTES: usize = 48 + be_block_protocol::MAX_NOTIFICATION_BYTES;
/// Failures in a row, spread over at least `FAILING_FOR`, after which an endpoint is dropped.
pub const MAX_FAILURES: i64 = 5;
pub const FAILING_FOR: Duration = Duration::from_secs(24 * 60 * 60);
/// How long a VAPID token is valid: RFC 8292 allows at most a day.
const VAPID_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
/// The push services' hosts a Web Push endpoint may be on, exactly: Chrome's (FCM), Firefox's
/// (Mozilla autopush) and Safari's (Apple). The server posts nowhere else unless its operator
/// adds a host (`PushConfig::hosts`), so it is no relay for POSTs to whatever a member names
/// (decisions/push_endpoints_are_allowlisted.md).
pub const PUSH_HOSTS: &[&str] = &[
    "fcm.googleapis.com",
    "updates.push.services.mozilla.com",
    "web.push.apple.com",
];
/// Suffixes of the push services' hosts that vary per device: Mozilla's other autopush hosts
/// and Edge's (Microsoft WNS, `*.notify.windows.com`).
pub const PUSH_HOST_SUFFIXES: &[&str] = &[".push.services.mozilla.com", ".notify.windows.com"];

/// Whether a Web Push endpoint may be on `host`: a known push service's, or one the operator
/// added.
pub fn allowed_host(host: &str, added: &[String]) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    PUSH_HOSTS.contains(&host.as_str())
        || PUSH_HOST_SUFFIXES
            .iter()
            .any(|suffix| host.len() > suffix.len() && host.ends_with(suffix))
        || added.iter().any(|added| added.eq_ignore_ascii_case(&host))
}

/// FCM's API, which `FcmConfig::endpoint` replaces in tests.
pub const FCM_ENDPOINT: &str = "https://fcm.googleapis.com";

/// Where pushes may go (`ServerConfig::push`).
#[derive(Clone, Debug)]
pub struct PushConfig {
    /// Lets Web Push endpoints be `http://` and on loopback or private addresses, for tests
    /// that serve one. Off, a device's endpoint must be `https://` on a public address, so a
    /// client cannot make the server post into its own network.
    pub allow_local_endpoints: bool,
    /// FCM, when the server was started with `--fcm-credentials`; without, devices register
    /// only Web Push endpoints.
    pub fcm: Option<FcmConfig>,
    /// Hosts Web Push endpoints may be on besides the known push services (`PUSH_HOSTS`,
    /// `PUSH_HOST_SUFFIXES`), each matched exactly: a self-hosted UnifiedPush server, such as
    /// ntfy (`--push-host`).
    pub hosts: Vec<String>,
}

impl PushConfig {
    pub const DEFAULT: Self = Self {
        allow_local_endpoints: false,
        fcm: None,
        hosts: Vec::new(),
    };
}

/// A Firebase project's service account (the JSON file the console hands out) and where its
/// API is.
#[derive(Clone, Debug)]
pub struct FcmConfig {
    pub project_id: String,
    pub client_email: String,
    /// The account's private key, PKCS#8 DER.
    pub private_key: Vec<u8>,
    /// Where an access token is asked for.
    pub token_uri: String,
    /// FCM's API: `FCM_ENDPOINT`, or a test's fake.
    pub endpoint: String,
}

impl FcmConfig {
    /// Reads a service account's JSON file.
    pub fn from_service_account(json: &str) -> Result<Self, String> {
        let account: ureq::serde_json::Value =
            ureq::serde_json::from_str(json).map_err(|error| error.to_string())?;
        let field = |name: &str| {
            account[name]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("the service account has no {name}"))
        };
        let private_key = pem_body(&field("private_key")?)?;
        signature::RsaKeyPair::from_pkcs8(&private_key)
            .map_err(|error| format!("the service account's private key is {error}"))?;
        Ok(Self {
            project_id: field("project_id")?,
            client_email: field("client_email")?,
            private_key,
            token_uri: field("token_uri")?,
            endpoint: FCM_ENDPOINT.to_owned(),
        })
    }
}

fn pem_body(pem: &str) -> Result<Vec<u8>, String> {
    let body: String = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .flat_map(|line| line.chars().filter(|char| !char.is_whitespace()))
        .collect();
    STANDARD
        .decode(body)
        .map_err(|error| format!("the private key is not PEM: {error}"))
}

/// The server's VAPID key pair, made on first start and kept in the database (`push_key`).
pub(crate) struct Vapid {
    pair: signature::EcdsaKeyPair,
}

impl Vapid {
    pub(crate) fn load_or_create(connection: &Connection) -> Result<Self, ServerError> {
        let random = SystemRandom::new();
        let held: Option<Vec<u8>> = connection
            .query_row("SELECT pkcs8 FROM push_key WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()?;
        let pkcs8 = match held {
            Some(pkcs8) => pkcs8,
            None => {
                let made = signature::EcdsaKeyPair::generate_pkcs8(
                    &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
                    &random,
                )
                .map_err(|_| ServerError::Corrupt)?;
                connection.execute(
                    "INSERT OR IGNORE INTO push_key (id, pkcs8) VALUES (1, ?1)",
                    params![made.as_ref()],
                )?;
                connection.query_row("SELECT pkcs8 FROM push_key WHERE id = 1", [], |row| {
                    row.get(0)
                })?
            }
        };
        let pair = signature::EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            &pkcs8,
            &random,
        )
        .map_err(|_| ServerError::Corrupt)?;
        Ok(Self { pair })
    }

    /// The public key, uncompressed: what a browser subscribes with.
    pub(crate) fn public(&self) -> Vec<u8> {
        self.pair.public_key().as_ref().to_vec()
    }

    /// The `Authorization` header for a push to `endpoint` (RFC 8292): a token for the
    /// endpoint's origin, signed by this key, and the key.
    pub(crate) fn authorization(&self, endpoint: &url::Url, subject: &str, now: u64) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
        let claims = ureq::serde_json::json!({
            "aud": endpoint.origin().ascii_serialization(),
            "exp": now + VAPID_LIFETIME.as_secs(),
            "sub": subject,
        });
        let claims = URL_SAFE_NO_PAD.encode(claims.to_string());
        let signed = format!("{header}.{claims}");
        let signature = self
            .pair
            .sign(&SystemRandom::new(), signed.as_bytes())
            .expect("signing with a P-256 key does not fail");
        format!(
            "vapid t={signed}.{}, k={}",
            URL_SAFE_NO_PAD.encode(signature.as_ref()),
            URL_SAFE_NO_PAD.encode(self.public()),
        )
    }
}

/// What became of one push.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Sent {
    Delivered,
    /// The service says the device is gone (Web Push 404 or 410, FCM `UNREGISTERED`): the
    /// endpoint is dropped.
    Gone,
    /// The service asked to be sent less (429, 503): not the endpoint's failure.
    Throttled,
    /// Anything else, counted against the endpoint (`MAX_FAILURES`).
    Failed(String),
}

/// One push waiting to go.
struct Job {
    organization: Uuid,
    endpoint: PushEndpoint,
    /// Whether the endpoint has failures counted, which a delivery clears.
    failing: bool,
    block: Uuid,
    /// Whether it mentions the recipient, which sends it at high urgency.
    urgent: bool,
    message: Arc<[u8]>,
}

#[derive(Default)]
struct Queues {
    by_account: HashMap<Uuid, VecDeque<Job>>,
    total: usize,
}

impl Queues {
    /// Drops the newest pushes of the longest queues until `MAX_QUEUED` wait, so the accounts
    /// with the most waiting lose theirs, not whoever queues next. Runs only past the cap.
    fn shed(&mut self) {
        while self.total > MAX_QUEUED {
            let Some(longest) = self
                .by_account
                .values_mut()
                .max_by_key(|queue| queue.len())
                .filter(|queue| !queue.is_empty())
            else {
                return;
            };
            longest.pop_back();
            self.total -= 1;
        }
    }
}

/// One recipient's devices, for `Pusher::queue`.
pub(crate) struct Delivery {
    pub(crate) account: Uuid,
    /// Each endpoint, and whether it has failures counted.
    pub(crate) endpoints: Vec<(PushEndpoint, bool)>,
    pub(crate) urgent: bool,
}

/// Sends pushes on blocking threads, at most `SENDERS` at once. Each account's go one at a
/// time, in the order they were queued and its devices in the order they registered, so a
/// device whose service is slow or never answers delays only its own account's pushes; a
/// push about a block that already waits for a device replaces the one waiting.
pub(crate) struct Pusher {
    queues: Mutex<Queues>,
    senders: Arc<tokio::sync::Semaphore>,
    vapid: Vapid,
    subject: String,
    /// Device endpoints: public addresses only, unless `allow_local_endpoints`, and never
    /// following a redirect.
    endpoints: ureq::Agent,
    /// FCM and its token endpoint, which the operator configured.
    trusted: ureq::Agent,
    allow_local: bool,
    /// Hosts the operator added to the push services' (`PushConfig::hosts`).
    hosts: Vec<String>,
    fcm: Option<Fcm>,
}

struct Fcm {
    config: FcmConfig,
    key: signature::RsaKeyPair,
    /// The access token and when it stops being used; held while one is asked for, so pushes
    /// waiting on it ask once.
    token: Mutex<Option<(String, u64)>>,
}

impl Pusher {
    pub(crate) fn new(vapid: Vapid, subject: String, config: &PushConfig) -> Arc<Self> {
        let allow_local = config.allow_local_endpoints;
        let endpoints = ureq::AgentBuilder::new()
            .timeout(REQUEST_TIMEOUT)
            .redirects(0)
            .resolver(PublicOnly { allow_local })
            .build();
        let trusted = ureq::AgentBuilder::new()
            .timeout(REQUEST_TIMEOUT)
            .redirects(0)
            .build();
        let fcm = config.fcm.clone().and_then(|config| {
            let key = signature::RsaKeyPair::from_pkcs8(&config.private_key).ok()?;
            Some(Fcm {
                config,
                key,
                token: Mutex::new(None),
            })
        });
        Arc::new(Self {
            queues: Mutex::new(Queues::default()),
            senders: Arc::new(tokio::sync::Semaphore::new(SENDERS)),
            vapid,
            subject,
            endpoints,
            trusted,
            allow_local,
            hosts: config.hosts.clone(),
            fcm,
        })
    }

    pub(crate) fn public_key(&self) -> Vec<u8> {
        self.vapid.public()
    }

    pub(crate) fn offers_fcm(&self) -> bool {
        self.fcm.is_some()
    }

    /// Refuses an endpoint this server would not send to: an FCM token on a server without
    /// FCM, a Web Push URL that is not `https://` on port 443 (unless local endpoints are
    /// allowed), or keys of the wrong length.
    pub(crate) fn check(&self, endpoint: &PushEndpoint) -> Result<(), ServerError> {
        let refuse = |why: &str| {
            Err(ServerError::Refused(
                be_block_protocol::ErrorCode::InvalidRequest,
                why.to_owned(),
            ))
        };
        if endpoint.p256dh.len() != 65 || endpoint.auth.len() != 16 {
            return refuse("an endpoint's keys are a 65-byte P-256 key and a 16-byte secret");
        }
        if !be_lib_web_push::is_device_key(&endpoint.p256dh) {
            return refuse("an endpoint's key is not a point on P-256");
        }
        if endpoint.address.is_empty() || endpoint.address.len() > MAX_ADDRESS_BYTES {
            return refuse("an endpoint's address is empty or too long");
        }
        match endpoint.service {
            PushService::WebPush => {
                let Ok(url) = url::Url::parse(&endpoint.address) else {
                    return refuse("a Web Push endpoint is a URL");
                };
                let served = url.host().is_some()
                    && match self.allow_local {
                        true => matches!(url.scheme(), "https" | "http"),
                        false => {
                            url.scheme() == "https" && url.port_or_known_default() == Some(443)
                        }
                    };
                if !served {
                    return refuse("a Web Push endpoint is an https:// URL on port 443");
                }
                match url.host_str().is_some_and(|host| allowed_host(host, &self.hosts)) {
                    true => Ok(()),
                    false => refuse(
                        "this server pushes only to the known push services (FCM, Mozilla, Apple, Windows) and the hosts its operator added with --push-host",
                    ),
                }
            }
            PushService::Fcm if self.fcm.is_none() => {
                refuse("this server does not send through FCM")
            }
            PushService::Fcm => Ok(()),
        }
    }

    /// Queues `message`, a push about `block`, for each device of every delivery, behind
    /// whatever the account already waits for.
    pub(crate) fn queue(
        self: &Arc<Self>,
        store: &ServerStore,
        organization: Uuid,
        block: Uuid,
        deliveries: Vec<Delivery>,
        message: &Arc<[u8]>,
    ) {
        let mut queues = self.queues.lock().unwrap_or_else(|held| held.into_inner());
        for delivery in deliveries {
            let draining = queues.by_account.contains_key(&delivery.account);
            let mut added = 0;
            let queue = queues.by_account.entry(delivery.account).or_default();
            for (endpoint, failing) in delivery.endpoints {
                if let Some(waiting) = queue
                    .iter_mut()
                    .find(|job| job.block == block && job.endpoint.address == endpoint.address)
                {
                    waiting.message = Arc::clone(message);
                    waiting.urgent |= delivery.urgent;
                    continue;
                }
                if queue.len() >= MAX_QUEUED_PER_ACCOUNT {
                    tracing::debug!(account = %delivery.account, "dropping a push: the account's queue is full");
                    break;
                }
                queue.push_back(Job {
                    organization,
                    endpoint,
                    failing,
                    block,
                    urgent: delivery.urgent,
                    message: Arc::clone(message),
                });
                added += 1;
            }
            queues.total += added;
            if queues.total > MAX_QUEUED {
                tracing::warn!("dropping pushes: the server's push queue is full");
                queues.shed();
            }
            let empty = queues
                .by_account
                .get(&delivery.account)
                .is_none_or(VecDeque::is_empty);
            // A drain under way removes the entry itself once it finds it empty.
            if empty && !draining {
                queues.by_account.remove(&delivery.account);
            } else if !empty && !draining {
                tokio::spawn(Arc::clone(self).drain(store.clone(), delivery.account));
            }
        }
    }

    /// Sends `account`'s pushes until none waits, then lets the queue go.
    async fn drain(self: Arc<Self>, store: ServerStore, account: Uuid) {
        loop {
            let job = {
                let mut queues = self.queues.lock().unwrap_or_else(|held| held.into_inner());
                match queues
                    .by_account
                    .get_mut(&account)
                    .and_then(VecDeque::pop_front)
                {
                    Some(job) => {
                        queues.total = queues.total.saturating_sub(1);
                        job
                    }
                    None => {
                        queues.by_account.remove(&account);
                        return;
                    }
                }
            };
            let Ok(permit) = Arc::clone(&self.senders).acquire_owned().await else {
                return;
            };
            let address = job.endpoint.address.clone();
            let (organization, failing) = (job.organization, job.failing);
            let pusher = Arc::clone(&self);
            let sent = tokio::task::spawn_blocking(move || {
                let sent = pusher.send(&job.endpoint, &job.message, job.urgent);
                drop(permit);
                sent
            })
            .await;
            let noted = match sent {
                Ok(Sent::Delivered) if failing => {
                    store
                        .endpoint_answered(account, organization, address)
                        .await
                }
                Ok(Sent::Delivered) => Ok(()),
                Ok(Sent::Gone) => {
                    self.forget_queued(account, &address);
                    store.drop_endpoint(address).await
                }
                Ok(Sent::Throttled) => Ok(()),
                Ok(Sent::Failed(error)) => {
                    tracing::debug!(%error, "a push was not delivered");
                    store.endpoint_failed(account, organization, address).await
                }
                Err(error) => {
                    tracing::warn!(%error, "sending a push panicked");
                    Ok(())
                }
            };
            if let Err(error) = noted {
                tracing::warn!(%error, "recording what became of a push failed");
            }
        }
    }

    /// Drops what waits for an endpoint its service said is gone.
    fn forget_queued(&self, account: Uuid, address: &str) {
        let mut queues = self.queues.lock().unwrap_or_else(|held| held.into_inner());
        if let Some(queue) = queues.by_account.get_mut(&account) {
            let before = queue.len();
            queue.retain(|job| job.endpoint.address != address);
            let dropped = before - queue.len();
            queues.total = queues.total.saturating_sub(dropped);
        }
    }

    /// Encrypts `message` to the endpoint, padded to `PADDED_BYTES`, and sends it at high
    /// urgency when `urgent`. Blocks: run it off the runtime.
    pub(crate) fn send(&self, endpoint: &PushEndpoint, message: &[u8], urgent: bool) -> Sent {
        let body =
            match be_lib_web_push::encrypt(&endpoint.p256dh, &endpoint.auth, message, PADDED_BYTES)
            {
                Ok(body) => body,
                Err(error) => return Sent::Failed(error.to_string()),
            };
        match endpoint.service {
            PushService::WebPush => self.web_push(&endpoint.address, &body, urgent),
            PushService::Fcm => match &self.fcm {
                Some(fcm) => self.fcm(fcm, &endpoint.address, &body, urgent),
                None => Sent::Failed("FCM is not configured".into()),
            },
        }
    }

    fn web_push(&self, address: &str, body: &[u8], urgent: bool) -> Sent {
        let Ok(url) = url::Url::parse(address) else {
            return Sent::Gone;
        };
        // Checked again here: an operator may have taken a host off since it registered.
        if !url
            .host_str()
            .is_some_and(|host| allowed_host(host, &self.hosts))
        {
            return Sent::Gone;
        }
        let authorization = self.vapid.authorization(&url, &self.subject, unix_now());
        let sent = self
            .endpoints
            .post(address)
            .set("TTL", &TIME_TO_LIVE.as_secs().to_string())
            .set("Urgency", if urgent { "high" } else { "normal" })
            .set("Content-Encoding", "aes128gcm")
            .set("Content-Type", "application/octet-stream")
            .set("Authorization", &authorization)
            .send_bytes(body);
        match sent {
            Ok(_) => Sent::Delivered,
            Err(ureq::Error::Status(404 | 410, _)) => Sent::Gone,
            Err(ureq::Error::Status(429 | 503, _)) => Sent::Throttled,
            Err(error) => Sent::Failed(error.to_string()),
        }
    }

    fn fcm(&self, fcm: &Fcm, token: &str, body: &[u8], urgent: bool) -> Sent {
        let access = match self.access_token(fcm) {
            Ok(access) => access,
            Err(error) => return Sent::Failed(error),
        };
        let message = ureq::serde_json::json!({
            "message": {
                "token": token,
                "data": { "push": URL_SAFE_NO_PAD.encode(body) },
                "android": {
                    "priority": if urgent { "high" } else { "normal" },
                    "ttl": format!("{}s", TIME_TO_LIVE.as_secs()),
                },
            }
        });
        let url = format!(
            "{}/v1/projects/{}/messages:send",
            fcm.config.endpoint.trim_end_matches('/'),
            fcm.config.project_id
        );
        let sent = self
            .trusted
            .post(&url)
            .set("Authorization", &format!("Bearer {access}"))
            .send_json(message);
        match sent {
            Ok(_) => Sent::Delivered,
            Err(ureq::Error::Status(status, response)) => {
                let answer = response.into_string().unwrap_or_default();
                fcm_outcome(status, &answer)
            }
            Err(error) => Sent::Failed(error.to_string()),
        }
    }

    /// An OAuth access token for FCM, from the service account (RFC 7523), kept until a minute
    /// before it expires.
    fn access_token(&self, fcm: &Fcm) -> Result<String, String> {
        let now = unix_now();
        let mut held = fcm.token.lock().unwrap_or_else(|held| held.into_inner());
        if let Some((token, until)) = &*held
            && now < *until
        {
            return Ok(token.clone());
        }
        let assertion = fcm_assertion(&fcm.config, &fcm.key, now);
        let answer: ureq::serde_json::Value = self
            .trusted
            .post(&fcm.config.token_uri)
            .send_form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &assertion),
            ])
            .map_err(|error| format!("asking for an FCM access token failed: {error}"))?
            .into_json()
            .map_err(|error| format!("the FCM access token is {error}"))?;
        let token = answer["access_token"]
            .as_str()
            .ok_or("the token endpoint answered no access token")?
            .to_owned();
        let lasts = answer["expires_in"].as_u64().unwrap_or(3600);
        *held = Some((token.clone(), now + lasts.saturating_sub(60)));
        Ok(token)
    }
}

/// What an FCM error answer means for the endpoint: gone only when FCM names the token
/// `UNREGISTERED`, never for a 404 alone, which a wrong project also answers.
pub(crate) fn fcm_outcome(status: u16, answer: &str) -> Sent {
    let unregistered = ureq::serde_json::from_str::<ureq::serde_json::Value>(answer)
        .ok()
        .and_then(|answer| {
            answer["error"]["details"].as_array().map(|details| {
                details
                    .iter()
                    .any(|detail| detail["errorCode"] == "UNREGISTERED")
            })
        })
        .unwrap_or(false);
    match (unregistered, status) {
        (true, _) => Sent::Gone,
        (false, 429 | 503) => Sent::Throttled,
        (false, status) => Sent::Failed(format!("FCM answered {status}: {answer}")),
    }
}

/// The JWT a service account signs to ask for an access token (RFC 7523, RS256).
pub(crate) fn fcm_assertion(config: &FcmConfig, key: &signature::RsaKeyPair, now: u64) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
    let claims = ureq::serde_json::json!({
        "iss": config.client_email,
        "scope": FCM_SCOPE,
        "aud": config.token_uri,
        "iat": now,
        "exp": now + 3600,
    });
    let signed = format!("{header}.{}", URL_SAFE_NO_PAD.encode(claims.to_string()));
    let mut signature = vec![0; key.public().modulus_len()];
    key.sign(
        &signature::RSA_PKCS1_SHA256,
        &SystemRandom::new(),
        signed.as_bytes(),
        &mut signature,
    )
    .expect("signing with an RSA key that loaded does not fail");
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature))
}

/// The longest endpoint address kept: a Web Push URL or an FCM token.
pub const MAX_ADDRESS_BYTES: usize = 2048;

pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// Resolves device endpoints to public addresses only, so a client cannot have the server post
/// to loopback, a private network or a link-local metadata service.
struct PublicOnly {
    allow_local: bool,
}

impl ureq::Resolver for PublicOnly {
    fn resolve(&self, netloc: &str) -> io::Result<Vec<SocketAddr>> {
        let addresses: Vec<SocketAddr> = netloc
            .to_socket_addrs()?
            .filter(|address| self.allow_local || is_public(address.ip()))
            .collect();
        match addresses.is_empty() {
            true => Err(io::Error::other(format!(
                "{netloc} has no public address to push to"
            ))),
            false => Ok(addresses),
        }
    }
}

/// Whether `address` is a public unicast address: IPv4 outside every private, shared, reserved
/// and documentation range, or IPv6 in global unicast (`2000::/3`) outside the ranges that
/// embed an IPv4 address (6to4, Teredo) or are not routed (documentation, the IETF's
/// `2001::/23`). An IPv4-mapped address is judged as IPv4; an IPv4-compatible, NAT64 or
/// site-local one is outside `2000::/3`.
pub(crate) fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            let [a, b, ..] = address.octets();
            !(address.is_loopback()
                || address.is_private()
                || address.is_link_local()
                || address.is_unspecified()
                || address.is_broadcast()
                || address.is_documentation()
                || address.is_multicast()
                || a == 0
                || (a == 100 && (64..128).contains(&b))
                || (a == 192 && b == 0)
                || (a == 198 && (18..20).contains(&b))
                || a >= 240)
        }
        IpAddr::V6(address) => match address.to_ipv4_mapped() {
            Some(mapped) => is_public(IpAddr::V4(mapped)),
            None => {
                let [first, second, ..] = address.segments();
                (first & 0xe000) == 0x2000
                    && first != 0x2002
                    && !(first == 0x2001 && (second < 0x0200 || second == 0x0db8))
            }
        },
    }
}
