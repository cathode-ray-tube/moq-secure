pub fn time_only_hhmmss() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() % 86_400;
    let hh = secs / 3600;
    let mm = (secs % 3600) / 60;
    let ss = secs % 60;
    format!("{:02}:{:02}:{:02}", hh, mm, ss)
}

pub fn gen_u8_random() -> u8 {
    let mut b = [0u8; 1];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b[0]
}

pub fn gen_hex_or_b64_aead_32_bytes_hex() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    hex::encode(b)
}

pub fn gen_hex_signing_private_seed_32() -> String {
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    hex::encode(seed)
}

pub fn ensure_tracking_name(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        "default".to_string()
    } else {
        t.to_string()
    }
}
