pub struct Email {
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

pub trait Delivery: Send + Sync {
    fn send(&self, email: Email);
}

pub struct LogDelivery;

impl Delivery for LogDelivery {
    fn send(&self, email: Email) {
        tracing::info!(
            recipient = email.recipient,
            subject = email.subject,
            body = email.body,
            "email delivery (log only)"
        );
    }
}

pub fn validate_origin(origin: &str) -> Result<(), String> {
    let (scheme, authority) = origin
        .split_once("://")
        .ok_or("Expected an https:// origin.")?;
    if authority.is_empty()
        || authority
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '@' | '\\'))
    {
        return Err("Use an origin without a path, query, credentials, or fragment.".into());
    }
    let local = authority == "localhost"
        || authority.starts_with("localhost:")
        || authority == "127.0.0.1"
        || authority.starts_with("127.0.0.1:")
        || authority == "[::1]"
        || authority.starts_with("[::1]:");
    if scheme != "https" && !(scheme == "http" && local) {
        return Err("Use HTTPS, or HTTP on localhost for development.".into());
    }
    Ok(())
}
