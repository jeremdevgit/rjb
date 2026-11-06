// SPDX-License-Identifier: GPL-2.0-or-later — Copyright (C) 2026 jeremdevgit — voir LICENSE
//! Icône de la zone de notification et son menu.

use crate::langue::{tr, trd};
use crate::engine::Shared;
use crate::{post, Action};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

pub struct Tray {
    icon: Option<TrayIcon>,
    off_item: Option<CheckMenuItem>,
    profile_items: Vec<(String, CheckMenuItem)>,
    last: (bool, String),
}

impl Tray {
    pub fn new(shared: &Shared) -> Tray {
        TrayIconEvent::set_event_handler(Some(|e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                post(Action::Open);
            }
        }));
        MenuEvent::set_event_handler(Some(|e: MenuEvent| {
            let id = e.id.0.as_str();
            let a = match id {
                "open" => Action::Open,
                "off" => Action::ToggleAllOff,
                "rescan" => Action::Rescan,
                "quit" => Action::Quit,
                p if p.starts_with("profile:") => Action::Profile(p["profile:".len()..].to_string()),
                _ => return,
            };
            post(a);
        }));
        let mut t = Tray {
            icon: None,
            off_item: None,
            profile_items: Vec::new(),
            last: (false, String::new()),
        };
        t.rebuild(shared);
        t
    }

    pub fn rebuild(&mut self, shared: &Shared) {
        let (all_off, active, profiles) = {
            let c = shared.config.lock().unwrap();
            (c.all_off, c.active_profile.clone(), c.profiles.keys().cloned().collect::<Vec<_>>())
        };
        let menu = Menu::new();
        let open = MenuItem::with_id("open", tr("Ouvrir rJb"), true, None);
        let off = CheckMenuItem::with_id("off", tr("Tout éteindre"), true, all_off, None);
        let sub = Submenu::new(tr("Profil"), true);
        self.profile_items.clear();
        for p in &profiles {
            let it = CheckMenuItem::with_id(format!("profile:{p}"), trd(p), true, *p == active, None);
            let _ = sub.append(&it);
            self.profile_items.push((p.clone(), it));
        }
        let rescan = MenuItem::with_id("rescan", tr("Rechercher les appareils"), true, None);
        let quit = MenuItem::with_id("quit", tr("Quitter"), true, None);
        let _ = menu.append_items(&[
            &open,
            &PredefinedMenuItem::separator(),
            &off,
            &sub,
            &rescan,
            &PredefinedMenuItem::separator(),
            &quit,
        ]);
        self.off_item = Some(off);
        self.last = (all_off, active);

        if let Some(icon) = &self.icon {
            icon.set_menu(Some(Box::new(menu)));
        } else {
            let rgba = crate::icon::render_icon(32);
            let img = Icon::from_rgba(rgba, 32, 32).ok();
            let mut b = TrayIconBuilder::new()
                .with_tooltip("rJb")
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false);
            if let Some(i) = img {
                b = b.with_icon(i);
            }
            self.icon = b.build().ok();
        }
    }

    /// L'icône n'a pas pu être créée (barre des tâches pas encore prête à
    /// l'ouverture de session) : la boucle principale réessaie.
    pub fn missing(&self) -> bool {
        self.icon.is_none()
    }

    /// Met à jour les coches si l'état a changé (appel peu coûteux).
    pub fn refresh_checks(&mut self, shared: &Shared) {
        let (all_off, active) = {
            let c = shared.config.lock().unwrap();
            (c.all_off, c.active_profile.clone())
        };
        if (all_off, active.clone()) == self.last {
            return;
        }
        if let Some(o) = &self.off_item {
            o.set_checked(all_off);
        }
        for (p, it) in &self.profile_items {
            it.set_checked(*p == active);
        }
        self.last = (all_off, active);
    }
}
