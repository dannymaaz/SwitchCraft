use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Codex,
    Gemini,
    Claude,
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

pub struct ImportedSession {
    pub credentials: Value,
    pub email: Option<String>,
}

pub struct PlannedWrite {
    pub slot: &'static str,
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

pub fn parse_platform(value: &str) -> Result<Platform, String> {
    match value.to_lowercase().as_str() {
        "codex" => Ok(Platform::Codex),
        "gemini" => Ok(Platform::Gemini),
        "claude" => Ok(Platform::Claude),
        _ => Err("Invalid provider. Use codex, gemini, or claude".into()),
    }
}

fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Could not determine the current home directory".to_string())
}

pub fn codex_auth_path() -> Result<PathBuf, String> {
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        Ok(PathBuf::from(codex_home).join("auth.json"))
    } else {
        Ok(home_dir()?.join(".codex").join("auth.json"))
    }
}

pub fn gemini_oauth_path() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".gemini").join("oauth_creds.json"))
}

pub fn gemini_accounts_path() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".gemini").join("google_accounts.json"))
}

pub fn claude_credentials_path() -> Result<PathBuf, String> {
    if let Ok(config_dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(config_dir).join(".credentials.json"));
    }
    Ok(home_dir()?.join(".claude").join(".credentials.json"))
}

pub fn primary_auth_path(platform: &Platform) -> Result<PathBuf, String> {
    match platform {
        Platform::Codex => codex_auth_path(),
        Platform::Gemini => gemini_oauth_path(),
        Platform::Claude => claude_credentials_path(),
    }
}

pub fn supported_targets(platform: &Platform) -> Vec<String> {
    match platform {
        Platform::Codex => vec!["Codex file-backed session".into()],
        Platform::Gemini => vec!["Gemini CLI OAuth".into()],
        Platform::Claude => vec!["Claude Code file-backed session".into()],
    }
}

fn read_json(path: &PathBuf) -> Result<Value, String> {
    if !path.exists() {
        return Err(format!("No active session file was found at {}", path.display()));
    }

    let raw = fs::read_to_string(path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("{} does not contain valid JSON: {e}", path.display()))
}

fn find_email(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in ["email", "account_email", "user_email"] {
                if let Some(email) = map.get(key).and_then(Value::as_str) {
                    return Some(email.to_string());
                }
            }
            map.values().find_map(find_email)
        }
        Value::Array(values) => values.iter().find_map(find_email),
        _ => None,
    }
}

fn gemini_active_email() -> Option<String> {
    let path = gemini_accounts_path().ok()?;
    let value = read_json(&path).ok()?;
    value
        .get("active")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

pub fn import_current(platform: &Platform) -> Result<ImportedSession, String> {
    let path = primary_auth_path(platform)?;
    let credentials = read_json(&path).map_err(|err| {
        if *platform == Platform::Claude && cfg!(target_os = "macos") {
            format!(
                "{err}. Claude Code normally uses macOS Keychain; SwitchCraft v1.1 only imports the file-backed fallback or an isolated CLAUDE_CONFIG_DIR profile."
            )
        } else if *platform == Platform::Codex {
            format!(
                "{err}. If Codex is configured with cli_auth_credentials_store=\"keyring\", direct keyring switching is not enabled in v1.1."
            )
        } else {
            err
        }
    })?;

    let email = match platform {
        Platform::Gemini => gemini_active_email().or_else(|| find_email(&credentials)),
        _ => find_email(&credentials),
    };

    Ok(ImportedSession { credentials, email })
}

fn merge_gemini_accounts(target_email: &str) -> Result<Value, String> {
    let path = gemini_accounts_path()?;
    let mut current = if path.exists() {
        read_json(&path).unwrap_or_else(|_| json!({ "active": null, "old": [] }))
    } else {
        json!({ "active": null, "old": [] })
    };

    if !current.is_object() {
        current = json!({ "active": null, "old": [] });
    }

    let previous_active = current
        .get("active")
        .and_then(Value::as_str)
        .map(str::to_string);

    let mut old: Vec<String> = current
        .get("old")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    if let Some(previous) = previous_active {
        if previous != target_email && !old.iter().any(|value| value == &previous) {
            old.push(previous);
        }
    }
    old.retain(|value| value != target_email);

    Ok(json!({
        "active": target_email,
        "old": old
    }))
}

pub fn build_write_set(
    platform: &Platform,
    credentials: &Value,
    email: Option<&str>,
) -> Result<Vec<PlannedWrite>, String> {
    let primary_path = primary_auth_path(platform)?;
    let primary_bytes = serde_json::to_vec_pretty(credentials)
        .map_err(|e| format!("Could not serialize credentials: {e}"))?;

    let mut writes = vec![PlannedWrite {
        slot: "primary",
        path: primary_path,
        bytes: primary_bytes,
    }];

    if *platform == Platform::Gemini {
        let target_email = email.ok_or_else(|| {
            "This Gemini profile has no Google account email. Import it from an active Gemini CLI session so SwitchCraft can update google_accounts.json safely.".to_string()
        })?;
        let accounts = merge_gemini_accounts(target_email)?;
        writes.push(PlannedWrite {
            slot: "accounts",
            path: gemini_accounts_path()?,
            bytes: serde_json::to_vec_pretty(&accounts)
                .map_err(|e| format!("Could not serialize Gemini account registry: {e}"))?,
        });
    }

    Ok(writes)
}
