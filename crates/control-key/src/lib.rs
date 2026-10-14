//! The control channel's key file, one rule for the app (which writes it) and the MCP bridge
//! (which reads it): `<settings>/control-key.<instance>`. See `docs/control-protocol.md`, Keys.

use std::path::{Path, PathBuf};

/// Optimo's settings folder name. Optimo keeps its own folder, apart from upstream WordCraft's:
/// an installed WordCraft rewrote the shared `ui.json` and dropped the fork's settings
/// (incident 2026-10-14). The app was called Margin until 2026-10-14.
pub const APP_DIR: &str = "Optimo";
/// The folders the app used before [`APP_DIR`], newest first: Margin's own folder (until the
/// rename, 2026-10-14), then the one shared with WordCraft. [`migrate_from_previous`] copies from
/// the first that has preferences.
pub const PREVIOUS_DIRS: [&str; 2] = ["Margin", "WordCraft"];

/// The app's settings folder (preferences, logs, key files): macOS
/// `~/Library/Application Support/Optimo`, Windows `%APPDATA%\Optimo`, otherwise
/// `$XDG_CONFIG_HOME/optimo` or `~/.config/optimo`.
pub fn settings_dir() -> Option<PathBuf> {
    settings_dir_named(APP_DIR)
}

/// The folders of [`PREVIOUS_DIRS`], newest first.
pub fn previous_settings_dirs() -> Vec<PathBuf> {
    PREVIOUS_DIRS.iter().filter_map(|n| settings_dir_named(n)).collect()
}

fn settings_dir_named(name: &str) -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support").join(name))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(name))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|c| c.join(name.to_lowercase()))
    }
}

/// Files [`migrate`] carries over from the old folder.
const MIGRATED: [&str; 2] = ["ui.json", "custom-dictionary.txt"];

/// One-time move to the app's own folder: when `new` has no `ui.json` yet, copy the preferences
/// and the user dictionary from `old` (copied, not moved: the old folder stays as a backup and an
/// installed WordCraft keeps its own). Returns the files copied. Never overwrites anything in `new`.
pub fn migrate(old: &Path, new: &Path) -> std::io::Result<usize> {
    if new.join("ui.json").exists() || !old.join("ui.json").is_file() {
        return Ok(0);
    }
    std::fs::create_dir_all(new)?;
    let mut n = 0;
    for f in MIGRATED {
        let (from, to) = (old.join(f), new.join(f));
        if from.is_file() && !to.exists() {
            std::fs::copy(&from, &to)?;
            n += 1;
        }
    }
    Ok(n)
}

/// [`migrate`] from the first of `previous` (newest first) that has a `ui.json`: Optimo ← Margin,
/// else Optimo ← WordCraft. Returns the folder copied from and the files copied; `None` when
/// `new` already has preferences or no previous folder has any.
pub fn migrate_from_previous(previous: &[PathBuf], new: &Path) -> std::io::Result<Option<(PathBuf, usize)>> {
    if new.join("ui.json").exists() {
        return Ok(None);
    }
    let Some(old) = previous.iter().find(|d| d.join("ui.json").is_file()) else { return Ok(None) };
    Ok(Some((old.clone(), migrate(old, new)?)))
}

/// The `instance-id=` value of a `/.flatpak-info` file, only when it is `[A-Za-z0-9_-]+` (it
/// becomes part of a file name).
pub fn parse_instance_id(info: &str) -> Option<String> {
    let v = info.lines().find_map(|l| l.strip_prefix("instance-id="))?.trim();
    (!v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')).then(|| v.to_string())
}

/// The Flatpak instance id, or `None` outside Flatpak.
pub fn instance_id() -> Option<String> {
    parse_instance_id(&std::fs::read_to_string("/.flatpak-info").ok()?)
}

/// `<settings>/control-key.<instance>`, with `<instance>` the Flatpak instance id or
/// `local-<port>`.
pub fn key_path(settings: &Path, port: u16) -> PathBuf {
    key_path_for(settings, instance_id().as_deref(), port)
}

fn key_path_for(settings: &Path, instance: Option<&str>, port: u16) -> PathBuf {
    match instance {
        Some(id) => settings.join(format!("control-key.{id}")),
        None => settings.join(format!("control-key.local-{port}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("optimo-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn settings_live_in_optimos_own_folder_not_margins_or_wordcrafts() {
        let d = settings_dir().unwrap();
        let name = d.file_name().unwrap().to_string_lossy().to_lowercase();
        assert_eq!(name, "optimo");
        let previous = previous_settings_dirs();
        assert_eq!(previous.len(), 2);
        assert!(!previous.contains(&d));
        let names: Vec<String> = previous.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_lowercase()).collect();
        assert_eq!(names, ["margin", "wordcraft"], "newest first");
    }

    #[test]
    fn migrate_copies_once_and_never_overwrites() {
        let (old, new) = (temp("old"), temp("new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("ui.json"), b"{\"assistant\":{\"enabled\":true}}").unwrap();
        std::fs::write(old.join("custom-dictionary.txt"), b"ceramicist\n").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), 2);
        assert_eq!(std::fs::read(new.join("ui.json")).unwrap(), b"{\"assistant\":{\"enabled\":true}}");
        // The old app rewrites its own copy later: the new one stays as it is.
        std::fs::write(old.join("ui.json"), b"{}").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), 0);
        assert_eq!(std::fs::read(new.join("ui.json")).unwrap(), b"{\"assistant\":{\"enabled\":true}}");
        // Nothing to migrate from: nothing happens.
        let (none, fresh) = (temp("none"), temp("fresh"));
        assert_eq!(migrate(&none, &fresh).unwrap(), 0);
        assert!(!fresh.exists());
        for d in [old, new, fresh] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Hop 1: Margin → Optimo. Margin's folder is the newest, so it wins over WordCraft's.
    #[test]
    fn hop_margin_to_optimo_is_preferred_and_keeps_the_old_folder() {
        let (margin, wordcraft, optimo) = (temp("hop1-margin"), temp("hop1-wordcraft"), temp("hop1-optimo"));
        std::fs::create_dir_all(&margin).unwrap();
        std::fs::create_dir_all(&wordcraft).unwrap();
        std::fs::write(margin.join("ui.json"), b"{\"from\":\"margin\"}").unwrap();
        std::fs::write(margin.join("custom-dictionary.txt"), b"glaze\n").unwrap();
        std::fs::write(wordcraft.join("ui.json"), b"{\"from\":\"wordcraft\"}").unwrap();
        let previous = [margin.clone(), wordcraft.clone()];
        let (from, n) = migrate_from_previous(&previous, &optimo).unwrap().unwrap();
        assert_eq!((from, n), (margin.clone(), 2));
        assert_eq!(std::fs::read(optimo.join("ui.json")).unwrap(), b"{\"from\":\"margin\"}");
        assert_eq!(std::fs::read(optimo.join("custom-dictionary.txt")).unwrap(), b"glaze\n");
        assert!(margin.join("ui.json").is_file(), "copied, not moved: Margin's folder stays as a backup");
        // Once Optimo has preferences, nothing is copied again (never overwritten).
        std::fs::write(optimo.join("ui.json"), b"{\"from\":\"optimo\"}").unwrap();
        assert!(migrate_from_previous(&previous, &optimo).unwrap().is_none());
        assert_eq!(std::fs::read(optimo.join("ui.json")).unwrap(), b"{\"from\":\"optimo\"}");
        for d in [margin, wordcraft, optimo] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// Hop 2: WordCraft → Optimo, for someone who never ran Margin's own folder.
    #[test]
    fn hop_wordcraft_to_optimo_when_there_is_no_margin_folder() {
        let (margin, wordcraft, optimo) = (temp("hop2-margin"), temp("hop2-wordcraft"), temp("hop2-optimo"));
        std::fs::create_dir_all(&wordcraft).unwrap();
        std::fs::write(wordcraft.join("ui.json"), b"{\"from\":\"wordcraft\"}").unwrap();
        // A Margin folder without preferences (only a log) doesn't count.
        std::fs::create_dir_all(&margin).unwrap();
        std::fs::write(margin.join("wordcraft.log"), b"log").unwrap();
        let previous = [margin.clone(), wordcraft.clone()];
        let (from, n) = migrate_from_previous(&previous, &optimo).unwrap().unwrap();
        assert_eq!((from, n), (wordcraft.clone(), 1));
        assert_eq!(std::fs::read(optimo.join("ui.json")).unwrap(), b"{\"from\":\"wordcraft\"}");
        assert!(wordcraft.join("ui.json").is_file());
        // Nothing anywhere: nothing happens, no folder is made.
        let (e1, e2, fresh) = (temp("hop2-e1"), temp("hop2-e2"), temp("hop2-fresh"));
        assert!(migrate_from_previous(&[e1, e2], &fresh).unwrap().is_none());
        assert!(!fresh.exists());
        for d in [margin, wordcraft, optimo] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    use std::path::Path;

    #[test]
    fn instance_id_only_safe_values() {
        assert_eq!(parse_instance_id("[Instance]\ninstance-id=abc_123-X\n").as_deref(), Some("abc_123-X"));
        assert_eq!(parse_instance_id("instance-id= 42 \n").as_deref(), Some("42"));
        // It ends up in a file name.
        assert_eq!(parse_instance_id("instance-id=../etc\n"), None);
        assert_eq!(parse_instance_id("instance-id=a/b\n"), None);
        assert_eq!(parse_instance_id("instance-id=a b\n"), None);
        assert_eq!(parse_instance_id("instance-id=..\n"), None);
        assert_eq!(parse_instance_id("instance-id=\n"), None);
        assert_eq!(parse_instance_id("app=ai.storyteller.wordcraft\n"), None);
    }

    #[test]
    fn key_file_per_instance() {
        let dir = Path::new("/settings/wordcraft");
        assert_eq!(key_path_for(dir, Some("1234"), 7981), dir.join("control-key.1234"));
        assert_eq!(key_path_for(dir, None, 7981), dir.join("control-key.local-7981"));
        assert_eq!(key_path_for(dir, None, 7982), dir.join("control-key.local-7982"));
        if instance_id().is_none() {
            assert_eq!(key_path(dir, 7981), dir.join("control-key.local-7981"));
        }
    }
}
