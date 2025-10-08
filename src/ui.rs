use crate::{
    commands::{ItemCache, collect_applications, collect_commands, launch_item},
    config::Config,
    error::LauncherError,
    fuzzy,
};
use image::ImageReader;
use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::Transform;
use resvg::usvg;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time,
};
use x11rb::{
    COPY_FROM_PARENT,
    connection::Connection,
    protocol::{Event, xproto::*},
    rust_connection::RustConnection,
};

fn find_icon(icon_name: &str) -> Option<String> {
    if icon_name.contains('/') {
        if std::path::Path::new(icon_name).exists() {
            return Some(icon_name.to_string());
        }
    }

    let home_dir = std::env::var("HOME").unwrap_or_default();
    let icon_themes = [
        format!("{}/.local/share/icons", home_dir),
        "/usr/share/icons/hicolor".to_string(),
        "/usr/share/pixmaps".to_string(),
    ];

    let sizes = [
        "256x256", "128x128", "64x64", "48x48", "32x32", "16x16", "scalable",
    ];
    let exts = [".png", ".svg"];

    for theme in &icon_themes {
        for size in &sizes {
            for ext in &exts {
                let path = format!("{}/{}/apps/{}{}", theme, size, icon_name, ext);
                if std::path::Path::new(&path).exists() {
                    return Some(path);
                }
                let path = format!("{}/{}/devices/{}{}", theme, size, icon_name, ext);
                if std::path::Path::new(&path).exists() {
                    return Some(path);
                }
            }
        }

        for ext in &exts {
            let path = format!("{}/{}{}", theme, icon_name, ext);
            if std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }
    }

    None
}

fn draw_icon(
    conn: &RustConnection,
    window: Window,
    x: i16,
    y: i16,
    size: u16,
    icon_name: &str,
) -> Result<(), LauncherError> {
    if let Some(icon_path) = find_icon(icon_name) {
        let img_data = if icon_path.ends_with(".svg") {
            let mut fontdb = usvg::fontdb::Database::new();
            fontdb.load_system_fonts();
            let svg_data = std::fs::read(&icon_path).map_err(|e| LauncherError::Io(e))?;
            let mut options = usvg::Options::default();
            options.default_size = usvg::Size::from_wh(size as f32, size as f32).unwrap();
            let tree = usvg::Tree::from_data(&svg_data, &options, &fontdb).map_err(|e| {
                LauncherError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;

            let mut pixmap = Pixmap::new(size as u32, size as u32).unwrap();
            resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
            pixmap.data().to_vec()
        } else {
            let img = ImageReader::open(&icon_path)
                .map_err(|e| LauncherError::Io(e))?
                .decode()
                .map_err(|e| {
                    LauncherError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ))
                })?;
            let img = img.thumbnail(size as u32, size as u32).to_rgba8();
            img.into_raw()
        };

        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new().foreground(0))?;

        conn.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            gc,
            size as u16,
            size as u16,
            x,
            y,
            0,
            conn.setup().roots[0].root_depth,
            &img_data,
        )?;
    }
    Ok(())
}

pub fn draw_rect(
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

pub fn draw_text(
    conn: &RustConnection,
    window: Window,
    x: i16,
    y: i16,
    text: &str,
    fg_color: u32,
    bg_color: u32,
) -> Result<(), LauncherError> {
    let gc = conn.generate_id()?;
    conn.create_gc(
        gc,
        window,
        &CreateGCAux::new().foreground(fg_color).background(bg_color),
    )?;
    conn.image_text8(window, gc, x, y, text.as_bytes())?;
    conn.free_gc(gc)?;
    Ok(())
}

const KEYCODE_A: u8 = 38;
const KEYCODE_0: u8 = 10;
const KEYCODE_SPACE: u8 = 65;
const KEYCODE_MINUS: u8 = 20;
const KEYCODE_EQUAL: u8 = 21;
const KEYCODE_COMMA: u8 = 51;
const KEYCODE_DOT: u8 = 52;
const KEYCODE_SLASH: u8 = 53;

pub fn setup_keyboard_map(
    conn: &RustConnection,
) -> Result<HashMap<u8, Vec<String>>, LauncherError> {
    let mut map = HashMap::new();

    let min_keycode = conn.setup().min_keycode;
    let max_keycode = conn.setup().max_keycode;

    let keyboard_mapping_cookie =
        conn.get_keyboard_mapping(min_keycode, (max_keycode - min_keycode + 1) as u8)?;

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
            let keycode = KEYCODE_A + i;
            let lower = ((b'a' + i) as char).to_string();
            let upper = ((b'A' + i) as char).to_string();
            map.insert(keycode, vec![lower, upper]);
        }

        // Numbers
        for i in 0..10 {
            let keycode = KEYCODE_0 + i;
            let num = ((b'0' + i) as char).to_string();
            map.insert(keycode, vec![num.clone(), num]);
        }

        // Common symbols
        map.insert(KEYCODE_SPACE, vec![" ".to_string()]); // Space
        map.insert(KEYCODE_MINUS, vec!["-".to_string(), "_".to_string()]);
        map.insert(KEYCODE_EQUAL, vec!["=".to_string(), "+".to_string()]);
        map.insert(KEYCODE_COMMA, vec![",".to_string(), "<".to_string()]);
        map.insert(KEYCODE_DOT, vec![".".to_string(), ">".to_string()]);
        map.insert(KEYCODE_SLASH, vec!["/".to_string(), "?".to_string()]);
    }

    Ok(map)
}

const KEYSYM_ASCII_START: u32 = 0x0020;
const KEYSYM_ASCII_END: u32 = 0x007E;
const KEYSYM_BACKSPACE: u32 = 0xFF08;
const KEYSYM_TAB: u32 = 0xFF09;
const KEYSYM_ENTER: u32 = 0xFF0D;
const KEYSYM_ESCAPE: u32 = 0xFF1B;
const KEYSYM_ARROW_START: u32 = 0xFF51;
const KEYSYM_ARROW_END: u32 = 0xFF58;

fn keysym_to_char(keysym: u32) -> Option<String> {
    match keysym {
        KEYSYM_ASCII_START..=KEYSYM_ASCII_END => Some((keysym as u8 as char).to_string()), // ASCII printable
        KEYSYM_BACKSPACE => None,                      // Backspace
        KEYSYM_TAB => Some("\t".to_string()),          // Tab
        KEYSYM_ENTER => None,                          // Enter
        KEYSYM_ESCAPE => None,                         // Escape
        KEYSYM_ARROW_START..=KEYSYM_ARROW_END => None, // Arrow keys, etc.
        _ => None,
    }
}

pub fn run_ui(cfg: Config, conn: RustConnection, screen_num: usize) -> Result<(), LauncherError> {
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

    conn.change_window_attributes(win, &ChangeWindowAttributesAux::new().override_redirect(1))?;

    conn.map_window(win)?;
    conn.flush()?;

    std::thread::sleep(std::time::Duration::from_millis(150));

    let grab_cookie = conn.grab_keyboard(
        true, // owner_events
        win,
        x11rb::CURRENT_TIME,
        GrabMode::ASYNC,
        GrabMode::ASYNC,
    )?;
    if grab_cookie.reply()?.status != GrabStatus::SUCCESS {
        return Err(LauncherError::Other("Could not grab keyboard".into()));
    }

    conn.set_input_focus(InputFocus::POINTER_ROOT, win, 0u32)?;
    conn.flush()?;

    draw_rect(&conn, win, 0, 0, cfg.width, cfg.height, cfg.theme.bg_color)?;
    draw_text(
        &conn,
        win,
        (cfg.width / 2 - 80) as i16,
        (cfg.height / 2) as i16,
        "Loading applications...",
        cfg.theme.fg_color,
        cfg.theme.bg_color,
    )?;
    conn.flush()?;

    let cache = Arc::new(Mutex::new(ItemCache::new(cfg.cache_timeout)));

    // Perform initial load synchronously to prevent empty list on first run
    {
        let mut all_items = Vec::new();
        all_items.extend(collect_commands());
        all_items.extend(collect_applications());
        if let Ok(mut cache_guard) = cache.lock() {
            cache_guard.update(all_items);
        }
    }

    let mut query = String::new();
    let mut sel = 0usize;
    let mut start_index = 0usize; // New: start_index
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

        let filtered = fuzzy::fuzzy_search(&query, cache_guard.get(), cfg.max_results);

        // Calculate item_heights for all filtered items
        let item_heights: Vec<u16> = filtered
            .iter()
            .map(|(item, _score)| {
                let has_desc =
                    cfg.show_descriptions && item.description.is_some() && cfg.item_height > 24;
                if has_desc {
                    cfg.item_height + cfg.font_size + cfg.padding / 2
                } else {
                    cfg.item_height
                }
            })
            .collect();

        sel = sel.min(filtered.len().saturating_sub(1));

        // Determine max_visible dynamically based on available height
        let mut current_display_height = 0;
        let mut dynamic_max_visible = 0;
        let query_h = cfg.item_height + cfg.padding;
        let available_display_height = cfg.height.saturating_sub(query_h + cfg.padding * 2);

        for i in start_index..filtered.len() {
            if let Some(item_h) = item_heights.get(i) {
                if current_display_height + *item_h <= available_display_height {
                    current_display_height += *item_h;
                    dynamic_max_visible += 1;
                } else {
                    break;
                }
            }
        }
        // A LOT to fix here
        let max_visible = dynamic_max_visible.max(1); // Ensure at least one item is visible

        // Adjust start_index to keep sel in view
        if sel >= start_index + max_visible {
            // If sel is below the current visible window, scroll down
            start_index = sel - max_visible + 1;
        } else if sel < start_index {
            // If sel is above the current visible window, scroll up
            start_index = sel;
        }
        // Clamp start_index to valid range
        start_index = start_index.min(filtered.len().saturating_sub(max_visible).max(0));

        // Clear background
        draw_rect(&conn, win, 0, 0, cfg.width, cfg.height, cfg.theme.bg_color)?;

        draw_rect(
            &conn,
            win,
            cfg.padding as i16,
            cfg.padding as i16,
            cfg.width - cfg.padding * 2,
            query_h,
            cfg.theme.query_bg,
        )?;

        let prompt = if query.is_empty() {
            "Search applications and commands..."
        } else {
            &format!("❯ {}", query)
        };

        let prompt_color = if query.is_empty() {
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
            cfg.theme.query_bg,
        )?;

        if !query.is_empty() {
            let counter = format!("{} results", filtered.len());
            draw_text(
                &conn,
                win,
                (cfg.width - cfg.padding - 100) as i16,
                (cfg.padding + cfg.font_size + 6) as i16,
                &counter,
                cfg.theme.fg_color,
                cfg.theme.query_bg,
            )?;
        }

        let list_start_y = query_h + cfg.padding * 2;
        let mut current_y = list_start_y;
        for (idx, (item, _score)) in filtered
            .iter()
            .enumerate()
            .skip(start_index)
            .take(max_visible)
        // Use the dynamically calculated max_visible
        {
            let has_desc =
                cfg.show_descriptions && item.description.is_some() && cfg.item_height > 24;
            let current_item_height = if has_desc {
                cfg.item_height + cfg.font_size + cfg.padding / 2 // Increased height for description
            } else {
                cfg.item_height
            };

            let y = current_y;
            let is_selected = idx == sel;

            let (item_bg_color, item_fg_color) = if is_selected {
                (cfg.theme.selected_bg, cfg.theme.selected_fg)
            } else {
                (cfg.theme.bg_color, cfg.theme.fg_color)
            };

            if is_selected {
                draw_rect(
                    &conn,
                    win,
                    cfg.padding as i16,
                    y as i16,
                    cfg.width - cfg.padding * 2,
                    current_item_height,
                    item_bg_color,
                )?;
            }

            let text_start_x = if cfg.show_icons && item.icon.is_some() {
                let icon_size = cfg.item_height - 8; // A bit smaller than item_height
                let icon_x = cfg.padding as i16 + 4;
                let icon_y = y as i16 + 4;
                if let Some(icon_path) = &item.icon {
                    if let Err(e) = draw_icon(&conn, win, icon_x, icon_y, icon_size, icon_path) {
                        eprintln!("Failed to draw icon for {}: {}", item.display_name, e);
                    }
                }
                (icon_x + icon_size as i16 + 8) as i16 // 8px gap after icon
            } else {
                (cfg.padding + 12) as i16 // Default text start
            };

            let type_indicator = match item.item_type {
                crate::commands::ItemType::Application => "App:",
                crate::commands::ItemType::Command => "Cmd:",
            };

            let display_text = format!("{} {}", type_indicator, item.display_name);

            let display_text_y = (y + cfg.padding) as i16; // Position name with padding from top of current_item_height

            draw_text(
                &conn,
                win,
                text_start_x,
                display_text_y,
                &display_text,
                item_fg_color,
                item_bg_color,
            )?;

            // Description if enabled and available
            if has_desc {
                let desc = item.description.as_ref().unwrap();
                let desc = if desc.len() > 60 {
                    format!("{}...", &desc[..57])
                } else {
                    desc.clone()
                };

                let desc_color = if is_selected {
                    item_fg_color
                } else {
                    // Dimmed description color
                    let r = ((cfg.theme.fg_color >> 16) & 0xFF) * 3 / 4;
                    let g = ((cfg.theme.fg_color >> 8) & 0xFF) * 3 / 4;
                    let b = (cfg.theme.fg_color & 0xFF) * 3 / 4;
                    (r << 16) | (g << 8) | b
                };

                let desc_y = (y + cfg.padding + cfg.font_size + cfg.padding / 4) as i16; // Position description below name
                draw_text(
                    &conn,
                    win,
                    text_start_x,
                    desc_y,
                    &desc,
                    desc_color,
                    item_bg_color,
                )?;
            }
            current_y += current_item_height;
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
                        if !filtered.is_empty() && sel + 1 < filtered.len() {
                            sel += 1;
                        }
                    }
                    22 => {
                        // Backspace
                        query.pop();
                        sel = 0;
                        start_index = 0; // Reset start_index on query change
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
