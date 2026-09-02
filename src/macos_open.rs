//! Finder / Dock / `open -a Token file` delivery on macOS.
//!
//! LaunchServices hands opened documents to the running application via
//! `application:openURLs:` on the `NSApplicationDelegate`. winit owns the
//! delegate (`WinitApplicationDelegate`) and does not implement that
//! selector, so this adds one to its class at runtime and forwards the
//! paths as an `OpenPaths` automation request — the same path the CLI
//! handoff uses.

use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};

use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{ffi, sel};
use objc2_foundation::{NSArray, NSURL};
use winit::event_loop::EventLoopProxy;

use crate::automation::{AutomationEnvelope, AutomationRequest, OpenPath};

static SINK: OnceLock<Mutex<(Sender<AutomationEnvelope>, EventLoopProxy<()>)>> = OnceLock::new();

/// Register the `application:openURLs:` handler. Must run after the
/// winit event loop is built (that is what registers the delegate class)
/// and before it runs.
pub(crate) fn install(tx: Sender<AutomationEnvelope>, proxy: EventLoopProxy<()>) {
    if SINK.set(Mutex::new((tx, proxy))).is_err() {
        return;
    }
    let Some(class) = AnyClass::get(c"WinitApplicationDelegate") else {
        tracing::warn!("winit application delegate class not found; Finder opens are disabled");
        return;
    };
    let imp: extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>) = open_urls;
    // SAFETY: the class exists and is registered; the selector, the IMP
    // signature `(self, _cmd, NSApplication*, NSArray<NSURL*>*) -> void`
    // and the type encoding "v@:@@" agree, and the method is added before
    // AppKit can dispatch it. Casting the typed fn to `Imp` is the
    // documented way to hand an implementation to the runtime.
    let added = unsafe {
        ffi::class_addMethod(
            class as *const AnyClass as *mut AnyClass,
            sel!(application:openURLs:),
            std::mem::transmute::<
                extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>),
                objc2::runtime::Imp,
            >(imp),
            c"v@:@@".as_ptr(),
        )
    };
    if !added.as_bool() {
        tracing::warn!("could not add application:openURLs:; Finder opens are disabled");
    }
}

extern "C-unwind" fn open_urls(
    _this: &AnyObject,
    _cmd: Sel,
    _app: &AnyObject,
    urls: &NSArray<NSURL>,
) {
    let paths: Vec<OpenPath> = urls
        .iter()
        .filter_map(|url| url.path())
        .map(|path| OpenPath {
            path: path.to_string().into(),
            line: None,
            column: None,
        })
        .collect();
    let Some(sink) = SINK.get() else {
        return;
    };
    let Ok(sink) = sink.lock() else {
        return;
    };
    // Nobody reads the response; the channel just satisfies the envelope.
    let (response_tx, _response_rx) = mpsc::sync_channel(1);
    let request = AutomationRequest::OpenPaths { paths, wait: false };
    if sink
        .0
        .send(AutomationEnvelope {
            request,
            response_tx,
        })
        .is_ok()
    {
        let _ = sink.1.send_event(());
    }
}
