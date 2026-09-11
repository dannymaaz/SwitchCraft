use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    if !path.exists() {
        return Ok(None);
    }

    fs::read(path)
        .map(Some)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid destination path: {}", path.display()))?;

    fs::create_dir_all(parent)
        .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;

    let mut temp = NamedTempFile::new_in(parent)
        .map_err(|e| format!("Could not create temporary file in {}: {e}", parent.display()))?;

    temp.write_all(bytes)
        .map_err(|e| format!("Could not write temporary credentials file: {e}"))?;
    temp.flush()
        .map_err(|e| format!("Could not flush temporary credentials file: {e}"))?;
    temp.as_file()
        .sync_all()
        .map_err(|e| format!("Could not sync temporary credentials file: {e}"))?;

    temp.persist(path)
        .map_err(|e| format!("Could not atomically replace {}: {}", path.display(), e.error))?;

    #[cfg(unix)]
    {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    Ok(())
}

pub fn restore(path: &Path, previous: Option<&[u8]>) -> Result<(), String> {
    match previous {
        Some(bytes) => atomic_write(path, bytes),
        None => {
            if path.exists() {
                fs::remove_file(path)
                    .map_err(|e| format!("Could not remove {} during rollback: {e}", path.display()))?;
            }
            Ok(())
        }
    }
}
