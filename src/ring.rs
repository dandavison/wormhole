//! Projects in two orders, both with the current project at index 0.
//!
//! The ring order is what the Hammerspoon switcher (ctrl+cmd+left/right) moves
//! around: stepping rotates it, so repeated steps visit every project in turn.
//! An explicit switch moves the target to the front with the project left at
//! index 1, so a single previous step returns to it.
//!
//! The recency order is most-recently-used, for pickers: index 1 is always the
//! project left, however it was left.

use std::collections::VecDeque;
use std::ops::Deref;

use crate::project::ProjectKey;

#[derive(Default)]
pub struct Ring {
    keys: VecDeque<ProjectKey>,
    recent: VecDeque<ProjectKey>,
}

#[derive(Clone, Copy)]
pub enum Direction {
    Previous,
    Next,
}

impl Ring {
    pub fn bring_to_front(&mut self, key: &ProjectKey) {
        self.keys.retain(|k| k != key);
        self.keys.push_back(key.clone());
        self.keys.rotate_right(1);
        self.make_most_recent(key);
    }

    /// Rotate to the nearest eligible project in `direction` and return it.
    pub fn step(
        &mut self,
        direction: Direction,
        is_eligible: impl Fn(&ProjectKey) -> bool,
    ) -> Option<ProjectKey> {
        let reached = (0..self.keys.len()).find_map(|_| {
            match direction {
                Direction::Previous => self.keys.rotate_left(1),
                Direction::Next => self.keys.rotate_right(1),
            }
            self.keys.front().filter(|k| is_eligible(k)).cloned()
        })?;
        self.make_most_recent(&reached);
        Some(reached)
    }

    pub fn recent(&self) -> impl Iterator<Item = &ProjectKey> {
        self.recent.iter()
    }

    pub fn push_front(&mut self, key: ProjectKey) {
        self.keys.push_front(key.clone());
        self.recent.push_front(key);
    }

    pub fn push_back(&mut self, key: ProjectKey) {
        self.keys.push_back(key.clone());
        self.recent.push_back(key);
    }

    pub fn retain(&mut self, f: impl Fn(&ProjectKey) -> bool) {
        self.keys.retain(&f);
        self.recent.retain(&f);
    }

    fn make_most_recent(&mut self, key: &ProjectKey) {
        self.recent.retain(|k| k != key);
        self.recent.push_front(key.clone());
    }
}

impl Deref for Ring {
    type Target = VecDeque<ProjectKey>;
    fn deref(&self) -> &Self::Target {
        &self.keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(names: &str) -> Ring {
        let mut r = Ring::default();
        for c in names.chars() {
            r.push_back(key(c));
        }
        r
    }

    fn key(name: char) -> ProjectKey {
        ProjectKey::project(name.to_string())
    }

    fn order(ring: &Ring) -> String {
        ring.iter().map(|k| k.to_string()).collect()
    }

    fn recency(ring: &Ring) -> String {
        ring.recent().map(|k| k.to_string()).collect()
    }

    fn step(ring: &mut Ring, direction: Direction) {
        ring.step(direction, |_| true);
    }

    #[test]
    fn previous_goes_round_the_ring() {
        let mut r = ring("abcd");
        for expected in ["bcda", "cdab", "dabc", "abcd"] {
            step(&mut r, Direction::Previous);
            assert_eq!(order(&r), expected);
        }
    }

    #[test]
    fn next_goes_round_the_ring_the_other_way() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Next);
        assert_eq!(order(&r), "dabc");
        step(&mut r, Direction::Next);
        assert_eq!(order(&r), "cdab");
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "dabc");
    }

    #[test]
    fn steps_skip_ineligible_projects() {
        let mut r = ring("abc");
        r.step(Direction::Previous, |k| *k != key('b'));
        assert_eq!(order(&r), "cab");
    }

    #[test]
    fn bring_to_front_places_the_project_left_next_in_the_ring() {
        let mut r = ring("abcd");
        r.bring_to_front(&key('c'));
        assert_eq!(order(&r), "cabd");
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "abdc");
    }

    #[test]
    fn recency_lists_the_project_left_second_after_steps() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Previous);
        assert_eq!(recency(&r), "bacd");
        step(&mut r, Direction::Previous);
        assert_eq!(recency(&r), "cbad");
        r.bring_to_front(&key('a'));
        assert_eq!(recency(&r), "acbd");
    }

    #[test]
    fn switching_among_a_few_projects_keeps_them_most_recent() {
        let mut r = ring("abcdef");
        for k in ['e', 'b', 'e', 'a', 'b'] {
            r.bring_to_front(&key(k));
        }
        assert_eq!(recency(&r), "baecdf");
    }

    #[test]
    fn removed_projects_leave_both_orders() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Previous);
        r.retain(|k| *k != key('c'));
        assert_eq!(order(&r), "bda");
        assert_eq!(recency(&r), "bad");
    }
}
