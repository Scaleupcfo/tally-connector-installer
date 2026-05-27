//! Windows system tray icon + right-click menu.
//!
//! Runs on the main thread because Win32 tray notifications require a thread
//! with a message pump. We use `tao` to provide the event loop (it's what
//! Tauri uses for the same reason) and `tray-icon` for the icon + menu.
//!
//! Menu:
//!   • header (disabled): "Lekha Tally Agent v0.1.0"
//!   • Show pairing token   → opens token.txt in Notepad
//!   • Open data folder     → opens %LOCALAPPDATA%\LekhaTallyInstaller in Explorer
//!   • Quit                 → exits the whole process

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::tls;

/// Build a 32x32 RGBA icon procedurally — solid blue with a darker border.
/// Phase 8 (installer) can replace this with a real PNG of the Lekha logo.
fn build_icon() -> tray_icon::Icon {
    const SIZE: u32 = 32;
    const BLUE: [u8; 4] = [0x4A, 0x90, 0xE2, 0xFF];
    const DARK: [u8; 4] = [0x1A, 0x55, 0xA8, 0xFF];

    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let on_edge = x == 0 || x == SIZE - 1 || y == 0 || y == SIZE - 1;
            rgba.extend_from_slice(if on_edge { &DARK } else { &BLUE });
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
        format!("Lekha Tally Agent v{}", env!("CARGO_PKG_VERSION")),
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
        .with_tooltip("Lekha Tally Agent — local bridge to Tally Prime")
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
                // Open token.txt in Notepad — simplest "show + allow copy" UI.
                let token_path = tls::data_dir().join("token.txt");
                let _ = std::process::Command::new("notepad").arg(&token_path).spawn();
            }
        }
    });
}
