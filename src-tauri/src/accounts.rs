use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub credentials: serde_json::Value,
    pub is_active: bool,
    pub created_at: String,
    pub usage: Option<UsageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Codex,
    Gemini,
    Claude,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub plan_name: String,
    pub weekly_limit: Option<u32>,
    pub weekly_used: Option<u32>,
    pub hourly_limit: Option<u32>,
    pub hourly_used: Option<u32>,
    pub reset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
}

impl Default for AccountStore {
    fn default() -> Self {
        Self {
            accounts: Vec::new(),
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Codex => write!(f, "codex"),
            Platform::Gemini => write!(f, "gemini"),
            Platform::Claude => write!(f, "claude"),
        }
    }
}

fn get_home_dir() -> PathBuf {
    dirs::home_dir().expect("Could not determine home directory")
}

fn get_codex_auth_path() -> PathBuf {
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        PathBuf::from(codex_home).join("auth.json")
    } else {
        get_home_dir().join(".codex").join("auth.json")
    }
}

fn get_gemini_tokens_path() -> PathBuf {
    let oauth_path = get_home_dir().join(".gemini").join("oauth_creds.json");
    if oauth_path.exists() {
        oauth_path
    } else {
        get_home_dir().join(".gemini").join("tokens.json")
    }
}

fn get_gemini_accounts_path() -> PathBuf {
    get_home_dir().join(".gemini").join("google_accounts.json")
}

fn get_claude_session_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| {
            get_home_dir()
                .join("AppData")
                .join("Roaming")
                .to_string_lossy()
                .to_string()
        });
        PathBuf::from(appdata).join("Claude").join("session.json")
    }
    #[cfg(target_os = "macos")]
    {
        get_home_dir()
            .join("Library")
            .join("Application Support")
            .join("Claude")
            .join("session.json")
    }
    #[cfg(target_os = "linux")]
    {
        get_home_dir()
            .join(".config")
            .join("Claude")
            .join("session.json")
    }
}

fn get_auth_path(platform: &Platform) -> PathBuf {
    match platform {
        Platform::Codex => get_codex_auth_path(),
        Platform::Gemini => get_gemini_tokens_path(),
        Platform::Claude => get_claude_session_path(),
    }
}

fn get_backup_dir() -> PathBuf {
    let dir = get_home_dir().join(".switchcraft").join("backups");
    fs::create_dir_all(&dir).ok();
    dir
}

fn get_store_path() -> PathBuf {
    let dir = get_home_dir().join(".switchcraft");
    fs::create_dir_all(&dir).ok();
    dir.join("accounts.json")
}

pub fn load_store() -> AccountStore {
    let path = get_store_path();
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        AccountStore::default()
    }
}

pub fn save_store(store: &AccountStore) {
    let path = get_store_path();
    let data = serde_json::to_string_pretty(store).unwrap_or_default();
    fs::write(&path, data).ok();
}

#[tauri::command]
pub fn get_accounts() -> Result<Vec<Account>, String> {
    let store = load_store();
    Ok(store.accounts)
}

#[tauri::command]
pub fn add_account(name: String, platform: String, credentials: serde_json::Value) -> Result<Account, String> {
    let mut store = load_store();

    let plat = match platform.to_lowercase().as_str() {
        "codex" => Platform::Codex,
        "gemini" => Platform::Gemini,
        "claude" => Platform::Claude,
        _ => return Err("Invalid platform. Use: codex, gemini, or claude".into()),
    };

    let account = Account {
        id: Uuid::new_v4().to_string(),
        name,
        platform: plat,
        credentials,
        is_active: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        usage: None,
    };

    store.accounts.push(account.clone());
    save_store(&store);
    Ok(account)
}

#[tauri::command]
pub fn delete_account(id: String) -> Result<(), String> {
    let mut store = load_store();
    let initial_len = store.accounts.len();
    store.accounts.retain(|a| a.id != id);

    if store.accounts.len() == initial_len {
        return Err("Account not found".into());
    }

    save_store(&store);
    Ok(())
}

#[tauri::command]
pub fn switch_account(id: String) -> Result<Account, String> {
    let mut store = load_store();

    let target_idx = store.accounts.iter().position(|a| a.id == id)
        .ok_or("Account not found")?;

    let target_platform = store.accounts[target_idx].platform.clone();
    let auth_path = get_auth_path(&target_platform);

    // Backup current credentials if they exist
    if auth_path.exists() {
        let backup_name = format!(
            "{}_{}_backup.json",
            target_platform,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let backup_path = get_backup_dir().join(backup_name);
        fs::copy(&auth_path, &backup_path)
            .map_err(|e| format!("Failed to backup current credentials: {}", e))?;
    }

    // Ensure parent directory exists
    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }

    // Write new credentials
    let creds = serde_json::to_string_pretty(&store.accounts[target_idx].credentials)
        .map_err(|e| format!("Failed to serialize credentials: {}", e))?;
    fs::write(&auth_path, &creds)
        .map_err(|e| format!("Failed to write credentials: {}", e))?;

    // If Gemini/Antigravity, also sync google_accounts.json
    if target_platform == Platform::Gemini {
        let email = store.accounts[target_idx].credentials.get("email")
            .and_then(|v| v.as_str())
            .unwrap_or(&store.accounts[target_idx].name);
        let accounts_json = serde_json::json!({
            "active": email,
            "old": []
        });
        let acc_path = get_gemini_accounts_path();
        let _ = fs::write(&acc_path, serde_json::to_string_pretty(&accounts_json).unwrap_or_default());
    }

    // Update active status: deactivate others of same platform, activate target
    for account in &mut store.accounts {
        if account.platform == target_platform {
            account.is_active = account.id == id;
        }
    }

    save_store(&store);
    Ok(store.accounts[target_idx].clone())
}

#[tauri::command]
pub fn import_current_account(name: String, platform: String) -> Result<Account, String> {
    let plat = match platform.to_lowercase().as_str() {
        "codex" => Platform::Codex,
        "gemini" => Platform::Gemini,
        "claude" => Platform::Claude,
        _ => return Err("Invalid platform".into()),
    };

    let auth_path = get_auth_path(&plat);
    if !auth_path.exists() {
        return Err(format!(
            "No credentials found at {}. Make sure you're logged in to {}.",
            auth_path.display(),
            platform
        ));
    }

    let data = fs::read_to_string(&auth_path)
        .map_err(|e| format!("Failed to read credentials: {}", e))?;
    let credentials: serde_json::Value = serde_json::from_str(&data)
        .map_err(|e| format!("Failed to parse credentials: {}", e))?;

    let mut store = load_store();

    let account = Account {
        id: Uuid::new_v4().to_string(),
        name,
        platform: plat,
        credentials,
        is_active: true,
        created_at: chrono::Utc::now().to_rfc3339(),
        usage: None,
    };

    store.accounts.push(account.clone());
    save_store(&store);
    Ok(account)
}

#[tauri::command]
pub fn update_usage(id: String, usage: UsageInfo) -> Result<(), String> {
    let mut store = load_store();
    let account = store.accounts.iter_mut().find(|a| a.id == id)
        .ok_or("Account not found")?;
    account.usage = Some(usage);
    save_store(&store);
    Ok(())
}

#[tauri::command]
pub fn get_active_accounts() -> Result<Vec<Account>, String> {
    let store = load_store();
    let active: Vec<Account> = store.accounts.iter()
        .filter(|a| a.is_active)
        .cloned()
        .collect();
    Ok(active)
}

#[tauri::command]
pub fn rename_account(id: String, new_name: String) -> Result<(), String> {
    let mut store = load_store();
    let account = store.accounts.iter_mut().find(|a| a.id == id)
        .ok_or("Account not found")?;
    account.name = new_name;
    save_store(&store);
    Ok(())
}
