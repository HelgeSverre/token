//! Native webview capture. Call on the UI thread while the view is visible.

use anyhow::{ensure, Context, Result};
use image::{ImageFormat, ImageReader, RgbaImage};
use std::io::Cursor;
use token::util::ByteSize;

pub(super) const MAX_SNAPSHOT_BYTES: ByteSize = ByteSize::mebibytes(64);

fn decode_png(bytes: &[u8]) -> Result<RgbaImage> {
    ensure!(
        bytes.len() <= MAX_SNAPSHOT_BYTES.as_usize(),
        "preview snapshot is too large"
    );
    let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_SNAPSHOT_BYTES.as_u64());
    reader.limits(limits);
    let image = reader.decode().context("decoding preview snapshot")?;
    ensure!(
        u64::from(image.width()) * u64::from(image.height()) * 4 <= MAX_SNAPSHOT_BYTES.as_u64(),
        "preview snapshot dimensions are too large"
    );
    Ok(image.into_rgba8())
}

#[cfg(target_os = "macos")]
pub(super) fn capture(
    webview: &wry::WebView,
    complete: impl FnOnce(Result<RgbaImage>) + 'static,
) -> Result<()> {
    use block2::RcBlock;
    use objc2::AnyThread;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
    use objc2_foundation::{NSDictionary, NSError};
    use std::cell::RefCell;
    use wry::WebViewExtMacOS;

    let complete = RefCell::new(Some(complete));
    let callback = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
        let Some(complete) = complete.borrow_mut().take() else {
            return;
        };
        // SAFETY: WebKit supplies nullable objects valid for this callback on
        // the main thread. We copy the image bytes before returning.
        let result = unsafe {
            (|| {
                if let Some(error) = error.as_ref() {
                    anyhow::bail!("WebKit snapshot: {error}");
                }
                let image = image.as_ref().context("WebKit returned no snapshot")?;
                let data = image
                    .TIFFRepresentation()
                    .context("snapshot has no bitmap")?;
                let bitmap = NSBitmapImageRep::initWithData(NSBitmapImageRep::alloc(), &data)
                    .context("reading snapshot bitmap")?;
                // The empty dictionary contains no incorrectly typed properties.
                let png = bitmap
                    .representationUsingType_properties(
                        NSBitmapImageFileType::PNG,
                        &NSDictionary::new(),
                    )
                    .context("encoding snapshot bitmap")?;
                decode_png(png.as_bytes_unchecked())
            })()
        };
        complete(result);
    });
    // SAFETY: This function is called on the event-loop thread. WebKit copies
    // the callback and a nil configuration captures the visible bounds.
    unsafe {
        webview
            .webview()
            .takeSnapshotWithConfiguration_completionHandler(None, &callback);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub(super) fn capture(
    webview: &wry::WebView,
    complete: impl FnOnce(Result<RgbaImage>) + 'static,
) -> Result<()> {
    use webview2_com::{
        CapturePreviewCompletedHandler,
        Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
    };
    use windows::Win32::System::Com::{
        StructuredStorage::CreateStreamOnHGlobal, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET,
    };
    use wry::WebViewExtWindows;

    // SAFETY: A null HGLOBAL asks COM to allocate an owned stream; it frees the
    // allocation when the final reference (including the callback's) is dropped.
    let stream = unsafe { CreateStreamOnHGlobal(Default::default(), true)? };
    let captured_stream = stream.clone();
    let callback = CapturePreviewCompletedHandler::create(Box::new(move |status| {
        // SAFETY: The callback runs on the WebView's UI thread, after writing
        // finishes. Stat/Read receive valid, correctly sized output buffers.
        let result = unsafe {
            (|| {
                status?;
                let mut stat = STATSTG::default();
                captured_stream.Stat(&mut stat, STATFLAG_NONAME)?;
                ensure!(
                    stat.cbSize <= MAX_SNAPSHOT_BYTES.as_u64(),
                    "preview snapshot is too large"
                );
                captured_stream.Seek(0, STREAM_SEEK_SET, None)?;
                let mut bytes = vec![0; stat.cbSize as usize];
                let mut read = 0;
                captured_stream
                    .Read(
                        bytes.as_mut_ptr().cast(),
                        bytes.len() as u32,
                        Some(&mut read),
                    )
                    .ok()?;
                ensure!(read as usize == bytes.len(), "incomplete preview snapshot");
                decode_png(&bytes)
            })()
        };
        complete(result);
        Ok(())
    }));
    // SAFETY: The stream and handler are retained by WebView2 for the async call.
    unsafe {
        webview.webview().CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &callback,
        )?;
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub(super) fn capture(
    webview: &wry::WebView,
    complete: impl FnOnce(Result<RgbaImage>) + 'static,
) -> Result<()> {
    use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};
    use wry::WebViewExtUnix;

    webview.webview().snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        None::<&webkit2gtk::gio::Cancellable>,
        move |result| {
            complete((|| {
                let surface = result.context("WebKitGTK snapshot")?;
                let mut png = Vec::new();
                surface
                    .write_to_png(&mut png)
                    .context("encoding WebKitGTK snapshot")?;
                decode_png(&png)
            })());
        },
    );
    Ok(())
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
pub(super) fn capture(_: &wry::WebView, _: impl FnOnce(Result<RgbaImage>) + 'static) -> Result<()> {
    anyhow::bail!("native preview capture is unavailable on this platform")
}
