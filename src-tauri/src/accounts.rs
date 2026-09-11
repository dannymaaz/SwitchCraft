use crate::atomic_fs;
use crate::providers::{self, PlannedMutation, Platform};
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

// v1.1 compatibility format. New recovery data is stored as one secure manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyRecoverySnapshot {
    existed: bool,
    content: Option<String>,
    captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecoveryManifest {
    captured_at: String,
    targets: Vec<RecoveryTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RecoveryTarget {
    File {
        slot: String,
        path: String,
        existed: bool,
        content: Option<String>,
    },
    Keyring {
        slot: String,
        service: String,
        account: String,
        existed: bool,
        content: Option<String>,
    },
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

fn utf8_file_content(path: &PathBuf) -> Result<Option<String>, String> {
    let Some(bytes) = atomic_fs::read_optional(path)? else {
        return Ok(None);
    };
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| format!("Credential file {} is not valid UTF-8", path.display()))
}

fn snapshot_mutation(mutation: &PlannedMutation) -> Result<RecoveryTarget, String> {
    match mutation {
        PlannedMutation::File { slot, path, .. } => {
            let content = utf8_file_content(path)?;
            Ok(RecoveryTarget::File {
                slot: (*slot).to_string(),
                path: path.to_string_lossy().into_owned(),
                existed: content.is_some(),
                content,
            })
        }
        PlannedMutation::Keyring {
            slot,
            service,
            account,
            ..
        } => {
            let content = secure_store::try_get_external_text(service, account)?;
            Ok(RecoveryTarget::Keyring {
                slot: (*slot).to_string(),
                service: service.clone(),
                account: account.clone(),
                existed: content.is_some(),
                content,
            })
        }
    }
}

fn snapshot_recovery_target(target: &RecoveryTarget) -> Result<RecoveryTarget, String> {
    match target {
        RecoveryTarget::File { slot, path, .. } => {
            let path_buf = PathBuf::from(path);
            let content = utf8_file_content(&path_buf)?;
            Ok(RecoveryTarget::File {
                slot: slot.clone(),
                path: path.clone(),
                existed: content.is_some(),
                content,
            })
        }
        RecoveryTarget::Keyring {
            slot,
            service,
            account,
            ..
        } => {
            let content = secure_store::try_get_external_text(service, account)?;
            Ok(RecoveryTarget::Keyring {
                slot: slot.clone(),
                service: service.clone(),
                account: account.clone(),
                existed: content.is_some(),
                content,
            })
        }
    }
}

fn apply_mutation(mutation: &PlannedMutation) -> Result<(), String> {
    match mutation {
        PlannedMutation::File { path, value, .. } => atomic_fs::restore(path, value.as_deref()),
        PlannedMutation::Keyring {
            service,
            account,
            value,
            ..
        } => match value {
            Some(value) => secure_store::put_external_text(service, account, value),
            None => secure_store::delete_external(service, account),
        },
    }
}

fn restore_target(target: &RecoveryTarget) -> Result<(), String> {
    match target {
        RecoveryTarget::File {
            path,
            existed,
            content,
            ..
        } => {
            let path = PathBuf::from(path);
            if *existed {
                atomic_fs::atomic_write(&path, content.as_deref().unwrap_or_default().as_bytes())
            } else {
                atomic_fs::restore(&path, None)
            }
        }
        RecoveryTarget::Keyring {
            service,
            account,
            existed,
            content,
            ..
        } => {
            if *existed {
                secure_store::put_external_text(
                    service,
                    account,
                    content.as_deref().unwrap_or_default(),
                )
            } else {
                secure_store::delete_external(service, account)
            }
        }
    }
}

fn rollback_targets(targets: &[RecoveryTarget], applied: usize) -> Result<(), String> {
    let mut errors = Vec::new();
    for index in (0..applied.min(targets.len())).rev() {
        if let Err(err) = restore_target(&targets[index]) {
            errors.push(err);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" | "))
    }
}

fn apply_transaction(mutations: &[PlannedMutation]) -> Result<RecoveryManifest, String> {
    let mut previous = Vec::with_capacity(mutations.len());
    for mutation in mutations {
        previous.push(snapshot_mutation(mutation)?);
    }

    for (index, mutation) in mutations.iter().enumerate() {
        if let Err(err) = apply_mutation(mutation) {
            // Restoring the failing target as well is safe and covers credential-store
            // implementations that may report an error after a partial side effect.
            let rollback = rollback_targets(&previous, index + 1);
            return match rollback {
                Ok(_) => Err(format!("Switch failed and was rolled back: {err}")),
                Err(rollback_err) => Err(format!(
                    "Switch failed: {err}. Automatic rollback also reported: {rollback_err}"
                )),
            };
        }
    }

    Ok(RecoveryManifest {
        captured_at: Utc::now().to_rfc3339(),
        targets: previous,
    })
}

fn persist_recovery_manifest(platform: &Platform, manifest: &RecoveryManifest) -> Result<(), String> {
    let value = serde_json::to_value(manifest)
        .map_err(|e| format!("Could not serialize recovery manifest: {e}"))?;
    secure_store::put_json(
        &secure_store::recovery_manifest_key(&platform.to_string()),
        &value,
    )
}

fn restore_previous_manifest_value(platform: &Platform, previous: Option<Value>) -> Result<(), String> {
    let key = secure_store::recovery_manifest_key(&platform.to_string());
    match previous {
        Some(value) => secure_store::put_json(&key, &value),
        None => {
            secure_store::delete(&key);
            Ok(())
        }
    }
}

fn load_legacy_recovery_manifest(platform: &Platform) -> Result<RecoveryManifest, String> {
    let mut slots = vec![("primary", providers::primary_auth_path(platform)?)];
    if *platform == Platform::Gemini {
        slots.push(("accounts", providers::gemini_accounts_path()?));
    }

    let mut targets = Vec::with_capacity(slots.len());
    for (slot, path) in slots {
        let value = secure_store::get_json(&secure_store::recovery_key(&platform.to_string(), slot))?;
        let snapshot: LegacyRecoverySnapshot = serde_json::from_value(value)
            .map_err(|e| format!("Legacy recovery snapshot is invalid: {e}"))?;
        targets.push(RecoveryTarget::File {
            slot: slot.to_string(),
            path: path.to_string_lossy().into_owned(),
            existed: snapshot.existed,
            content: snapshot.content,
        });
    }

    Ok(RecoveryManifest {
        captured_at: Utc::now().to_rfc3339(),
        targets,
    })
}

fn load_recovery_manifest(platform: &Platform) -> Result<RecoveryManifest, String> {
    let key = secure_store::recovery_manifest_key(&platform.to_string());
    if let Some(value) = secure_store::try_get_json(&key)? {
        return serde_json::from_value(value)
            .map_err(|e| format!("Recovery manifest is invalid: {e}"));
    }
    load_legacy_recovery_manifest(platform)
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

    // Recovery metadata is read before touching provider state. If the local vault is
    // unavailable, the switch aborts while the currently active session is still intact.
    let recovery_key = secure_store::recovery_manifest_key(&target.platform.to_string());
    let previous_manifest = secure_store::try_get_json(&recovery_key)?;

    let credentials = load_credentials(&target)?;
    let mutations = providers::build_mutations(
        &target.platform,
        &credentials,
        target.email.as_deref(),
    )?;
    let recovery = apply_transaction(&mutations)?;

    if let Err(err) = persist_recovery_manifest(&target.platform, &recovery) {
        let rollback = rollback_targets(&recovery.targets, recovery.targets.len());
        return match rollback {
            Ok(_) => Err(format!(
                "The switch was reverted because its recovery state could not be secured: {err}"
            )),
            Err(rollback_err) => Err(format!(
                "Could not secure recovery state: {err}. Rollback also reported: {rollback_err}"
            )),
        };
    }

    for account in &mut store.accounts {
        if account.platform == target.platform {
            account.is_active = account.id == target.id;
        }
    }

    if let Err(metadata_err) = save_store(&store) {
        let rollback = rollback_targets(&recovery.targets, recovery.targets.len());
        let manifest_restore = restore_previous_manifest_value(&target.platform, previous_manifest);
        let mut details = Vec::new();
        if let Err(err) = rollback {
            details.push(format!("session rollback: {err}"));
        }
        if let Err(err) = manifest_restore {
            details.push(format!("recovery-manifest rollback: {err}"));
        }
        return if details.is_empty() {
            Err(format!(
                "The session was restored because SwitchCraft could not save account state: {metadata_err}"
            ))
        } else {
            Err(format!(
                "Could not save account state: {metadata_err}. Rollback also reported: {}",
                details.join(" | ")
            ))
        };
    }

    Ok(store.accounts[target_index].view())
}

#[tauri::command]
pub fn restore_last_session(platform: String) -> Result<(), String> {
    let platform = providers::parse_platform(&platform)?;
    let mut store = load_store()?;

    // Snapshot the current recovery pointer before provider state is changed.
    let recovery_key = secure_store::recovery_manifest_key(&platform.to_string());
    let previous_manifest_value = secure_store::try_get_json(&recovery_key)?;
    let manifest = load_recovery_manifest(&platform)?;

    let mut current = Vec::with_capacity(manifest.targets.len());
    for target in &manifest.targets {
        current.push(snapshot_recovery_target(target)?);
    }

    for (index, target) in manifest.targets.iter().enumerate() {
        if let Err(err) = restore_target(target) {
            let rollback = rollback_targets(&current, index + 1);
            return match rollback {
                Ok(_) => Err(format!("Restore failed and was rolled back: {err}")),
                Err(rollback_err) => Err(format!(
                    "Restore failed: {err}. Automatic rollback also reported: {rollback_err}"
                )),
            };
        }
    }

    // Save a reverse recovery point so Restore remains reversible.
    let reverse_manifest = RecoveryManifest {
        captured_at: Utc::now().to_rfc3339(),
        targets: current.clone(),
    };
    if let Err(err) = persist_recovery_manifest(&platform, &reverse_manifest) {
        let rollback = rollback_targets(&current, current.len());
        return match rollback {
            Ok(_) => Err(format!(
                "Restore was reverted because the reverse recovery point could not be secured: {err}"
            )),
            Err(rollback_err) => Err(format!(
                "Could not secure reverse recovery point: {err}. Rollback also reported: {rollback_err}"
            )),
        };
    }

    // The restored session may not correspond to a profile registered in SwitchCraft.
    for account in &mut store.accounts {
        if account.platform == platform {
            account.is_active = false;
        }
    }

    if let Err(metadata_err) = save_store(&store) {
        let rollback = rollback_targets(&current, current.len());
        let manifest_restore = restore_previous_manifest_value(&platform, previous_manifest_value);
        let mut details = Vec::new();
        if let Err(err) = rollback {
            details.push(format!("session rollback: {err}"));
        }
        if let Err(err) = manifest_restore {
            details.push(format!("recovery-manifest rollback: {err}"));
        }
        return if details.is_empty() {
            Err(format!(
                "The restored session was reverted because SwitchCraft could not save account state: {metadata_err}"
            ))
        } else {
            Err(format!(
                "Could not save restored account state: {metadata_err}. Rollback also reported: {}",
                details.join(" | ")
            ))
        };
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
    let secure = store
        .accounts
        .iter()
        .filter(|account| account.secret_ref.is_some())
        .count();
    let legacy = store
        .accounts
        .iter()
        .filter(|account| account.credentials.is_some())
        .count();
    Ok(json!({
        "secure_accounts": secure,
        "legacy_accounts": legacy,
        "total_accounts": store.accounts.len()
    }))
}
