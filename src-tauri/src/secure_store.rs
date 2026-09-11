use keyring::Entry;
use serde_json::Value;

const SERVICE_NAME: &str = "com.dannymaaz.switchcraft";

fn entry(key: &str) -> Result<Entry, String> {
    Entry::new(SERVICE_NAME, key)
        .map_err(|e| format!("Secure credential store is unavailable: {e}"))
}

pub fn put_text(key: &str, value: &str) -> Result<(), String> {
    entry(key)?
        .set_password(value)
        .map_err(|e| format!("Could not save secret in the OS credential store: {e}"))
}

pub fn get_text(key: &str) -> Result<String, String> {
    entry(key)?
        .get_password()
        .map_err(|e| format!("Could not read secret from the OS credential store: {e}"))
}

pub fn put_json(key: &str, value: &Value) -> Result<(), String> {
    let serialized = serde_json::to_string(value)
        .map_err(|e| format!("Could not serialize credentials: {e}"))?;
    put_text(key, &serialized)
}

pub fn get_json(key: &str) -> Result<Value, String> {
    let raw = get_text(key)?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Stored credentials are not valid JSON: {e}"))
}

pub fn delete(key: &str) {
    if let Ok(item) = entry(key) {
        let _ = item.delete_credential();
    }
}

pub fn account_key(account_id: &str) -> String {
    format!("account:{account_id}")
}

pub fn recovery_key(platform: &str, slot: &str) -> String {
    format!("recovery:{platform}:{slot}")
}
