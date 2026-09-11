//! Transient state for a non-editable dropdown. The owning feature commits values.
#[derive(Default)]
pub struct SelectState {
    pub open: bool,
    pub active: usize,
    pub scroll: usize,
}

impl SelectState {
    pub fn scroll_to(&mut self, first: usize, visible: usize, count: usize) {
        self.scroll = first.min(count.saturating_sub(visible.max(1)));
        self.active = self.active.clamp(
            self.scroll,
            (self.scroll + visible.saturating_sub(1)).min(count.saturating_sub(1)),
        );
    }
    pub fn open(&mut self, selected: usize) {
        self.open = true;
        self.active = selected;
        self.scroll = selected.saturating_sub(4);
    }
    pub fn move_by(&mut self, delta: isize, count: usize) {
        self.active = self
            .active
            .saturating_add_signed(delta)
            .min(count.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_and_scroll_clamp_without_committing_a_value() {
        let mut state = SelectState::default();
        state.open(3);
        state.move_by(-100, 20);
        assert_eq!(state.active, 0);
        state.scroll_to(100, 5, 20);
        assert_eq!((state.scroll, state.active), (15, 15));
        state.move_by(100, 20);
        assert_eq!(state.active, 19);
        state.scroll_to(0, 0, 0);
        assert_eq!((state.scroll, state.active), (0, 0));
    }
}
