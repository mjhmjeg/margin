//! The native macOS menu bar, built from `wordcraft_ui_egui::mac_menu::MENUS` with muda (AppKit
//! interop behind a safe API). Choosing an item queues its action; `poll` runs it on the next frame
//! and keeps check marks (Track Changes, Pageless, Classic Ribbon…) in step with the app.

use std::collections::HashMap;
use std::sync::{Mutex, mpsc};

use muda::accelerator::Accelerator;
use muda::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use wordcraft_ui_egui::WordApp;
use wordcraft_ui_egui::mac_menu::{Act, Entry, MENUS, Std, checked, perform};

pub struct NativeMenu {
    _menu: Menu,
    acts: HashMap<MenuId, Act>,
    checks: Vec<(CheckMenuItem, Act, bool)>,
    rx: mpsc::Receiver<MenuId>,
}

impl NativeMenu {
    /// Build the menu bar and make it the app's. Call on the main thread once AppKit is running
    /// (eframe's app creator).
    pub fn install(ctx: &egui::Context, app: &WordApp) -> Option<NativeMenu> {
        let menu = Menu::new();
        let mut acts = HashMap::new();
        let mut checks = Vec::new();
        let mut window_menu = None;
        let mut help_menu = None;
        for (title, entries) in MENUS {
            let sub = Submenu::new(*title, true);
            fill(&sub, entries, app, &mut acts, &mut checks);
            if let Err(e) = menu.append(&sub) {
                log::warn!("menu {title}: {e}");
            }
            match *title {
                "Window" => window_menu = Some(sub),
                "Help" => help_menu = Some(sub),
                _ => {}
            }
        }
        menu.init_for_nsapp();
        log::info!("menu bar installed: {} menus, {} items, {} with check marks", MENUS.len(), acts.len(), checks.len());
        if let Some(w) = &window_menu {
            w.set_as_windows_menu_for_nsapp();
        }
        if let Some(h) = &help_menu {
            h.set_as_help_menu_for_nsapp();
        }
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Ok(tx) = tx.lock() {
                let _ = tx.send(e.id);
            }
            ctx.request_repaint();
        }));
        Some(NativeMenu { _menu: menu, acts, checks, rx })
    }

    /// Run the chosen items and refresh check marks.
    pub fn poll(&mut self, app: &mut WordApp) {
        while let Ok(id) = self.rx.try_recv() {
            if let Some(act) = self.acts.get(&id) {
                perform(app, act);
            }
        }
        for (item, act, shown) in &mut self.checks {
            let now = checked(app, act).unwrap_or(false);
            if now != *shown {
                item.set_checked(now);
                *shown = now;
            }
        }
    }
}

fn fill(sub: &Submenu, entries: &[Entry], app: &WordApp, acts: &mut HashMap<MenuId, Act>, checks: &mut Vec<(CheckMenuItem, Act, bool)>) {
    for e in entries {
        let r = match e {
            Entry::Sep => sub.append(&PredefinedMenuItem::separator()),
            Entry::Sub(title, inner) => {
                let s = Submenu::new(*title, true);
                fill(&s, inner, app, acts, checks);
                sub.append(&s)
            }
            Entry::Item(_, Act::Std(std), _) => sub.append(&standard(*std)),
            Entry::Item(label, act, accel) => {
                let accel = accel.and_then(|a| a.parse::<Accelerator>().map_err(|e| log::warn!("accelerator {a}: {e}")).ok());
                if let Some(on) = checked(app, act) {
                    let item = CheckMenuItem::new(*label, true, on, accel);
                    acts.insert(item.id().clone(), *act);
                    let r = sub.append(&item);
                    checks.push((item, *act, on));
                    r
                } else {
                    let item = MenuItem::new(*label, true, accel);
                    acts.insert(item.id().clone(), *act);
                    sub.append(&item)
                }
            }
        };
        if let Err(e) = r {
            log::warn!("menu item: {e}");
        }
    }
}

fn standard(s: Std) -> PredefinedMenuItem {
    match s {
        Std::About => PredefinedMenuItem::about(
            Some("About Margin"),
            Some(muda::AboutMetadata { name: Some("Margin".into()), version: Some(env!("CARGO_PKG_VERSION").into()), ..Default::default() }),
        ),
        Std::Services => PredefinedMenuItem::services(None),
        Std::Hide => PredefinedMenuItem::hide(Some("Hide Margin")),
        Std::HideOthers => PredefinedMenuItem::hide_others(None),
        Std::ShowAll => PredefinedMenuItem::show_all(None),
        Std::Quit => PredefinedMenuItem::quit(Some("Quit Margin")),
        Std::Minimize => PredefinedMenuItem::minimize(None),
        Std::Zoom => PredefinedMenuItem::maximize(Some("Zoom")),
        Std::Fullscreen => PredefinedMenuItem::fullscreen(None),
        Std::BringAllToFront => PredefinedMenuItem::bring_all_to_front(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every accelerator in the menu data parses (a typo would silently drop the shortcut).
    #[test]
    fn accelerators_parse() {
        fn walk(entries: &[Entry]) {
            for e in entries {
                match e {
                    Entry::Item(l, _, Some(a)) => assert!(a.parse::<Accelerator>().is_ok(), "{l}: {a}"),
                    Entry::Sub(_, s) => walk(s),
                    _ => {}
                }
            }
        }
        for (_, m) in MENUS {
            walk(m);
        }
    }
}
