//! "Settings…" (⌘,) in the app menu, where macOS apps put it. Settings is a
//! sheet inside the main window, so the item only tells the page to open it.

use tauri::{
    AppHandle, Emitter, Runtime,
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

const MENU_ID: &str = "open-settings";
/// The event the page listens for; `apps/desktop/src/workbench/backend.ts`
/// names it too.
const OPEN_SETTINGS: &str = "gewu://open-settings";

/// The platform's default menu with "Settings…" after About.
pub fn menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let settings = MenuItem::with_id(app, MENU_ID, "Settings…", true, Some("CmdOrCtrl+,"))?;
    let first = menu.items()?.into_iter().next();
    match first.as_ref().and_then(|item| item.as_submenu()) {
        Some(app_menu) if cfg!(target_os = "macos") => {
            app_menu.insert_items(&[&settings, &PredefinedMenuItem::separator(app)?], 2)?;
        }
        _ => menu.prepend(&settings)?,
    }
    Ok(menu)
}

/// Opens the Settings sheet when `id` is the menu item.
pub fn handle<R: Runtime>(app: &AppHandle<R>, id: &str) {
    if id == MENU_ID
        && let Err(error) = app.emit(OPEN_SETTINGS, ())
    {
        eprintln!("Could not open Settings: {error}");
    }
}
