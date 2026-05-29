//! Windows system tray icon + right-click menu.
//!
//! Runs on the main thread because Win32 tray notifications require a thread
//! with a message pump. We use `tao` to provide the event loop (it's what
//! Tauri uses for the same reason) and `tray-icon` for the icon + menu.
//!
//! Menu:
//!   - header (disabled): "Lekha AI Tally Connector v..."
//!   - Show pairing token   -> opens token.txt in Notepad
//!   - Open data folder     -> opens data dir in Explorer
//!   - Quit                 -> exits the whole process

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::tls;

/// Build a 32x32 RGBA icon — Lekha AI branded "L" letterform.
/// Lime-green background (#C5E84D) with dark ink (#101012) "L" shape.
fn build_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    const LIME: [u8; 4] = [0xC5, 0xE8, 0x4D, 0xFF]; // #C5E84D
    const INK: [u8; 4] = [0x10, 0x10, 0x12, 0xFF];   // #101012

    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    // Fill background with lime
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&LIME);
    }

    // Draw "L" shape in ink:
    //   vertical stroke: columns 8-12, rows 6-25
    //   horizontal stroke: columns 8-23, rows 22-25
    for y in 0..SIZE {
        for x in 0..SIZE {
            let in_vertical = x >= 8 && x < 13 && y >= 6 && y < 26;
            let in_horizontal = x >= 8 && x < 24 && y >= 22 && y < 26;
            if in_vertical || in_horizontal {
                let idx = ((y * SIZE + x) * 4) as usize;
                rgba[idx..idx + 4].copy_from_slice(&INK);
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE, SIZE).expect("build icon")
}

/// Take over the main thread with the tray + event loop.
/// Never returns — `Quit` calls `std::process::exit(0)`.
pub fn run_event_loop() -> ! {
    let event_loop = EventLoopBuilder::new().build();

    // ----- Menu items -----
    let header = MenuItem::new(
        format!("Lekha AI Tally Connector v{}", env!("CARGO_PKG_VERSION")),
        false, // disabled — informational only
        None,
    );
    let show_token = MenuItem::new("Show pairing token", true, None);
    let open_folder = MenuItem::new("Open data folder", true, None);
    let quit = MenuItem::new("Quit", true, None);

    let menu = Menu::new();
    menu.append_items(&[
        &header,
        &PredefinedMenuItem::separator(),
        &show_token,
        &open_folder,
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .expect("append menu items");

    // ----- Build the tray icon (must outlive the event loop -> hold a ref) -----
    let _tray = TrayIconBuilder::new()
        .with_tooltip("Lekha AI — Tally Connector")
        .with_icon(build_icon())
        .with_menu(Box::new(menu))
        .build()
        .expect("build tray icon");

    // ----- Pre-grab IDs we'll compare against in the event loop -----
    let id_show_token = show_token.id().clone();
    let id_open_folder = open_folder.id().clone();
    let id_quit = quit.id().clone();

    let menu_channel = MenuEvent::receiver();

    // ----- Hand the main thread to the event loop -----
    event_loop.run(move |_event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Ok(ev) = menu_channel.try_recv() {
            if ev.id == id_quit {
                std::process::exit(0);
            } else if ev.id == id_open_folder {
                let path = tls::data_dir();
                let _ = std::process::Command::new("explorer").arg(&path).spawn();
            } else if ev.id == id_show_token {
                let token_path = tls::data_dir().join("token.txt");
                let _ = std::process::Command::new("notepad").arg(&token_path).spawn();
            }
        }
    });
}
