use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::sel;
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::{ns_string, MainThreadMarker, NSProcessInfo, NSString};

struct KeyEquivalent<'a> {
    key: &'a NSString,
    modifiers: Option<NSEventModifierFlags>,
}

/// Install the standard application menu after the first editor frame.
pub fn install() {
    let Some(main_thread) = MainThreadMarker::new() else {
        tracing::warn!("Cannot install the macOS application menu off the main thread");
        return;
    };
    let app = NSApplication::sharedApplication(main_thread);
    if app.mainMenu().is_some() {
        return;
    }

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
