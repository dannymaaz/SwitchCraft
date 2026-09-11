use crate::secure_store;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const CODEX_DIRECT_KEYRING_SERVICE: &str = "Codex Auth";

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

#[derive(Debug, Clone)]
pub enum PlannedMutation {
    File {
        slot: &'static str,
        path: PathBuf,
        value: Option<Vec<u8>>,
    },
    Keyring {
        slot: &'static str,
        service: String,
        account: String,
        value: Option<String>,
    },
}

impl PlannedMutation {
    pub fn slot(&self) -> &'static str {
        match self {
            Self::File { slot, .. } | Self::Keyring { slot, .. } => slot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexCredentialStoreMode {
    File,
    Keyring,
    Auto,
    Ephemeral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodexAuthConfig {
    mode: CodexCredentialStoreMode,
    secret_auth_storage: bool,
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

pub fn codex_home_dir() -> Result<PathBuf, String> {
    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        Ok(PathBuf::from(codex_home))
    } else {
        Ok(home_dir()?.join(".codex"))
    }
}

pub fn codex_auth_path() -> Result<PathBuf, String> {
    Ok(codex_home_dir()?.join("auth.json"))
}

fn codex_config_path() -> Result<PathBuf, String> {
    Ok(codex_home_dir()?.join("config.toml"))
}

fn codex_secrets_auth_path() -> Result<PathBuf, String> {
    Ok(codex_home_dir()?.join("secrets").join("codex_auth.age"))
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
        Platform::Codex => vec!["Codex file / direct keyring (config-aware)".into()],
        Platform::Gemini => vec!["Gemini CLI OAuth".into()],
        Platform::Claude => vec!["Claude Code file-backed session".into()],
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
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
                    if !email.trim().is_empty() {
                        return Some(email.to_string());
                    }
                }
            }
            map.values().find_map(find_email)
        }
        Value::Array(values) => values.iter().find_map(find_email),
        _ => None,
    }
}

fn parse_codex_auth_config(raw: Option<&str>, windows_default: bool) -> Result<CodexAuthConfig, String> {
    let Some(raw) = raw else {
        return Ok(CodexAuthConfig {
            mode: CodexCredentialStoreMode::File,
            secret_auth_storage: windows_default,
        });
    };

    let config: toml::Value = toml::from_str(raw)
        .map_err(|e| format!("Could not parse Codex config.toml: {e}"))?;

    let mode = match config
        .get("cli_auth_credentials_store")
        .and_then(toml::Value::as_str)
        .unwrap_or("file")
        .to_ascii_lowercase()
        .as_str()
    {
        "file" => CodexCredentialStoreMode::File,
        "keyring" => CodexCredentialStoreMode::Keyring,
        "auto" => CodexCredentialStoreMode::Auto,
        "ephemeral" => CodexCredentialStoreMode::Ephemeral,
        value => {
            return Err(format!(
                "Unsupported Codex cli_auth_credentials_store value '{value}'."
            ))
        }
    };

    let secret_auth_storage = config
        .get("features")
        .and_then(|features| features.get("secret_auth_storage"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(windows_default);

    Ok(CodexAuthConfig {
        mode,
        secret_auth_storage,
    })
}

fn load_codex_auth_config() -> Result<CodexAuthConfig, String> {
    let path = codex_config_path()?;
    if !path.exists() {
        return parse_codex_auth_config(None, cfg!(target_os = "windows"));
    }
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("Could not read Codex config at {}: {e}", path.display()))?;
    parse_codex_auth_config(Some(&raw), cfg!(target_os = "windows"))
}

fn codex_direct_keyring_account_for_home(codex_home: &Path) -> String {
    let canonical = codex_home
        .canonicalize()
        .unwrap_or_else(|_| codex_home.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    let hex = format!("{digest:x}");
    let short = hex.get(..16).unwrap_or(hex.as_str());
    format!("cli|{short}")
}

pub fn codex_direct_keyring_account() -> Result<String, String> {
    Ok(codex_direct_keyring_account_for_home(&codex_home_dir()?))
}

fn read_codex_direct_keyring() -> Result<Option<Value>, String> {
    let account = codex_direct_keyring_account()?;
    let Some(raw) = secure_store::try_get_external_text(CODEX_DIRECT_KEYRING_SERVICE, &account)? else {
        return Ok(None);
    };
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|e| format!("Codex keyring entry contains invalid auth JSON: {e}"))
}

fn unsupported_codex_secrets_message() -> String {
    "Codex is using SecretAuthStorage (encrypted codex_auth.age). SwitchCraft v1.2 does not modify that backend yet; disable secret_auth_storage or use file/direct-keyring mode until the encrypted-store adapter is available.".into()
}

fn import_codex() -> Result<ImportedSession, String> {
    let config = load_codex_auth_config()?;
    let file_path = codex_auth_path()?;

    let credentials = match config.mode {
        CodexCredentialStoreMode::File => read_json(&file_path)?,
        CodexCredentialStoreMode::Keyring => {
            if config.secret_auth_storage {
                return Err(unsupported_codex_secrets_message());
            }
            read_codex_direct_keyring()?.ok_or_else(|| {
                "Codex is configured for direct keyring storage, but no 'Codex Auth' credential was found for the current CODEX_HOME.".to_string()
            })?
        }
        CodexCredentialStoreMode::Auto => {
            if config.secret_auth_storage {
                if codex_secrets_auth_path()?.exists() {
                    return Err(unsupported_codex_secrets_message());
                }
                read_json(&file_path)?
            } else {
                match read_codex_direct_keyring() {
                    Ok(Some(value)) => value,
                    Ok(None) => read_json(&file_path)?,
                    Err(keyring_error) => read_json(&file_path).map_err(|file_error| {
                        format!(
                            "Codex auto storage could not be read from keyring ({keyring_error}) or file ({file_error})."
                        )
                    })?,
                }
            }
        }
        CodexCredentialStoreMode::Ephemeral => {
            return Err(
                "Codex is using ephemeral authentication. Process-memory credentials cannot be imported safely by SwitchCraft."
                    .into(),
            )
        }
    };

    Ok(ImportedSession {
        email: find_email(&credentials),
        credentials,
    })
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
    if *platform == Platform::Codex {
        return import_codex();
    }

    let path = primary_auth_path(platform)?;
    let credentials = read_json(&path).map_err(|err| {
        if *platform == Platform::Claude && cfg!(target_os = "macos") {
            format!(
                "{err}. Claude Code normally uses macOS Keychain; SwitchCraft currently imports the file-backed fallback or an isolated CLAUDE_CONFIG_DIR profile."
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

fn merge_gemini_accounts_value(mut current: Value, target_email: &str) -> Value {
    if !current.is_object() {
        current = json!({});
    }

    let previous_active = current
        .get("active")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);

    let mut old: Vec<String> = current
        .get("old")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|value| !value.trim().is_empty())
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
    old.dedup();

    if let Some(object) = current.as_object_mut() {
        object.insert("active".into(), Value::String(target_email.to_string()));
        object.insert(
            "old".into(),
            Value::Array(old.into_iter().map(Value::String).collect()),
        );
    }

    current
}

fn merge_gemini_accounts(target_email: &str) -> Result<Value, String> {
    let path = gemini_accounts_path()?;
    let current = if path.exists() {
        read_json(&path).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    Ok(merge_gemini_accounts_value(current, target_email))
}

fn file_mutation(slot: &'static str, path: PathBuf, value: Option<Vec<u8>>) -> PlannedMutation {
    PlannedMutation::File { slot, path, value }
}

fn codex_file_mutations(credentials: &Value) -> Result<Vec<PlannedMutation>, String> {
    Ok(vec![file_mutation(
        "codex_file",
        codex_auth_path()?,
        Some(
            serde_json::to_vec_pretty(credentials)
                .map_err(|e| format!("Could not serialize Codex credentials: {e}"))?,
        ),
    )])
}

fn codex_direct_keyring_mutations(credentials: &Value) -> Result<Vec<PlannedMutation>, String> {
    let serialized = serde_json::to_string(credentials)
        .map_err(|e| format!("Could not serialize Codex credentials: {e}"))?;
    Ok(vec![
        PlannedMutation::Keyring {
            slot: "codex_keyring",
            service: CODEX_DIRECT_KEYRING_SERVICE.into(),
            account: codex_direct_keyring_account()?,
            value: Some(serialized),
        },
        file_mutation("codex_file", codex_auth_path()?, None),
    ])
}

fn build_codex_mutations(credentials: &Value) -> Result<Vec<PlannedMutation>, String> {
    let config = load_codex_auth_config()?;
    match config.mode {
        CodexCredentialStoreMode::File => codex_file_mutations(credentials),
        CodexCredentialStoreMode::Keyring => {
            if config.secret_auth_storage {
                Err(unsupported_codex_secrets_message())
            } else {
                codex_direct_keyring_mutations(credentials)
            }
        }
        CodexCredentialStoreMode::Auto => {
            if config.secret_auth_storage {
                if codex_secrets_auth_path()?.exists() {
                    return Err(unsupported_codex_secrets_message());
                }
                return codex_file_mutations(credentials);
            }

            // In auto mode, preserve the currently resolved backend: an existing direct
            // keyring entry wins; otherwise use the file fallback. If keyring access itself
            // fails, Codex also falls back to file storage.
            match read_codex_direct_keyring() {
                Ok(Some(_)) => codex_direct_keyring_mutations(credentials),
                Ok(None) | Err(_) => codex_file_mutations(credentials),
            }
        }
        CodexCredentialStoreMode::Ephemeral => Err(
            "Codex is using ephemeral authentication. SwitchCraft cannot switch process-memory credentials."
                .into(),
        ),
    }
}

pub fn build_mutations(
    platform: &Platform,
    credentials: &Value,
    email: Option<&str>,
) -> Result<Vec<PlannedMutation>, String> {
    if *platform == Platform::Codex {
        return build_codex_mutations(credentials);
    }

    let primary_path = primary_auth_path(platform)?;
    let primary_bytes = serde_json::to_vec_pretty(credentials)
        .map_err(|e| format!("Could not serialize credentials: {e}"))?;

    let mut mutations = vec![file_mutation("primary", primary_path, Some(primary_bytes))];

    if *platform == Platform::Gemini {
        let target_email = email.ok_or_else(|| {
            "This Gemini profile has no Google account email. Import it from an active Gemini CLI session so SwitchCraft can update google_accounts.json safely.".to_string()
        })?;
        let accounts = merge_gemini_accounts(target_email)?;
        mutations.push(file_mutation(
            "accounts",
            gemini_accounts_path()?,
            Some(
                serde_json::to_vec_pretty(&accounts)
                    .map_err(|e| format!("Could not serialize Gemini account registry: {e}"))?,
            ),
        ));
    }

    Ok(mutations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_registry_preserves_unknown_fields() {
        let current = json!({
            "active": "old@example.com",
            "old": ["older@example.com"],
            "future_field": { "keep": true }
        });

        let merged = merge_gemini_accounts_value(current, "new@example.com");

        assert_eq!(merged["active"], "new@example.com");
        assert_eq!(merged["future_field"]["keep"], true);
        assert_eq!(
            merged["old"],
            json!(["older@example.com", "old@example.com"])
        );
    }

    #[test]
    fn gemini_registry_removes_target_from_old_history() {
        let current = json!({
            "active": "old@example.com",
            "old": ["new@example.com", "older@example.com"]
        });

        let merged = merge_gemini_accounts_value(current, "new@example.com");

        assert_eq!(merged["active"], "new@example.com");
        assert_eq!(
            merged["old"],
            json!(["older@example.com", "old@example.com"])
        );
    }

    #[test]
    fn find_email_ignores_empty_values_and_searches_nested_objects() {
        let value = json!({
            "email": "",
            "tokens": {
                "user_email": "person@example.com"
            }
        });
        assert_eq!(find_email(&value).as_deref(), Some("person@example.com"));
    }

    #[test]
    fn codex_config_defaults_to_file_and_platform_secret_default() {
        let parsed = parse_codex_auth_config(None, true).unwrap();
        assert_eq!(parsed.mode, CodexCredentialStoreMode::File);
        assert!(parsed.secret_auth_storage);
    }

    #[test]
    fn codex_config_parses_direct_keyring_and_feature_override() {
        let raw = r#"
cli_auth_credentials_store = "keyring"

[features]
secret_auth_storage = false
"#;
        let parsed = parse_codex_auth_config(Some(raw), true).unwrap();
        assert_eq!(parsed.mode, CodexCredentialStoreMode::Keyring);
        assert!(!parsed.secret_auth_storage);
    }

    #[test]
    fn codex_direct_keyring_key_uses_cli_prefix_and_short_hash() {
        let path = PathBuf::from("/switchcraft/nonexistent-codex-home");
        let key = codex_direct_keyring_account_for_home(&path);
        assert!(key.starts_with("cli|"));
        assert_eq!(key.len(), 20);
    }
}
