//! Modal capture lifecycle, independent of native webviews for deterministic tests.

use std::collections::HashSet;
use std::time::{Duration, Instant};
use token::model::editor_area::PreviewId;
use token::view::PreviewSnapshots;

const CAPTURE_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(Default)]
pub(super) struct Backdrop {
    pub generation: u64,
    pub active: bool,
    pub images: PreviewSnapshots,
    pending: HashSet<PreviewId>,
    deadline: Option<Instant>,
}

impl Backdrop {
    pub fn begin(&mut self, previews: impl Iterator<Item = PreviewId>, now: Instant) {
        self.generation = self.generation.wrapping_add(1);
        self.active = true;
        self.images.clear();
        self.pending = previews.collect();
        self.deadline = (!self.pending.is_empty()).then_some(now + CAPTURE_TIMEOUT);
    }

    pub fn finish(
        &mut self,
        id: PreviewId,
        generation: u64,
        image: Option<image::RgbaImage>,
    ) -> bool {
        if !self.active || self.generation != generation || !self.pending.remove(&id) {
            return false;
        }
        if let Some(image) = image {
            self.images.insert(id, image);
        }
        if self.pending.is_empty() {
            self.deadline = None;
        }
        true
    }

    pub fn remove(&mut self, id: PreviewId) {
        self.images.remove(&id);
        self.pending.remove(&id);
        if self.pending.is_empty() {
            self.deadline = None;
        }
    }

    pub fn resume(&mut self) {
        self.active = false;
        self.pending.clear();
        self.deadline = None;
        self.images.clear();
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.pending.clear();
            self.deadline = None;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> image::RgbaImage {
        image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]))
    }

    #[test]
    fn waits_for_all_previews_and_keeps_successes_when_one_fails() {
        let mut backdrop = Backdrop::default();
        backdrop.begin([PreviewId(1), PreviewId(2)].into_iter(), Instant::now());
        let generation = backdrop.generation;
        assert!(backdrop.finish(PreviewId(1), generation, Some(image())));
        assert!(backdrop.deadline().is_some());
        assert!(backdrop.finish(PreviewId(2), generation, None));
        assert!(backdrop.deadline().is_none());
        assert_eq!(backdrop.images.len(), 1);
    }

    #[test]
    fn dismissal_and_reopen_reject_previous_capture() {
        let mut backdrop = Backdrop::default();
        backdrop.begin([PreviewId(1)].into_iter(), Instant::now());
        let old = backdrop.generation;
        backdrop.resume();
        assert!(!backdrop.finish(PreviewId(1), old, Some(image())));
        backdrop.begin([PreviewId(1)].into_iter(), Instant::now());
        assert!(!backdrop.finish(PreviewId(1), old, Some(image())));
        assert!(backdrop.finish(PreviewId(1), backdrop.generation, Some(image())));
        backdrop.resume();
        assert!(backdrop.images.is_empty());
    }

    #[test]
    fn timeout_unblocks_modal_and_rejects_late_images() {
        let now = Instant::now();
        let mut backdrop = Backdrop::default();
        backdrop.begin([PreviewId(1)].into_iter(), now);
        assert!(!backdrop.expire(now));
        assert!(backdrop.expire(now + CAPTURE_TIMEOUT));
        assert!(backdrop.deadline().is_none());
        assert!(!backdrop.finish(PreviewId(1), backdrop.generation, Some(image())));
    }

    #[test]
    fn removed_or_refreshed_preview_cannot_accept_stale_image() {
        let mut backdrop = Backdrop::default();
        backdrop.begin([PreviewId(1)].into_iter(), Instant::now());
        backdrop.remove(PreviewId(1));
        assert!(!backdrop.finish(PreviewId(1), backdrop.generation, Some(image())));
        assert!(backdrop.deadline().is_none());
    }
}
