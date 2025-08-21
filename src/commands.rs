use crate::error::LauncherError;
use std::{
    env,
    ffi::OsStr,
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct LaunchItem {
    pub name: String,
    pub display_name: String,
    pub command: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub item_type: ItemType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemType {
    Command,
    Application,
}

pub struct ItemCache {
    pub items: Vec<LaunchItem>,
    last_updated: Instant,
    timeout: Duration,
}

impl ItemCache {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            items: Vec::new(),
            last_updated: Instant::now() - Duration::from_secs(timeout_secs + 1),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.last_updated.elapsed() > self.timeout
    }

    pub fn update(&mut self, items: Vec<LaunchItem>) {
        self.items = items;
        self.last_updated = Instant::now();
    }

    pub fn get(&self) -> &[LaunchItem] {
        &self.items
    }
}

pub fn collect_commands() -> Vec<LaunchItem> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Ok(path_var) = env::var("PATH") {
        for dir in path_var.split(':') {
            if dir.is_empty() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && is_executable(&path) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if !name.starts_with('.') && seen.insert(name.to_string()) {
                                items.push(LaunchItem {
                                    name: name.to_string(),
                                    display_name: name.to_string(),
                                    command: name.to_string(),
                                    description: None,
                                    icon: None,
                                    item_type: ItemType::Command,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    items.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    items
}

pub fn collect_applications() -> Vec<LaunchItem> {
    let mut items = Vec::new();
    let desktop_dirs = vec![
        "/usr/share/applications".to_string(),
        "/usr/local/share/applications".to_string(),
        format!(
            "{}/.local/share/applications",
            env::var("HOME").unwrap_or_default()
        ),
        "/var/lib/flatpak/exports/share/applications".to_string(),
        format!(
            "{}/.local/share/flatpak/exports/share/applications",
            env::var("HOME").unwrap_or_default()
        ),
    ];

    for dir in desktop_dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension() == Some(OsStr::new("desktop")) {
                    if let Some(app) = parse_desktop_entry(&path) {
                        items.push(app);
                    }
                }
            }
        }
    }

    items.sort_unstable_by(|a, b| a.display_name.cmp(&b.display_name));
    items
}

fn parse_desktop_entry(path: &Path) -> Option<LaunchItem> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut comment = None;
    let mut icon = None;
    let mut no_display = false;
    let mut hidden = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("NoDisplay=true") {
            no_display = true;
        } else if line.starts_with("Hidden=true") {
            hidden = true;
        } else if line.starts_with("Name=") && name.is_none() {
            name = Some(line[5..].to_string());
        } else if line.starts_with("Exec=") {
            exec = Some(line[5..].to_string());
        } else if line.starts_with("Comment=") {
            comment = Some(line[8..].to_string());
        } else if line.starts_with("Icon=") {
            icon = Some(line[5..].to_string());
        }
    }

    if no_display || hidden {
        return None;
    }

    let name = name?;
    let exec = exec?;

    // Clean up exec command (remove %u, %f, etc.)
    let exec = exec
        .split_whitespace()
        .filter(|&arg| !arg.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ");

    Some(LaunchItem {
        name: name.clone(),
        display_name: name,
        command: exec,
        description: comment,
        icon,
        item_type: ItemType::Application,
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

pub fn launch_item(item: &LaunchItem) -> Result<(), LauncherError> {
    // Parse command for shell execution
    if item.command.contains(' ') || item.command.contains('&') || item.command.contains(';') {
        Command::new("sh")
            .arg("-c")
            .arg(&item.command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    } else {
        Command::new(&item.command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }
    Ok(())
}
