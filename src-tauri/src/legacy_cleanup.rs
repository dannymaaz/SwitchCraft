use std::fs;
use std::io;
use std::path::Path;

const LEGACY_BACKUP_PROVIDERS: [&str; 3] = ["codex", "gemini", "claude"];

fn looks_like_v1_timestamp(value: &str) -> bool {
    if value.len() != 15 {
        return false;
    }

    value.bytes().enumerate().all(|(index, byte)| {
        if index == 8 {
            byte == b'_'
        } else {
            byte.is_ascii_digit()
        }
    })
}

fn is_v1_plaintext_backup_name(name: &str) -> bool {
    LEGACY_BACKUP_PROVIDERS.iter().any(|provider| {
        let prefix = format!("{provider}_");
        name.strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix("_backup.json"))
            .is_some_and(looks_like_v1_timestamp)
    })
}

fn cleanup_v1_plaintext_backups_in(dir: &Path) -> io::Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };

        if !is_v1_plaintext_backup_name(file_name) {
            continue;
        }

        fs::remove_file(entry.path())?;
        removed += 1;
    }

    if fs::read_dir(dir)?.next().transpose()?.is_none() {
        fs::remove_dir(dir)?;
    }

    Ok(removed)
}

pub fn cleanup_v1_plaintext_backups() -> io::Result<usize> {
    let home = dirs::home_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine the home directory for legacy SwitchCraft cleanup",
        )
    })?;

    cleanup_v1_plaintext_backups_in(&home.join(".switchcraft").join("backups"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_the_exact_v1_backup_pattern() {
        assert!(is_v1_plaintext_backup_name(
            "codex_20260911_235959_backup.json"
        ));
        assert!(is_v1_plaintext_backup_name(
            "gemini_20260101_000000_backup.json"
        ));
        assert!(is_v1_plaintext_backup_name(
            "claude_20261231_120102_backup.json"
        ));

        assert!(!is_v1_plaintext_backup_name("notes_backup.json"));
        assert!(!is_v1_plaintext_backup_name("codex_personal_backup.json"));
        assert!(!is_v1_plaintext_backup_name(
            "openai_20260911_235959_backup.json"
        ));
        assert!(!is_v1_plaintext_backup_name(
            "codex_20260911-235959_backup.json"
        ));
        assert!(!is_v1_plaintext_backup_name(
            "codex_20260911_235959_backup.json.bak"
        ));
    }

    #[test]
    fn cleanup_removes_only_switchcraft_v1_generated_backups() {
        let temp = tempfile::tempdir().expect("tempdir");
        let backup_dir = temp.path().join("backups");
        fs::create_dir_all(&backup_dir).expect("backup dir");

        let generated = backup_dir.join("codex_20260911_235959_backup.json");
        let unrelated = backup_dir.join("codex_personal_backup.json");
        fs::write(&generated, "{\"token\":\"legacy-secret\"}").expect("legacy backup");
        fs::write(&unrelated, "keep me").expect("unrelated file");

        let removed = cleanup_v1_plaintext_backups_in(&backup_dir).expect("cleanup");

        assert_eq!(removed, 1);
        assert!(!generated.exists());
        assert!(unrelated.exists());
        assert!(backup_dir.exists());
    }

    #[test]
    fn cleanup_removes_empty_legacy_backup_directory() {
        let temp = tempfile::tempdir().expect("tempdir");
        let backup_dir = temp.path().join("backups");
        fs::create_dir_all(&backup_dir).expect("backup dir");
        fs::write(backup_dir.join("claude_20260911_010203_backup.json"), "{}")
            .expect("legacy backup");

        let removed = cleanup_v1_plaintext_backups_in(&backup_dir).expect("cleanup");

        assert_eq!(removed, 1);
        assert!(!backup_dir.exists());
    }
}
