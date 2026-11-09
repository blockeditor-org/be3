//! Sending pushes: Web Push (RFC 8030) with its payloads encrypted to the device (RFC 8291) and
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
    aead, agreement, hmac,
    rand::{SecureRandom, SystemRandom},
    signature::{self, KeyPair},
};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use crate::{ServerError, store::ServerStore};

/// The record size every push declares (RFC 8188): one record holds the whole payload.
const RECORD_SIZE: u32 = 4096;
/// The header before the ciphertext: salt, record size, key id length, the sender's key.
pub const HEADER_BYTES: usize = 16 + 4 + 1 + 65;
/// How long a push service keeps a push for a device that is away.
const TIME_TO_LIVE: Duration = Duration::from_secs(24 * 60 * 60);
/// How long one request to a push service may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a VAPID token is valid: RFC 8292 allows at most a day.
const VAPID_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);
const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
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
}

impl PushConfig {
    pub const DEFAULT: Self = Self {
        allow_local_endpoints: false,
        fcm: None,
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

/// Encrypts `plain` to a device's keys (RFC 8291, `aes128gcm`): a new sender key pair and salt
/// for each push.
pub fn encrypt(ua_public: &[u8], auth: &[u8], plain: &[u8]) -> Result<Vec<u8>, ServerError> {
    let random = SystemRandom::new();
    let unusable = || {
        ServerError::Refused(
            be_block_protocol::ErrorCode::InvalidRequest,
            "the endpoint's keys are not a P-256 public key and a 16-byte secret".into(),
        )
    };
    if auth.len() != 16 {
        return Err(unusable());
    }
    let private = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &random)
        .map_err(|_| unusable())?;
    let as_public = private.compute_public_key().map_err(|_| unusable())?;
    let mut salt = [0; 16];
    random.fill(&mut salt).map_err(|_| unusable())?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, ua_public);
    agreement::agree_ephemeral(private, &peer, |secret| {
        seal(secret, as_public.as_ref(), ua_public, auth, &salt, plain)
    })
    .map_err(|_| unusable())
}

/// RFC 8291's encryption from the shared secret on: the content encryption key and nonce, then
/// the header and the one record. `encrypt` with every random input given.
pub fn seal(
    ecdh_secret: &[u8],
    as_public: &[u8],
    ua_public: &[u8],
    auth: &[u8],
    salt: &[u8; 16],
    plain: &[u8],
) -> Vec<u8> {
    let (key, nonce) = derive(ecdh_secret, as_public, ua_public, auth, salt);
    let mut record = plain.to_vec();
    record.push(2);
    key.seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut record)
        .expect("a push fits one record");
    let mut body = Vec::with_capacity(HEADER_BYTES + record.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(as_public.len() as u8);
    body.extend_from_slice(as_public);
    body.extend_from_slice(&record);
    body
}

/// The sender's public key in a push `seal` made, which the device agrees on a secret with.
pub fn sender_key(body: &[u8]) -> Option<&[u8]> {
    let length = usize::from(*body.get(20)?);
    body.get(21..21 + length)
}

/// Decrypts a push `seal` made, given the secret the device agreed on with `sender_key`: what a
/// device does on receiving one.
pub fn open(ecdh_secret: &[u8], ua_public: &[u8], auth: &[u8], body: &[u8]) -> Option<Vec<u8>> {
    let salt: [u8; 16] = body.get(..16)?.try_into().ok()?;
    let as_public = sender_key(body)?;
    let start = 21 + as_public.len();
    let (key, nonce) = derive(ecdh_secret, as_public, ua_public, auth, &salt);
    let mut record = body.get(start..)?.to_vec();
    let plain = key
        .open_in_place(nonce, aead::Aad::empty(), &mut record)
        .ok()?;
    let end = plain.iter().rposition(|byte| *byte != 0)?;
    (plain[end] == 2).then(|| plain[..end].to_vec())
}

fn derive(
    ecdh_secret: &[u8],
    as_public: &[u8],
    ua_public: &[u8],
    auth: &[u8],
    salt: &[u8; 16],
) -> (aead::LessSafeKey, aead::Nonce) {
    let mac = |key: &[u8], parts: &[&[u8]]| {
        let mut context = hmac::Context::with_key(&hmac::Key::new(hmac::HMAC_SHA256, key));
        for part in parts {
            context.update(part);
        }
        context.sign()
    };
    let prk_key = mac(auth, &[ecdh_secret]);
    let ikm = mac(
        prk_key.as_ref(),
        &[b"WebPush: info\0", ua_public, as_public, &[1]],
    );
    let prk = mac(salt, &[ikm.as_ref()]);
    let cek = mac(prk.as_ref(), &[b"Content-Encoding: aes128gcm\0", &[1]]);
    let nonce = mac(prk.as_ref(), &[b"Content-Encoding: nonce\0", &[1]]);
    let key = aead::UnboundKey::new(&aead::AES_128_GCM, &cek.as_ref()[..16])
        .expect("16 bytes is an AES-128 key");
    let nonce = aead::Nonce::try_assume_unique_for_key(&nonce.as_ref()[..12])
        .expect("12 bytes is a GCM nonce");
    (aead::LessSafeKey::new(key), nonce)
}

/// What became of one push.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Sent {
    Delivered,
    /// The service says the device is gone (Web Push 404 or 410, FCM `UNREGISTERED`): the
    /// endpoint is dropped.
    Gone,
    Failed(String),
}

/// Pushes waiting for one account's devices, past which more are dropped.
const MAX_QUEUED_PER_ACCOUNT: usize = 256;

/// One push waiting to go.
struct Job {
    endpoint: PushEndpoint,
    message: Arc<[u8]>,
}

/// Sends pushes off the runtime's threads. Each account's go one at a time, in the order they
/// were queued and its devices in the order they registered, so a device whose service is slow
/// or never answers delays only its own account's pushes.
pub(crate) struct Pusher {
    queues: Mutex<HashMap<Uuid, VecDeque<Job>>>,
    vapid: Vapid,
    subject: String,
    /// Device endpoints: public addresses only, unless `allow_local_endpoints`, and never
    /// following a redirect.
    endpoints: ureq::Agent,
    /// FCM and its token endpoint, which the operator configured.
    trusted: ureq::Agent,
    allow_local: bool,
    fcm: Option<Fcm>,
}

struct Fcm {
    config: FcmConfig,
    key: signature::RsaKeyPair,
    /// The access token and when it stops being used.
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
            queues: Mutex::new(HashMap::new()),
            vapid,
            subject,
            endpoints,
            trusted,
            allow_local,
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
    /// FCM, a Web Push URL that is not `https://` (unless local endpoints are allowed), or keys
    /// of the wrong length.
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
        if endpoint.address.is_empty() || endpoint.address.len() > MAX_ADDRESS_BYTES {
            return refuse("an endpoint's address is empty or too long");
        }
        match endpoint.service {
            PushService::WebPush => {
                let Ok(url) = url::Url::parse(&endpoint.address) else {
                    return refuse("a Web Push endpoint is a URL");
                };
                match (url.scheme(), self.allow_local) {
                    ("https", _) | ("http", true) if url.host().is_some() => Ok(()),
                    _ => refuse("a Web Push endpoint is an https:// URL"),
                }
            }
            PushService::Fcm if self.fcm.is_none() => {
                refuse("this server does not send through FCM")
            }
            PushService::Fcm => Ok(()),
        }
    }

    /// Queues `message` for each of `account`'s `endpoints`, sending it behind whatever the
    /// account already waits for; an endpoint the service says is gone is dropped from
    /// `store`, with what else waits for it.
    pub(crate) fn queue(
        self: &Arc<Self>,
        store: &ServerStore,
        account: Uuid,
        endpoints: Vec<PushEndpoint>,
        message: &Arc<[u8]>,
    ) {
        let mut queues = self.queues.lock().unwrap_or_else(|held| held.into_inner());
        let draining = queues.contains_key(&account);
        let queue = queues.entry(account).or_default();
        for endpoint in endpoints {
            if queue.len() >= MAX_QUEUED_PER_ACCOUNT {
                tracing::debug!(%account, "dropping a push: the account's queue is full");
                break;
            }
            queue.push_back(Job {
                endpoint,
                message: Arc::clone(message),
            });
        }
        if !draining {
            tokio::spawn(Arc::clone(self).drain(store.clone(), account));
        }
    }

    /// Sends `account`'s pushes until none waits, then lets the queue go.
    async fn drain(self: Arc<Self>, store: ServerStore, account: Uuid) {
        loop {
            let job = {
                let mut queues = self.queues.lock().unwrap_or_else(|held| held.into_inner());
                match queues.get_mut(&account).and_then(VecDeque::pop_front) {
                    Some(job) => job,
                    None => {
                        queues.remove(&account);
                        return;
                    }
                }
            };
            let address = job.endpoint.address.clone();
            let pusher = Arc::clone(&self);
            let sent =
                tokio::task::spawn_blocking(move || pusher.send(&job.endpoint, &job.message))
                    .await;
            match sent {
                Ok(Sent::Delivered) => {}
                Ok(Sent::Gone) => {
                    if let Some(queue) = self
                        .queues
                        .lock()
                        .unwrap_or_else(|held| held.into_inner())
                        .get_mut(&account)
                    {
                        queue.retain(|job| job.endpoint.address != address);
                    }
                    if let Err(error) = store.drop_endpoint(address).await {
                        tracing::warn!(%error, "dropping a push endpoint failed");
                    }
                }
                Ok(Sent::Failed(error)) => tracing::debug!(%error, "a push was not delivered"),
                Err(error) => tracing::warn!(%error, "sending a push panicked"),
            }
        }
    }

    /// Encrypts `message` to the endpoint and sends it. Blocks: run it off the runtime.
    pub(crate) fn send(&self, endpoint: &PushEndpoint, message: &[u8]) -> Sent {
        let body = match encrypt(&endpoint.p256dh, &endpoint.auth, message) {
            Ok(body) => body,
            Err(error) => return Sent::Failed(error.to_string()),
        };
        match endpoint.service {
            PushService::WebPush => self.web_push(&endpoint.address, &body),
            PushService::Fcm => match &self.fcm {
                Some(fcm) => self.fcm(fcm, &endpoint.address, &body),
                None => Sent::Failed("FCM is not configured".into()),
            },
        }
    }

    fn web_push(&self, address: &str, body: &[u8]) -> Sent {
        let Ok(url) = url::Url::parse(address) else {
            return Sent::Gone;
        };
        let authorization = self.vapid.authorization(&url, &self.subject, unix_now());
        let sent = self
            .endpoints
            .post(address)
            .set("TTL", &TIME_TO_LIVE.as_secs().to_string())
            .set("Urgency", "high")
            .set("Content-Encoding", "aes128gcm")
            .set("Content-Type", "application/octet-stream")
            .set("Authorization", &authorization)
            .send_bytes(body);
        match sent {
            Ok(_) => Sent::Delivered,
            Err(ureq::Error::Status(404 | 410, _)) => Sent::Gone,
            Err(error) => Sent::Failed(error.to_string()),
        }
    }

    fn fcm(&self, fcm: &Fcm, token: &str, body: &[u8]) -> Sent {
        let access = match self.access_token(fcm) {
            Ok(access) => access,
            Err(error) => return Sent::Failed(error),
        };
        let message = ureq::serde_json::json!({
            "message": {
                "token": token,
                "data": { "push": URL_SAFE_NO_PAD.encode(body) },
                "android": {
                    "priority": "high",
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
            Err(ureq::Error::Status(404, _)) => Sent::Gone,
            Err(ureq::Error::Status(status, response)) => {
                let answer = response.into_string().unwrap_or_default();
                match answer.contains("UNREGISTERED") {
                    true => Sent::Gone,
                    false => Sent::Failed(format!("FCM answered {status}: {answer}")),
                }
            }
            Err(error) => Sent::Failed(error.to_string()),
        }
    }

    /// An OAuth access token for FCM, from the service account (RFC 7523), kept until a minute
    /// before it expires.
    fn access_token(&self, fcm: &Fcm) -> Result<String, String> {
        let now = unix_now();
        if let Some((token, until)) = &*fcm.token.lock().unwrap_or_else(|held| held.into_inner())
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
        *fcm.token.lock().unwrap_or_else(|held| held.into_inner()) =
            Some((token.clone(), now + lasts.saturating_sub(60)));
        Ok(token)
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

fn unix_now() -> u64 {
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
                let first = address.segments()[0];
                !(address.is_loopback()
                    || address.is_unspecified()
                    || address.is_multicast()
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80
                    || first == 0x2001 && address.segments()[1] == 0x0db8
                    || first == 0x0064 && address.segments()[1] == 0xff9b)
            }
        },
    }
}
