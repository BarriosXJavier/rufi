
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    fs,
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use x11rb::{
    COPY_FROM_PARENT,
    connection::Connection,
    protocol::{xproto::*, Event},
    rust_connection::{RustConnection, ConnectError},
};
use thiserror::Error;

#[derive(Debug, Clone)]
struct LaunchItem {
    name: String,
    display_name: String,
    command: String,
    description: Option<String>,
    icon: Option<String>,
    item_type: ItemType,
}

#[derive(Debug, Clone, PartialEq)]
enum ItemType {
    Command,
    Application,
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct Theme {
    bg_color: u32,
    fg_color: u32,
    selected_bg: u32,
    selected_fg: u32,
    border_color: u32,
    query_bg: u32,
    accent_color: u32,
}

#[derive(Deserialize, Debug)]
struct Config {
    font: String,
    font_size: u16,
    width: u16,
    height: u16,
    item_height: u16,
    padding: u16,
    border_width: u16,
    corner_radius: u16,
    max_results: usize,
    show_descriptions: bool,
    show_icons: bool,
    cache_timeout: u64, // seconds
    theme: Theme,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            font: "JetBrains Mono".into(),
            font_size: 14,
            width: 800,
            height: 500,
            item_height: 32,
            padding: 16,
            border_width: 2,
            corner_radius: 12,
            max_results: 50,
            show_descriptions: true,
            show_icons: true,
            cache_timeout: 300,
            theme: Theme {
                bg_color: 0x1e1e2e,      // catppuccin mocha base
                fg_color: 0xcdd6f4,      // catppuccin mocha text
                selected_bg: 0x89b4fa,   // catppuccin mocha blue
                selected_fg: 0x1e1e2e,   // catppuccin mocha base
                border_color: 0x6c7086,  // catppuccin mocha surface2
                query_bg: 0x313244,      // catppuccin mocha surface0
                accent_color: 0xf38ba8,  // catppuccin mocha pink
            },
        }
    }
}

impl Config {
    fn load(path: &str) -> Self {
        match fs::read_to_string(path) {
            Ok(data) => toml::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}

#[derive(Debug, Error)]
enum LauncherError {
    #[error("X11 connection error: {0}")]
    X11Connection(#[from] x11rb::errors::ConnectionError),
    #[error("X11 connect error: {0}")]
    X11Connect(#[from] ConnectError),
    #[error("X11 reply error: {0}")]
    X11Reply(#[from] x11rb::errors::ReplyError),
    #[error("X11 reply or ID error: {0}")]
    X11ReplyOrId(#[from] x11rb::errors::ReplyOrIdError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),
}

struct ItemCache {
    items: Vec<LaunchItem>,
    last_updated: std::time::Instant,
    timeout: Duration,
}

impl ItemCache {
    fn new(timeout_secs: u64) -> Self {
        Self {
            items: Vec::new(),
            last_updated: std::time::Instant::now() - Duration::from_secs(timeout_secs + 1),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    fn is_expired(&self) -> bool {
        self.last_updated.elapsed() > self.timeout
    }

    fn update(&mut self, items: Vec<LaunchItem>) {
        self.items = items;
        self.last_updated = std::time::Instant::now();
    }

    fn get(&self) -> &[LaunchItem] {
        &self.items
    }
}

fn collect_commands() -> Vec<LaunchItem> {
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

fn collect_applications() -> Vec<LaunchItem> {
    let mut items = Vec::new();
    let desktop_dirs = vec![
        "/usr/share/applications".to_string(),
        "/usr/local/share/applications".to_string(),
        format!(
            "{}/.local/share/applications",
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

fn fuzzy_score(query: &str, item: &LaunchItem) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let query = query.to_lowercase();
    let name = item.display_name.to_lowercase();
    let command = item.command.to_lowercase();

    // Boost applications slightly
    let type_bonus = match item.item_type {
        ItemType::Application => 50,
        ItemType::Command => 0,
    };

    // Perfect matches get highest score
    if name == query || command == query {
        return Some(2000 + type_bonus);
    }

    // Prefix matches
    if name.starts_with(&query) {
        return Some(1500 - query.len() as i32 + type_bonus);
    }

    if command.starts_with(&query) {
        return Some(1400 - query.len() as i32 + type_bonus);
    }

    // Substring matches
    if name.contains(&query) {
        return Some(1000 - query.len() as i32 + type_bonus);
    }

    if command.contains(&query) {
        return Some(900 - query.len() as i32 + type_bonus);
    }

    // Description matches
    if let Some(desc) = &item.description {
        let desc = desc.to_lowercase();
        if desc.contains(&query) {
            return Some(600 - query.len() as i32 + type_bonus);
        }
    }

    // Fuzzy matching
    let mut best_score: Option<i32> = None;

    for target in [&name, &command] {
        if let Some(score) = fuzzy_match_score(&query, target) {
            let adjusted_score = score + type_bonus;
            best_score = Some(best_score.map_or(adjusted_score, |s| s.max(adjusted_score)));
        }
    }

    best_score
}

fn fuzzy_match_score(query: &str, target: &str) -> Option<i32> {
    let mut query_chars = query.chars();
    let mut current_char = query_chars.next()?;
    let mut score = 200;
    let mut last_match = 0;
    let mut consecutive = 0;

    for (i, target_char) in target.chars().enumerate() {
        if target_char == current_char {
            let gap = i - last_match;
            if gap == 1 {
                consecutive += 1;
                score += consecutive * 10; // Bonus for consecutive matches
            } else {
                consecutive = 0;
                score -= gap as i32; // Penalize gaps
            }

            last_match = i;
            if let Some(next) = query_chars.next() {
                current_char = next;
            } else {
                return Some(score);
            }
        }
    }

    None
}

fn fuzzy_search(query: &str, items: &[LaunchItem], max_results: usize) -> Vec<(LaunchItem, i32)> {
    let mut scored: Vec<(LaunchItem, i32)> = items
        .iter()
        .filter_map(|item: &LaunchItem| fuzzy_score(query, item).map(|score| (item.clone(), score)))
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(max_results);
    scored
}

fn draw_rect(
    conn: &RustConnection,
    window: Window,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    color: u32,
) -> Result<(), LauncherError> {
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().foreground(color))?;
    conn.poly_fill_rectangle(
        window,
        gc,
        &[Rectangle {
            x,
            y,
            width,
            height,
        }],
    )?;
    conn.free_gc(gc)?;
    Ok(())
}

fn draw_text(
    conn: &RustConnection,
    window: Window,
    x: i16,
    y: i16,
    text: &str,
    color: u32,
) -> Result<(), LauncherError> {
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().foreground(color))?;
    conn.image_text8(window, gc, x, y, text.as_bytes())?;
    conn.free_gc(gc)?;
    Ok(())
}

// Improved keyboard mapping using X11 keysym lookup
fn setup_keyboard_map(conn: &RustConnection) -> Result<HashMap<u8, Vec<String>>, LauncherError> {
    let mut map = HashMap::new();

    // Get keyboard mapping from X11
    let min_keycode = conn.setup().min_keycode;
    let max_keycode = conn.setup().max_keycode;

    let keyboard_mapping_cookie = conn.get_keyboard_mapping(min_keycode, (max_keycode - min_keycode + 1) as u8)?;
    
    if let Ok(keyboard_mapping) = keyboard_mapping_cookie.reply() {
        for keycode in min_keycode..=max_keycode {
            let index = (keycode - min_keycode) as usize;
            let syms_per_keycode = keyboard_mapping.keysyms_per_keycode as usize;

            if index * syms_per_keycode < keyboard_mapping.keysyms.len() {
                let mut variations = Vec::new();

                for i in 0..syms_per_keycode {
                    let sym_index = index * syms_per_keycode + i;
                    if sym_index < keyboard_mapping.keysyms.len() {
                        let keysym = keyboard_mapping.keysyms[sym_index];
                        if let Some(char) = keysym_to_char(keysym) {
                            variations.push(char);
                        }
                    }
                }

                if !variations.is_empty() {
                    map.insert(keycode, variations);
                }
            }
        }
    }

    // Fallback mapping if X11 lookup fails
    if map.is_empty() {
        // Basic ASCII mapping
        for i in 0..26 {
            let keycode = 38 + i;
            let lower = ((b'a' + i) as char).to_string();
            let upper = ((b'A' + i) as char).to_string();
            map.insert(keycode, vec![lower, upper]);
        }

        // Numbers
        for i in 0..10 {
            let keycode = 10 + i;
            let num = ((b'0' + i) as char).to_string();
            map.insert(keycode, vec![num.clone(), num]);
        }

        // Common symbols
        map.insert(65, vec![" ".to_string()]); // Space
        map.insert(20, vec!["-".to_string(), "_".to_string()]);
        map.insert(21, vec!["=".to_string(), "+".to_string()]);
        map.insert(51, vec![",".to_string(), "<".to_string()]);
        map.insert(52, vec!["._".to_string(), ">".to_string()]);
        map.insert(53, vec!["/".to_string(), "?".to_string()]);
    }

    Ok(map)
}

fn keysym_to_char(keysym: u32) -> Option<String> {
    match keysym {
        0x0020..=0x007E => Some((keysym as u8 as char).to_string()), // ASCII printable
        0xFF08 => None,                                              // Backspace
        0xFF09 => Some("\t".to_string()),                            // Tab
        0xFF0D => None,                                              // Enter
        0xFF1B => None,                                              // Escape
        0xFF51..=0xFF58 => None,                                     // Arrow keys, etc.
        _ => None,
    }
}

fn launch_item(item: &LaunchItem) -> Result<(), LauncherError> {
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

fn main() -> Result<(), LauncherError> {
    let cfg_path = dirs::home_dir().unwrap_or_default().join(".rufirc");
    let cfg = Config::load(cfg_path.to_str().unwrap_or(".rufirc"));

    let (conn, screen_num) = RustConnection::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let win = conn.generate_id()?;

    // Center window on screen
    let x = (screen.width_in_pixels.saturating_sub(cfg.width)) / 2;
    let y = (screen.height_in_pixels.saturating_sub(cfg.height)) / 3;

    conn.create_window(
        COPY_FROM_PARENT as u8,
        win,
        screen.root,
        x as i16,
        y as i16,
        cfg.width,
        cfg.height,
        cfg.border_width,
        WindowClass::INPUT_OUTPUT,
        COPY_FROM_PARENT,
        &CreateWindowAux::new()
            .background_pixel(cfg.theme.bg_color)
            .border_pixel(cfg.theme.border_color)
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::STRUCTURE_NOTIFY
                    | EventMask::FOCUS_CHANGE,
            ),
    )?;

    // Make window float above others
    conn.change_window_attributes(win, &ChangeWindowAttributesAux::new().override_redirect(1))?;

    conn.map_window(win)?;
    conn.set_input_focus(InputFocus::POINTER_ROOT, win, 0u32)?;
    conn.flush()?;

    // Initialize cache and load items
    let cache = Arc::new(Mutex::new(ItemCache::new(cfg.cache_timeout)));

    // Load items in background thread
    let cache_clone = cache.clone();
    thread::spawn(move || {
        let mut all_items = Vec::new();
        all_items.extend(collect_commands());
        all_items.extend(collect_applications());

        if let Ok(mut cache_guard) = cache_clone.lock() {
            cache_guard.update(all_items);
        }
    });

    // Wait a bit for initial load
    thread::sleep(Duration::from_millis(100));

    let mut query = String::new();
    let mut sel = 0usize;
    let mut shift_down = false;
    let keymap = setup_keyboard_map(&conn)?;

    println!("rufi launcher started");

    loop {
        let cache_guard = cache.lock().unwrap();

        if cache_guard.is_expired() {
            let reloader_cache = cache.clone();
            thread::spawn(move || {
                let mut new_items = Vec::new();
                new_items.extend(collect_commands());
                new_items.extend(collect_applications());
                if let Ok(mut guard) = reloader_cache.lock() {
                    guard.update(new_items);
                }
            });
        }
        
        // This is the core change. The `fuzzy_search` call is now properly scoped
        // with the `cache_guard` binding.
        let filtered = fuzzy_search(&query, cache_guard.get(), cfg.max_results);

        let max_visible = ((cfg.height.saturating_sub(cfg.padding * 4 + cfg.item_height))
            / cfg.item_height) as usize;
        let max_visible = max_visible.max(1).min(20);

        sel = sel.min(filtered.len().saturating_sub(1));

        // Clear background
        draw_rect(&conn, win, 0, 0, cfg.width, cfg.height, cfg.theme.bg_color)?;

        // Query bar
        let query_h = cfg.item_height + cfg.padding;
        draw_rect(
            &conn,
            win,
            cfg.padding as i16,
            cfg.padding as i16,
            cfg.width - cfg.padding * 2,
            query_h,
            cfg.theme.query_bg,
        )?;

        // Query text with prompt
        let prompt = if query.is_empty() {
            "Search applications and commands..."
        } else {
            &format!("❯ {}", query)
        };

        let prompt_color = if query.is_empty() {
            // Dimmed color for placeholder
            let r = ((cfg.theme.fg_color >> 16) & 0xFF) / 2;
            let g = ((cfg.theme.fg_color >> 8) & 0xFF) / 2;
            let b = (cfg.theme.fg_color & 0xFF) / 2;
            (r << 16) | (g << 8) | b
        } else {
            cfg.theme.accent_color
        };

        draw_text(
            &conn,
            win,
            (cfg.padding + 12) as i16,
            (cfg.padding + cfg.font_size + 6) as i16,
            prompt,
            prompt_color,
        )?;

        // Results counter
        if !query.is_empty() {
            let counter = format!("{} results", filtered.len());
            draw_text(
                &conn,
                win,
                (cfg.width - cfg.padding - 100) as i16,
                (cfg.padding + cfg.font_size + 6) as i16,
                &counter,
                cfg.theme.fg_color,
            )?;
        }

        // Items list
        let list_start_y = query_h + cfg.padding * 2;
        for (i, (item, _score)) in filtered.iter().enumerate().take(max_visible) {
            let y = list_start_y + (i as u16 * cfg.item_height);
            let is_selected = i == sel;

            if is_selected {
                draw_rect(
                    &conn,
                    win,
                    cfg.padding as i16,
                    y as i16,
                    cfg.width - cfg.padding * 2,
                    cfg.item_height,
                    cfg.theme.selected_bg,
                )?;
            }

            let fg_color = if is_selected {
                cfg.theme.selected_fg
            } else {
                cfg.theme.fg_color
            };

            // Item type indicator and name
            let type_indicator = match item.item_type {
                ItemType::Application => "📱",
                ItemType::Command => "⚡",
            };

            let display_text = format!("{} {}", type_indicator, item.display_name);
            draw_text(
                &conn,
                win,
                (cfg.padding + 12) as i16,
                (y + cfg.font_size + 8) as i16,
                &display_text,
                fg_color,
            )?;

            // Description if enabled and available
            if cfg.show_descriptions && item.description.is_some() && cfg.item_height > 24 {
                let desc = item.description.as_ref().unwrap();
                let desc = if desc.len() > 60 {
                    format!("{}...", &desc[..57])
                } else {
                    desc.clone()
                };

                let desc_color = if is_selected {
                    cfg.theme.selected_fg
                } else {
                    // Dimmed description color
                    let r = ((cfg.theme.fg_color >> 16) & 0xFF) * 3 / 4;
                    let g = ((cfg.theme.fg_color >> 8) & 0xFF) * 3 / 4;
                    let b = (cfg.theme.fg_color & 0xFF) * 3 / 4;
                    (r << 16) | (g << 8) | b
                };

                draw_text(
                    &conn,
                    win,
                    (cfg.padding + 32) as i16,
                    (y + cfg.font_size + 20) as i16,
                    &desc,
                    desc_color,
                )?;
            }
        }

        conn.flush()?;

        let ev = conn.wait_for_event()?;
        match ev {
            Event::KeyPress(k) => {
                let code = k.detail;
                match code {
                    9 => break, // ESC
                    36 => {
                        // Enter
                        if let Some((item, _)) = filtered.get(sel) {
                            println!("Launching: {} ({})", item.display_name, item.command);
                            if let Err(e) = launch_item(item) {
                                eprintln!("Failed to launch {}: {}", item.display_name, e);
                            }
                        }
                        break;
                    }
                    111 => {
                        // Up
                        if sel > 0 {
                            sel -= 1;
                        }
                    }
                    116 => {
                        // Down
                        if !filtered.is_empty() && sel + 1 < filtered.len().min(max_visible) {
                            sel += 1;
                        }
                    }
                    22 => {
                        // Backspace
                        query.pop();
                        sel = 0;
                    }
                    50 | 62 => {
                        // Shift (left/right)
                        shift_down = true;
                    }
                    _ => {
                        if let Some(variations) = keymap.get(&code) {
                            let variation_index = if shift_down && variations.len() > 1 {
                                1
                            } else {
                                0
                            };
                            if let Some(ch) = variations.get(variation_index) {
                                query.push_str(ch);
                                sel = 0;
                            }
                        }
                    }
                }
            }
            Event::KeyRelease(k) => {
                if k.detail == 50 || k.detail == 62 {
                    shift_down = false;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
