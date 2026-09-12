use keyring::Entry;
use serde_json::Value;

const SERVICE_NAME: &str = "com.dannymaaz.switchcraft";

fn entry_for(service: &str, account: &str) -> Result<Entry, String> {
    Entry::new(service, account).map_err(|e| format!("Secure credential store is unavailable: {e}"))
}

fn entry(key: &str) -> Result<Entry, String> {
    entry_for(SERVICE_NAME, key)
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

pub fn try_get_text(key: &str) -> Result<Option<String>, String> {
    try_get_external_text(SERVICE_NAME, key)
}

pub fn put_json(key: &str, value: &Value) -> Result<(), String> {
    let serialized = serde_json::to_string(value)
        .map_err(|e| format!("Could not serialize credentials: {e}"))?;
    put_text(key, &serialized)
}

pub fn get_json(key: &str) -> Result<Value, String> {
    let raw = get_text(key)?;
    serde_json::from_str(&raw).map_err(|e| format!("Stored credentials are not valid JSON: {e}"))
}

pub fn try_get_json(key: &str) -> Result<Option<Value>, String> {
    let Some(raw) = try_get_text(key)? else {
        return Ok(None);
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("Stored credentials are not valid JSON: {e}"))
}

pub fn delete(key: &str) {
    if let Ok(item) = entry(key) {
        let _ = item.delete_credential();
    }
}

/// Read an entry owned by a supported external client without exposing it to the webview.
/// `NoEntry` is a normal state and is returned as `Ok(None)`.
pub fn try_get_external_text(service: &str, account: &str) -> Result<Option<String>, String> {
    let item = entry_for(service, account)?;
    match item.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!(
            "Could not read credential store entry for service '{service}': {e}"
        )),
    }
}

/// Write an entry using the external client's documented service/account contract.
pub fn put_external_text(service: &str, account: &str, value: &str) -> Result<(), String> {
    entry_for(service, account)?
        .set_password(value)
        .map_err(|e| format!("Could not write credential store entry for service '{service}': {e}"))
}

/// Delete an external credential. Missing entries are treated as success.
pub fn delete_external(service: &str, account: &str) -> Result<(), String> {
    let item = entry_for(service, account)?;
    match item.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!(
            "Could not delete credential store entry for service '{service}': {e}"
        )),
    }
}

pub fn account_key(account_id: &str) -> String {
    format!("account:{account_id}")
}

pub fn recovery_key(platform: &str, slot: &str) -> String {
    format!("recovery:{platform}:{slot}")
}

pub fn recovery_manifest_key(platform: &str) -> String {
    format!("recovery:{platform}:manifest")
}
