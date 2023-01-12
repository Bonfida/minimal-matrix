use std::time::SystemTime;

pub fn current_time() -> u64 {
    let now = SystemTime::now();
    now.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn generate_tx_id(message: &str) -> String {
    let now = current_time();
    let mut m = sha1_smol::Sha1::new();
    m.update(format!("{}{}", now, message).as_bytes());
    m.digest().to_string()
}
