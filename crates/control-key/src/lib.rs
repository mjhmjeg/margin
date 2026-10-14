//! The control channel's key file, one rule for the app (which writes it) and the MCP bridge
//! (which reads it): `<settings>/control-key.<instance>`. See `docs/control-protocol.md`, Keys.

use std::path::{Path, PathBuf};

/// Margin's settings folder name. Margin keeps its own folder, apart from upstream WordCraft's:
/// an installed WordCraft rewrote the shared `ui.json` and dropped Margin's settings
/// (incident 2026-10-14).
pub const APP_DIR: &str = "Margin";
/// The folder Margin shared with WordCraft before; read once by [`migrate`].
pub const LEGACY_DIR: &str = "WordCraft";

/// The app's settings folder (preferences, logs, key files): macOS
/// `~/Library/Application Support/Margin`, Windows `%APPDATA%\Margin`, otherwise
/// `$XDG_CONFIG_HOME/margin` or `~/.config/margin`.
pub fn settings_dir() -> Option<PathBuf> {
    settings_dir_named(APP_DIR)
}

/// The folder Margin used before [`APP_DIR`] (shared with WordCraft).
pub fn legacy_settings_dir() -> Option<PathBuf> {
    settings_dir_named(LEGACY_DIR)
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

/// One-time move to Margin's own folder: when `new` has no `ui.json` yet, copy the preferences
/// and the user dictionary from `old` (copied, not moved: an installed WordCraft keeps its own).
/// Returns the files copied. Never overwrites anything in `new`.
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
        let d = std::env::temp_dir().join(format!("margin-migrate-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn settings_live_in_margins_own_folder_not_wordcrafts() {
        let d = settings_dir().unwrap();
        let name = d.file_name().unwrap().to_string_lossy().to_lowercase();
        assert_eq!(name, "margin");
        assert_ne!(Some(d), legacy_settings_dir());
    }

    #[test]
    fn migrate_copies_once_and_never_overwrites() {
        let (old, new) = (temp("old"), temp("new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("ui.json"), b"{\"assistant\":{\"enabled\":true}}").unwrap();
        std::fs::write(old.join("custom-dictionary.txt"), b"ceramicist\n").unwrap();
        assert_eq!(migrate(&old, &new).unwrap(), 2);
        assert_eq!(std::fs::read(new.join("ui.json")).unwrap(), b"{\"assistant\":{\"enabled\":true}}");
        // WordCraft rewrites its own copy later: Margin's stays as it is.
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
