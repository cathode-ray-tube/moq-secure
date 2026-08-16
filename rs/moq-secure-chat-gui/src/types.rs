use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub struct SubscriptionParams {
    pub track: String,
    pub publisher_key_id: u8,
    pub publisher_aead_key: String,
    pub publisher_signing_public_key: String,
}

#[derive(Clone, Debug)]
pub struct IncomingMessage {
    pub track: String,
    pub ts: String,
    pub plaintext: String,
}
