use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{ns_string, MainThreadMarker, NSProcessInfo, NSString};
use std::cell::RefCell;

struct SuspendedShortcut {
    item: Retained<NSMenuItem>,
    key: Retained<NSString>,
    effective_key: Retained<NSString>,
}

struct SuspendedMenu {
    shortcuts: Vec<SuspendedShortcut>,
    uses_user_equivalents: bool,
}

thread_local! {
    static SUSPENDED: RefCell<Option<SuspendedMenu>> = const { RefCell::new(None) };
}

/// AppKit consumes menu accelerators before winit receives KeyboardInput.
/// Suspend only the accelerators while recording, restoring exact originals.
pub(super) fn set_shortcut_capture(capturing: bool) {
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    SUSPENDED.with_borrow_mut(|suspended| {
        if capturing {
            if suspended.is_some() {
                return;
            }
            let Some(menu) = NSApplication::sharedApplication(main_thread).mainMenu() else {
                return;
            };
            let uses_user_equivalents = NSMenuItem::usesUserKeyEquivalents(main_thread);
            let mut shortcuts = Vec::new();
            suspend_menu(&menu, &mut shortcuts, uses_user_equivalents);
            NSMenuItem::setUsesUserKeyEquivalents(false, main_thread);
            *suspended = Some(SuspendedMenu {
                shortcuts,
                uses_user_equivalents,
            });
        } else if let Some(previous) = suspended.take() {
            for shortcut in previous.shortcuts {
                shortcut.item.setKeyEquivalent(&shortcut.key);
            }
            NSMenuItem::setUsesUserKeyEquivalents(previous.uses_user_equivalents, main_thread);
        }
    });
}

/// These accelerators will be restored when capture ends, so recording them
/// would create an unusable override. Native menu remapping is separate work.
pub(super) fn reserved_capture_key(stroke: token::keymap::Keystroke) -> Option<String> {
    use token::keymap::{KeyCode, Modifiers};
    let KeyCode::Char(character) = stroke.key else {
        return None;
    };
    SUSPENDED.with_borrow(|menu| {
        menu.as_ref()?.shortcuts.iter().find_map(|shortcut| {
            let key = shortcut.effective_key.to_string();
            let flags = shortcut.item.keyEquivalentModifierMask();
            let modifiers = Modifiers::new(
                flags.contains(NSEventModifierFlags::Control),
                flags.contains(NSEventModifierFlags::Shift),
                flags.contains(NSEventModifierFlags::Option),
                flags.contains(NSEventModifierFlags::Command),
            );
            (key.chars().count() == 1
                && key
                    .chars()
                    .next()
                    .is_some_and(|key| key.to_ascii_lowercase() == character)
                && stroke.mods == modifiers)
                .then(|| {
                    format!(
                        "Reserved by macOS menu: {}; choose another key",
                        shortcut.item.title()
                    )
                })
        })
    })
}

fn suspend_menu(
    menu: &NSMenu,
    shortcuts: &mut Vec<SuspendedShortcut>,
    uses_user_equivalents: bool,
) {
    for item in menu.itemArray() {
        if let Some(submenu) = item.submenu() {
            suspend_menu(&submenu, shortcuts, uses_user_equivalents);
        }
        let key = item.keyEquivalent();
        let user_key = item.userKeyEquivalent();
        let effective_key = if uses_user_equivalents && user_key.length() != 0 {
            user_key
        } else {
            key.clone()
        };
        if key.length() != 0 || effective_key.length() != 0 {
            item.setKeyEquivalent(ns_string!(""));
            shortcuts.push(SuspendedShortcut {
                item,
                key,
                effective_key,
            });
        }
    }
}

struct KeyEquivalent<'a> {
    key: &'a NSString,
    modifiers: Option<NSEventModifierFlags>,
}

/// Install the standard application menu after the first editor frame.
pub fn install() {
    set_shortcut_capture(false);
    let Some(main_thread) = MainThreadMarker::new() else {
        tracing::warn!("Cannot install the macOS application menu off the main thread");
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);

    // App bundles may already have an empty placeholder menu at this point.
    // Replace it with Token's menu instead of treating its presence as success.
    let menu_bar = NSMenu::new(main_thread);
    let app_menu_item = NSMenuItem::new(main_thread);
    menu_bar.addItem(&app_menu_item);

    let app_menu = NSMenu::new(main_thread);
    let process_name = NSProcessInfo::processInfo().processName();

    let about_title = ns_string!("About ").stringByAppendingString(&process_name);
    app_menu.addItem(&menu_item(
        main_thread,
        &about_title,
        Some(sel!(orderFrontStandardAboutPanel:)),
        None,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(main_thread));

    let services_menu = NSMenu::new(main_thread);
    let services_item = menu_item(main_thread, ns_string!("Services"), None, None);
    services_item.setSubmenu(Some(&services_menu));
    app_menu.addItem(&services_item);

    let hide_title = ns_string!("Hide ").stringByAppendingString(&process_name);
    app_menu.addItem(&menu_item(
        main_thread,
        &hide_title,
        Some(sel!(hide:)),
        Some(KeyEquivalent {
            key: ns_string!("h"),
            modifiers: None,
        }),
    ));
    app_menu.addItem(&menu_item(
        main_thread,
        ns_string!("Hide Others"),
        Some(sel!(hideOtherApplications:)),
        Some(KeyEquivalent {
            key: ns_string!("h"),
            modifiers: Some(NSEventModifierFlags::Option | NSEventModifierFlags::Command),
        }),
    ));
    app_menu.addItem(&menu_item(
        main_thread,
        ns_string!("Show All"),
        Some(sel!(unhideAllApplications:)),
        None,
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(main_thread));

    let quit_title = ns_string!("Quit ").stringByAppendingString(&process_name);
    app_menu.addItem(&menu_item(
        main_thread,
        &quit_title,
        Some(sel!(terminate:)),
        Some(KeyEquivalent {
            key: ns_string!("q"),
            modifiers: None,
        }),
    ));

    app_menu_item.setSubmenu(Some(&app_menu));
    app.setServicesMenu(Some(&services_menu));
    app.setMainMenu(Some(&menu_bar));
}

fn menu_item(
    main_thread: MainThreadMarker,
    title: &NSString,
    selector: Option<Sel>,
    key_equivalent: Option<KeyEquivalent<'_>>,
) -> Retained<NSMenuItem> {
    let (key, modifiers) = match key_equivalent {
        Some(equivalent) => (equivalent.key, equivalent.modifiers),
        None => (ns_string!(""), None),
    };

    // SAFETY: The marker proves main-thread access, and each selector is a
    // standard NSApplication action with the expected single-sender shape.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(main_thread.alloc(), title, selector, key)
    };
    if let Some(modifiers) = modifiers {
        item.setKeyEquivalentModifierMask(modifiers);
    }
    item
}
