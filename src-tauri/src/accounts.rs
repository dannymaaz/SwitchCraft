use crate::atomic_fs;
use crate::providers::{self, Platform, PlannedWrite};
use crate::secure_store;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretState {
    Secure,
    LegacyPlaintext,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    #[serde(default)]
    pub plan_name: String,
    #[serde(default)]
    pub five_hour_remaining_pct: Option<u8>,
    #[serde(default)]
    pub weekly_remaining_pct: Option<u8>,
    #[serde(default)]
    pub five_hour_reset_at: Option<String>,
    #[serde(default)]
    pub weekly_reset_at: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub email: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub usage: Option<UsageInfo>,
    pub security_state: SecretState,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccount {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
    // v1.0 compatibility only. Successful migration removes this field from disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<Value>,
    pub is_active: bool,
    pub created_at: String,
    #[serde(default)]
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AccountStore {
    #[serde(default)]
    pub accounts: Vec<StoredAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecoverySnapshot {
    existed: bool,
    content: Option<String>,
    captured_at: String,
}

impl StoredAccount {
    fn view(&self) -> Account {
        let security_state = if self.secret_ref.is_some() && self.credentials.is_none() {
            SecretState::Secure
        } else if self.credentials.is_some() {
            SecretState::LegacyPlaintext
        } else {
            SecretState::Missing
        };

        Account {
            id: self.id.clone(),
            name: self.name.clone(),
            platform: self.platform.clone(),
            email: self.email.clone(),
            is_active: self.is_active,
            created_at: self.created_at.clone(),
            usage: self.usage.clone(),
            security_state,
            targets: providers::supported_targets(&self.platform),
        }
    }
}

fn switchcraft_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    let dir = home.join(".switchcraft");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn store_path() -> Result<PathBuf, String> {
    Ok(switchcraft_dir()?.join("accounts.json"))
}

fn load_store_raw() -> Result<AccountStore, String> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(AccountStore::default());
    }

    let data = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read SwitchCraft metadata: {e}"))?;
    serde_json::from_str(&data).map_err(|e| {
        format!(
            "SwitchCraft metadata at {} is corrupted. No data was overwritten: {e}",
            path.display()
        )
    })
}

fn save_store(store: &AccountStore) -> Result<(), String> {
    let path = store_path()?;
    let data = serde_json::to_vec_pretty(store)
        .map_err(|e| format!("Could not serialize SwitchCraft metadata: {e}"))?;
    atomic_fs::atomic_write(&path, &data)
}

fn migrate_legacy_credentials(store: &mut AccountStore) -> bool {
    let mut changed = false;

    for account in &mut store.accounts {
        if account.secret_ref.is_some() || account.credentials.is_none() {
            continue;
        }

        let key = secure_store::account_key(&account.id);
        if let Some(credentials) = account.credentials.as_ref() {
            if secure_store::put_json(&key, credentials).is_ok() {
                account.secret_ref = Some(key);
                account.credentials = None;
                changed = true;
            }
        }
    }

    changed
}

fn load_store() -> Result<AccountStore, String> {
    let mut store = load_store_raw()?;
    if migrate_legacy_credentials(&mut store) {
        save_store(&store)?;
    }
    Ok(store)
}

fn load_credentials(account: &StoredAccount) -> Result<Value, String> {
    if let Some(key) = account.secret_ref.as_deref() {
        return secure_store::get_json(key);
    }

    if let Some(credentials) = account.credentials.as_ref() {
        return Ok(credentials.clone());
    }

    Err(format!(
        "The secure credentials for '{}' are missing. Import this account again.",
        account.name
    ))
}

fn save_recovery_snapshot(platform: &Platform, write: &PlannedWrite, previous: Option<&[u8]>) -> Result<(), String> {
    let snapshot = RecoverySnapshot {
        existed: previous.is_some(),
        content: previous.map(|bytes| String::from_utf8_lossy(bytes).to_string()),
        captured_at: Utc::now().to_rfc3339(),
    };
    let value = serde_json::to_value(snapshot)
        .map_err(|e| format!("Could not serialize recovery snapshot: {e}"))?;
    secure_store::put_json(
        &secure_store::recovery_key(&platform.to_string(), write.slot),
        &value,
    )
}

fn rollback_writes(writes: &[PlannedWrite], previous: &[Option<Vec<u8>>], applied: usize) -> Result<(), String> {
    let mut errors = Vec::new();
    for index in (0..applied).rev() {
        if let Err(err) = atomic_fs::restore(&writes[index].path, previous[index].as_deref()) {
            errors.push(err);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" | "))
    }
}

fn apply_transaction(platform: &Platform, writes: &[PlannedWrite]) -> Result<Vec<Option<Vec<u8>>>, String> {
    let mut previous = Vec::with_capacity(writes.len());

    for write in writes {
        let before = atomic_fs::read_optional(&write.path)?;
        save_recovery_snapshot(platform, write, before.as_deref())?;
        previous.push(before);
    }

    for (index, write) in writes.iter().enumerate() {
        if let Err(err) = atomic_fs::atomic_write(&write.path, &write.bytes) {
            let rollback = rollback_writes(writes, &previous, index);
            return match rollback {
                Ok(_) => Err(format!("Switch failed and was rolled back: {err}")),
                Err(rollback_err) => Err(format!(
                    "Switch failed: {err}. Automatic rollback also reported: {rollback_err}"
                )),
            };
        }
    }

    Ok(previous)
}

fn infer_top_level_email(credentials: &Value) -> Option<String> {
    credentials
        .get("email")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

#[tauri::command]
pub fn get_accounts() -> Result<Vec<Account>, String> {
    let store = load_store()?;
    Ok(store.accounts.iter().map(StoredAccount::view).collect())
}

#[tauri::command]
pub fn add_account(
    name: String,
    platform: String,
    credentials: Value,
    email: Option<String>,
) -> Result<Account, String> {
    let mut store = load_store()?;
    let platform = providers::parse_platform(&platform)?;
    let id = Uuid::new_v4().to_string();
    let secret_ref = secure_store::account_key(&id);

    secure_store::put_json(&secret_ref, &credentials)?;

    let account = StoredAccount {
        id,
        name,
        platform,
        email: email.or_else(|| infer_top_level_email(&credentials)),
        secret_ref: Some(secret_ref.clone()),
        credentials: None,
        is_active: false,
        created_at: Utc::now().to_rfc3339(),
        usage: None,
    };

    store.accounts.push(account.clone());
    if let Err(err) = save_store(&store) {
        secure_store::delete(&secret_ref);
        return Err(err);
    }

    Ok(account.view())
}

#[tauri::command]
pub fn import_current_account(name: String, platform: String) -> Result<Account, String> {
    let mut store = load_store()?;
    let platform = providers::parse_platform(&platform)?;
    let imported = providers::import_current(&platform)?;
    let id = Uuid::new_v4().to_string();
    let secret_ref = secure_store::account_key(&id);

    secure_store::put_json(&secret_ref, &imported.credentials)?;

    for account in &mut store.accounts {
        if account.platform == platform {
            account.is_active = false;
        }
    }

    let account = StoredAccount {
        id,
        name,
        platform,
        email: imported.email,
        secret_ref: Some(secret_ref.clone()),
        credentials: None,
        is_active: true,
        created_at: Utc::now().to_rfc3339(),
        usage: None,
    };

    store.accounts.push(account.clone());
    if let Err(err) = save_store(&store) {
        secure_store::delete(&secret_ref);
        return Err(err);
    }

    Ok(account.view())
}

#[tauri::command]
pub fn delete_account(id: String) -> Result<(), String> {
    let mut store = load_store()?;
    let index = store
        .accounts
        .iter()
        .position(|account| account.id == id)
        .ok_or_else(|| "Account not found".to_string())?;

    let removed = store.accounts.remove(index);
    save_store(&store)?;

    if let Some(secret_ref) = removed.secret_ref.as_deref() {
        secure_store::delete(secret_ref);
    }
    Ok(())
}

#[tauri::command]
pub fn switch_account(id: String) -> Result<Account, String> {
    let mut store = load_store()?;
    let target_index = store
        .accounts
        .iter()
        .position(|account| account.id == id)
        .ok_or_else(|| "Account not found".to_string())?;

    let target = store.accounts[target_index].clone();
    if target.is_active {
        return Ok(target.view());
    }

    let credentials = load_credentials(&target)?;
    let writes = providers::build_write_set(
        &target.platform,
        &credentials,
        target.email.as_deref(),
    )?;
    let previous = apply_transaction(&target.platform, &writes)?;

    for account in &mut store.accounts {
        if account.platform == target.platform {
            account.is_active = account.id == target.id;
        }
    }

    if let Err(metadata_err) = save_store(&store) {
        let rollback = rollback_writes(&writes, &previous, writes.len());
        return match rollback {
            Ok(_) => Err(format!(
                "The session files were restored because SwitchCraft could not save account state: {metadata_err}"
            )),
            Err(rollback_err) => Err(format!(
                "Could not save account state: {metadata_err}. Rollback also reported: {rollback_err}"
            )),
        };
    }

    Ok(store.accounts[target_index].view())
}

#[tauri::command]
pub fn restore_last_session(platform: String) -> Result<(), String> {
    let platform = providers::parse_platform(&platform)?;
    let mut targets = vec![("primary", providers::primary_auth_path(&platform)?)];
    if platform == Platform::Gemini {
        targets.push(("accounts", providers::gemini_accounts_path()?));
    }

    for (slot, path) in targets {
        let key = secure_store::recovery_key(&platform.to_string(), slot);
        let snapshot_value = secure_store::get_json(&key)?;
        let snapshot: RecoverySnapshot = serde_json::from_value(snapshot_value)
            .map_err(|e| format!("Recovery snapshot is invalid: {e}"))?;

        if snapshot.existed {
            let content = snapshot.content.unwrap_or_default();
            atomic_fs::atomic_write(&path, content.as_bytes())?;
        } else {
            atomic_fs::restore(&path, None)?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn update_usage(id: String, mut usage: UsageInfo) -> Result<(), String> {
    let mut store = load_store()?;
    let account = store
        .accounts
        .iter_mut()
        .find(|account| account.id == id)
        .ok_or_else(|| "Account not found".to_string())?;

    usage.five_hour_remaining_pct = usage.five_hour_remaining_pct.map(|value| value.min(100));
    usage.weekly_remaining_pct = usage.weekly_remaining_pct.map(|value| value.min(100));
    if usage.source.is_none() {
        usage.source = Some("manual".into());
    }
    if usage.observed_at.is_none() {
        usage.observed_at = Some(Utc::now().to_rfc3339());
    }

    account.usage = Some(usage);
    save_store(&store)
}

#[tauri::command]
pub fn get_active_accounts() -> Result<Vec<Account>, String> {
    let store = load_store()?;
    Ok(store
        .accounts
        .iter()
        .filter(|account| account.is_active)
        .map(StoredAccount::view)
        .collect())
}

#[tauri::command]
pub fn rename_account(id: String, new_name: String) -> Result<(), String> {
    let mut store = load_store()?;
    let account = store
        .accounts
        .iter_mut()
        .find(|account| account.id == id)
        .ok_or_else(|| "Account not found".to_string())?;
    account.name = new_name;
    save_store(&store)
}

#[tauri::command]
pub fn get_security_summary() -> Result<Value, String> {
    let store = load_store()?;
    let secure = store.accounts.iter().filter(|account| account.secret_ref.is_some()).count();
    let legacy = store.accounts.iter().filter(|account| account.credentials.is_some()).count();
    Ok(json!({
        "secure_accounts": secure,
        "legacy_accounts": legacy,
        "total_accounts": store.accounts.len()
    }))
}
