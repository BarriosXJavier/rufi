use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use x11rb::{
    connection::Connection,
    protocol::{xproto::*, Event},
    rust_connection::RustConnection,
    COPY_FROM_PARENT,
};
use crate::{
    commands::{collect_applications, collect_commands, launch_item, ItemCache},
    config::Config,
    error::LauncherError,
    fuzzy,
};
use image::ImageReader;
use resvg::tiny_skia::{Pixmap};
use resvg::usvg;
use resvg::tiny_skia::Transform;


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

    let sizes = ["256x256", "128x128", "64x64", "48x48", "32x32", "16x16", "scalable"];
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
            resvg::render(
                &tree,
                Transform::default(),
                &mut pixmap.as_mut(),
            );
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
    color: u32,
) -> Result<(), LauncherError> {
    let gc = conn.generate_id()?;
    conn.create_gc(gc, window, &CreateGCAux::new().foreground(color))?;
    conn.image_text8(window, gc, x, y, text.as_bytes())?;
    conn.free_gc(gc)?;
    Ok(())
}

pub fn setup_keyboard_map(conn: &RustConnection) -> Result<HashMap<u8, Vec<String>>, LauncherError> {
    let mut map = HashMap::new();

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
        0xFF09 => Some("	".to_string()),                            // Tab
        0xFF0D => None,                                              // Enter
        0xFF1B => None,                                              // Escape
        0xFF51..=0xFF58 => None,                                     // Arrow keys, etc.
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
    conn.set_input_focus(InputFocus::POINTER_ROOT, win, 0u32)?;
    conn.flush()?;

    let cache = Arc::new(Mutex::new(ItemCache::new(cfg.cache_timeout)));

    // background thread
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
        
        let filtered = fuzzy::fuzzy_search(&query, cache_guard.get(), cfg.max_results);

        let max_visible = ((cfg.height.saturating_sub(cfg.padding * 4 + cfg.item_height))
            / cfg.item_height) as usize;
        let max_visible = max_visible.max(1).min(20);

        sel = sel.min(filtered.len().saturating_sub(1));

        // Clear background
        draw_rect(&conn, win, 0, 0, cfg.width, cfg.height, cfg.theme.bg_color)?;

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
            )?;
        }

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
            let text_x_offset = if cfg.show_icons && item.icon.is_some() {
                let icon_size = cfg.item_height - 8; // A bit smaller than item_height
                let icon_x = cfg.padding as i16 + 4;
                let icon_y = y as i16 + 4;
                if let Some(icon_path) = &item.icon {
                    if let Err(e) = draw_icon(&conn, win, icon_x, icon_y, icon_size, icon_path) {
                        eprintln!("Failed to draw icon for {}: {}", item.display_name, e);
                    }
                }
                icon_size as i16 + 8 // Offset for text
            } else {
                0
            };

            let type_indicator = match item.item_type {
                crate::commands::ItemType::Application => "📱",
                crate::commands::ItemType::Command => "⚡",
            };

            let display_text = format!("{} {}", type_indicator, item.display_name);
            draw_text(
                &conn,
                win,
                (cfg.padding + 12) as i16 + text_x_offset,
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
                    (cfg.padding + 32) as i16 + text_x_offset,
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
