//! A fake OpenID Connect provider for be-server's tests, on loopback: it serves discovery,
//! its keys and a token endpoint, and signs real RS256 ID tokens. A test plays the browser and
//! the person: it reads the authorization URL the server redirects to, and `authorize` answers
//! it as the provider would, with the grant the test describes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine;
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreRsaPrivateSigningKey,
};
use openidconnect::{JsonWebKeyId, PrivateSigningKey};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::oidc::Provider;

/// The client the fake provider knows.
pub const CLIENT_ID: &str = "be3-test-client";
pub const CLIENT_SECRET: &str = "be3-test-secret";

// Two RSA keys made for these tests alone: the provider signs with the first and publishes it;
// the second is a stranger's, for a token the provider's keys don't verify.
const PROVIDER_KEY: &str = r"-----BEGIN RSA PRIVATE KEY-----
MIIEogIBAAKCAQEAs1ir5F50BVG063otIy/1gY+qgnbGHOOOQIIdRmD5KHi2lyV/
cxMJkANcwYXK1ZtcnlBh57+pbJHFfYeWLRq+03nYmuKqKObiQJxeIT5HlTC7SjqQ
wu7ImY+FEg3cOE37ZI31wakNy6/FesTWnLy6ChZ029o+SD5LEMzRok7GSBs22LTD
0jsDzYMu3UavJiDwsIMvz7GzASu42Jq3tLoxCn0Y2djX10+r1nnxIAILfOwIPsZM
Eqdo9/P1EvO8KkzQmmgHRW6rQutlSyAAFQ/MyCaxf3gLXsMzDRbQk0cttQKR5zV6
nPfugNQk5SiTiFxHzWOvz4Ep6ZdAn4J9F9F5MQIDAQABAoIBAAmjBp1/S8mrYcbT
alB9N7pt/1ICUtPjkFW8CBmrhUfc8Ka/f5Et4TIpVBcb+tGuhvQEyrX6sP5h+NQt
cG7dBIr1faOY8V7ARUEGhfJQV+VW2IF9EyE+Vm4V7acSMpad7hexnxZ4YILk334g
zI1ggLihC0dy+yzDmCH/d3Y6uGHQaXesta6OlM7OxaHbaecNwtUndSZroNQRYfd4
lf1RhBC+i7XUzM43WkGIs+Om5iTzlZG3e045CdQV7eCQeRTwZjn7HUNWMvC0Ip0C
bABITAxZDss/UrfGlKqJDiZSWoLSER+7NPPOWkxfvQLCa7CQUcGG5EPwTl7+AfMV
qUf8n5cCgYEA5E8x2VJdcIDMWICNvEFSNmb4+d0dIdIeTggivFEkGF1Itfe7zaVi
cckQ1B3ttfoST+YDWLOrmJEcJMn3zteO6Og6/y4n3CkSRMCDPQ4f1pG70w0kNqXx
r7Enhz9LfU/nHE3mnRO4U0Gvb8N9miuIq+apIUOShsKmuNrHwzOGiN8CgYEAyRk4
FeqHONZSjqNOpd/aL4Ql+g4AwemTZN0mfgf9FEJxKeX3MQN/o52bUNCWsXuZvvCx
beLBzHeUrmi6FwVbpNxHVHZBzyT3F18HQtlDLMwEywO2J7vi5EqLUMnS2IwHzI7s
v5WVq9dd5D0m7MmLEjnGBYbD+nmapNcdDsEBb+8CgYAwWKzfrldnGX8sRpy8/sAg
ERIsJyyiGtKcB5EDe9M+7ZIWJo24cIs2OTO9F1FdNcVm8fhHclSmDCR/jd1SCceT
fonvu6BpOLFLqdCgvzryvw2dFa1TfTR+1e7e7wJXWPJBxiVNof1odAJokmconItm
i4PJLXv72YBgqMOgjGqy1QKBgAX2SHJl5KJrwEDYWKGUVGyqN1IFjkIxgZww7k44
QGmUWrSKS9zZJLSeHJe1x3H1nyZJDomaenJsbS0Kj9HU59Wobjjz1Jmml+jKeXWi
6Vnvpoi6UHE3Ic/w4daAVU70P9rxtdUsQya4GOGGapwEkan1qohJaZAIgOw4MHVa
hYdjAoGAF1hO57NSA3O3o8znPbz8vQMmEZ0UsKMSAN3dp/czKCtpDcmxSJMb3R09
O6Caiyuz+NzvBSGwf/QWOMZhqfQZgrATJhCgPK2ojlIHoR6hvSss6zGc1Qg+J018
yAcLqFsvogek5joi+pvvkGLozAvTicr7dPgBODScYlXFhgzmchY=
-----END RSA PRIVATE KEY-----";
const STRANGER_KEY: &str = r"-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEAkVrhmwu37d8D0+1NjpPpkwjYpa83vuM4RQGN3Z84Y9XuZeT/
fINkkVkDlPPuCa3IArwV/Bx6FMENBV/ykPxC8z2ADx+7/OWZOCloH5tEgKHnoACK
kpyI/ytdgk9fhJ7QGgzJtpoC9MgRmqTPcvcQ+BUprgnTZ5hsoh/r9WZZ3u3VGiZm
xhAhnemHZq2uJZ91OjPVQha/xtOm2G4JCJeSMJtwvgF1CYmbkxGB9Ej9GwfVjVCb
v5ar48R6UjGTRFjl6ZmFN5ZqK2H0IAN53HNMMpzBaMaiu5G7onND4/5OykVVoMdw
Rp05b+AMq7/lhsrM4WsOE9aQXZvsybGuStYLzwIDAQABAoIBADasKuChI0EXGb78
aqgezYPPF4IWak7j81XNVFmQWmr+3Ck6XbitSUX8Z8L+j0HXujwBF/nYkji9NlMX
uEq9f8czEjheFNz2VU/RD0eYJMeHXnKJ0ZV8yHDGFGl/xmtBOwAZYEPNISCGIkz7
AcZ51MmjWCaMrpYwOGgwvpVytooJdOlo9cuJyqPer6Y3Ylk9Ap6KvSHUsKNQgyEH
PJZxy0X3dyfEgy/VvOuLBMPF8A/5UYeO1luRFDVH/VWpYwmzggxQsXpW/AvjLV3B
uYyvYbeEjcKrB2UeSB8wOjKzgSg97YVNd62XkNmV9tQbu7KqhW9+JLq5kJa0E9Z0
+q87S40CgYEAxqxPpF2PjsI1Am/fwVUrEiwf8xrI/FxjINzosu3qVVT1o2VlYfUR
Z6LYgYhexlqBzBB8CPu+Yn+38XdoeFYdQdrQJLc18RCZow5JhHfQS3Emh/3tu9dz
uHdQ+XXYqUn/pyhJU2Tjleo50DXdLb1roksP8CRP6JMJcwTExfi4JSsCgYEAu0wK
qZUbD9OglbdltYRfzhLjMiO2W6mcbrl2Ss2hWCxr0LAPwsHgf6dhsVspV4inC9ym
kVvb0ep5BbAsQB/NDTb8NEacEdNF4Z26ZuYgB/1y8zdmuKpDxTnzMfKrmJhMJMFq
4oWF39kYo+gUmZe9GWukDhLadJbsx5RzA97Xae0CgYAjjx/KGXziXlxCjWeNIQNO
g1tMldS3ZonPjfqQXAj/MO+uSS22NLjixcDenlvoQLbOoXyx1u0ikB2KOIcYUsF1
0gZDLA+ZkBEPoDUEYrWDa0MsQATXBB8QOzrhC3i2BCqf7pQBOyOI551laN6nmymy
qKC3qKgsxnSAg6/8hyHedQKBgQCfIfKFNvxxK/tqeRHfcYL80atbNGCB7M/Oe6D3
2VJWWheYjZ42eQdHLm2ghLXRe3eCaozTP9q6tZ7xqEdQjDRqH+eUetGri3f6Q0rW
lntsjniWEOIjz/YYoi3wqCAHXI6FRBiOMagXEpsO1+Z4wSWXbY9LrdUR7hFrWMOH
7FPV2QKBgGDndSk7qTb9EkLVGObnZzJlg43fJGzEG3fdw+eOvr6t11TRB/fPp/0F
6SLCxoHBmt511jb8s3iG1sEbMRSeuoxS98ybm2pGLZ1av/f1eLe+80hhwiUxpyBh
AdN5dnJlhDgcHBNL+eSjbFWa5GM85On2ZgZ1M3Kf8jBib/ATpc0W
-----END RSA PRIVATE KEY-----";

/// What the person's sign-in at the provider gives.
#[derive(Clone)]
pub struct Grant {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: String,
    /// The token's `aud`, if not the client's.
    pub audience: Option<String>,
    /// The token's nonce, if not the one the authorization request asked for.
    pub nonce: Option<String>,
    /// Signed with a key the provider doesn't publish.
    pub stranger_signs: bool,
    /// Not signed at all: `alg` is `none`.
    pub unsigned: bool,
    /// Expired before it was sent.
    pub expired: bool,
    /// How long ago the person signed in at the provider (`auth_time`).
    pub signed_in_ago: i64,
    /// The PKCE challenge the provider holds the code to, if not the request's.
    pub challenge: Option<String>,
}

impl Grant {
    pub fn new(sub: &str) -> Self {
        Self {
            sub: sub.into(),
            email: None,
            email_verified: false,
            name: "Ada Provider".into(),
            audience: None,
            nonce: None,
            stranger_signs: false,
            unsigned: false,
            expired: false,
            signed_in_ago: 0,
            challenge: None,
        }
    }

    pub fn verified(mut self, email: &str) -> Self {
        self.email = Some(email.into());
        self.email_verified = true;
        self
    }
}

struct Issued {
    grant: Grant,
    nonce: String,
    challenge: String,
}

pub struct FakeProvider {
    pub issuer: String,
    codes: Arc<Mutex<HashMap<String, Issued>>>,
    /// How many times its discovery document was read.
    pub discoveries: Arc<AtomicUsize>,
    /// While set, discovery answers an error.
    pub discovery_fails: Arc<AtomicBool>,
}

impl FakeProvider {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let codes: Arc<Mutex<HashMap<String, Issued>>> = Arc::default();
        let discoveries: Arc<AtomicUsize> = Arc::default();
        let discovery_fails: Arc<AtomicBool> = Arc::default();
        let (serving, held) = (issuer.clone(), codes.clone());
        let (counting, failing) = (discoveries.clone(), discovery_fails.clone());
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (issuer, codes) = (serving.clone(), held.clone());
                let (counting, failing) = (counting.clone(), failing.clone());
                tokio::spawn(async move {
                    let Some((path, body)) = read(&mut stream).await else {
                        return;
                    };
                    let (status, answer) = match path.as_str() {
                        "/.well-known/openid-configuration" => {
                            counting.fetch_add(1, Ordering::SeqCst);
                            match failing.load(Ordering::SeqCst) {
                                true => (500, "{}".to_owned()),
                                false => answer(&issuer, &codes, &path, &body),
                            }
                        }
                        _ => answer(&issuer, &codes, &path, &body),
                    };
                    let response = format!(
                        "HTTP/1.1 {status} Answered\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                        answer.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        Self {
            issuer,
            codes,
            discoveries,
            discovery_fails,
        }
    }

    /// The provider as the operator would configure it.
    pub fn provider(&self, id: &str, authoritative_email: bool) -> Provider {
        Provider {
            id: id.into(),
            name: "Test provider".into(),
            issuer: self.issuer.clone(),
            client_id: CLIENT_ID.into(),
            client_secret: CLIENT_SECRET.into(),
            authoritative_email,
        }
    }

    /// Answers the authorization URL the server sent the browser to, as the provider does once
    /// the person signs in: the callback path and query the browser is sent back with.
    pub fn authorize(&self, url: &str, grant: Grant) -> String {
        let url = url::Url::parse(url).unwrap();
        let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
        assert_eq!(query["client_id"], CLIENT_ID);
        assert_eq!(query["response_type"], "code");
        assert_eq!(query["code_challenge_method"], "S256");
        let code = uuid::Uuid::new_v4().simple().to_string();
        self.codes.lock().unwrap().insert(
            code.clone(),
            Issued {
                nonce: query["nonce"].clone(),
                challenge: grant
                    .challenge
                    .clone()
                    .unwrap_or_else(|| query["code_challenge"].clone()),
                grant,
            },
        );
        let redirect = url::Url::parse(&query["redirect_uri"]).unwrap();
        format!(
            "{}?code={code}&state={}",
            redirect.path(),
            crate::portal::encode(&query["state"])
        )
    }
}

fn key(pem: &str) -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(pem, Some(JsonWebKeyId::new("test-key".into()))).unwrap()
}

fn answer(
    issuer: &str,
    codes: &Mutex<HashMap<String, Issued>>,
    path: &str,
    body: &str,
) -> (u16, String) {
    match path {
        "/.well-known/openid-configuration" => (
            200,
            serde_json::json!({
                "issuer": issuer,
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint": format!("{issuer}/token"),
                "jwks_uri": format!("{issuer}/keys"),
                "response_types_supported": ["code"],
                "subject_types_supported": ["public"],
                "id_token_signing_alg_values_supported": ["RS256"],
                "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
            })
            .to_string(),
        ),
        "/keys" => (
            200,
            serde_json::to_string(&CoreJsonWebKeySet::new(vec![
                key(PROVIDER_KEY).as_verification_key(),
            ]))
            .unwrap(),
        ),
        "/token" => {
            let form: HashMap<String, String> = url::form_urlencoded::parse(body.as_bytes())
                .into_owned()
                .collect();
            let Some(issued) = form
                .get("code")
                .and_then(|code| codes.lock().unwrap().remove(code))
            else {
                return (400, r#"{"error":"invalid_grant"}"#.into());
            };
            // PKCE: only the verifier behind the authorization request's challenge redeems it.
            let verifier = form.get("code_verifier").cloned().unwrap_or_default();
            let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(Sha256::digest(verifier.as_bytes()));
            if challenge != issued.challenge {
                return (400, r#"{"error":"invalid_grant"}"#.into());
            }
            let grant = issued.grant;
            let now = crate::store::portal::now();
            let issued_at = match grant.expired {
                true => now - 1200,
                false => now,
            };
            let claims: CoreIdTokenClaims = serde_json::from_value(serde_json::json!({
                "iss": issuer,
                "aud": [grant.audience.clone().unwrap_or_else(|| CLIENT_ID.into())],
                "exp": issued_at + 600,
                "iat": issued_at,
                "auth_time": now - grant.signed_in_ago,
                "sub": grant.sub,
                "nonce": grant.nonce.clone().unwrap_or(issued.nonce),
                "email": grant.email,
                "email_verified": grant.email_verified,
                "name": grant.name,
            }))
            .unwrap();
            let signer = key(match grant.stranger_signs {
                true => STRANGER_KEY,
                false => PROVIDER_KEY,
            });
            let token = match grant.unsigned {
                true => {
                    let part = |json: String| {
                        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
                    };
                    format!(
                        "{}.{}.",
                        part(r#"{"alg":"none","typ":"JWT"}"#.into()),
                        part(serde_json::to_string(&claims).unwrap())
                    )
                }
                false => CoreIdToken::new(
                    claims,
                    &signer,
                    CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
                    None,
                    None,
                )
                .unwrap()
                .to_string(),
            };
            (
                200,
                serde_json::json!({
                    "access_token": "unused",
                    "token_type": "Bearer",
                    "expires_in": 600,
                    "id_token": token,
                })
                .to_string(),
            )
        }
        _ => (404, "{}".into()),
    }
}

/// The path and body of one HTTP request.
async fn read(stream: &mut tokio::net::TcpStream) -> Option<(String, String)> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let read = stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let Some((head, body)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let length = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            })
            .unwrap_or(0);
        if body.len() < length {
            continue;
        }
        let path = head.split_whitespace().nth(1)?.to_owned();
        let path = path.split('?').next().unwrap_or_default().to_owned();
        return Some((path, body[..length].to_owned()));
    }
}
