//! Projects in most-recently-used order: index 0 is the current project and
//! index 1 the one before it, so a picker that lists index 1 first toggles
//! between two projects with a bare Enter.
//!
//! Stepping (ctrl+cmd+left/right in the Hammerspoon switcher) walks through a
//! snapshot of the order taken when the walk began, bringing each project
//! reached to the front. The walk lasts until `release` (the Hammerspoon
//! switcher's modifiers going up) or an explicit switch; after it the project
//! reached is current and the one the walk began from is second, as with macOS
//! cmd+tab.

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};

use crate::project::ProjectKey;

#[derive(Default)]
pub struct Ring {
    keys: VecDeque<ProjectKey>,
    walk: Option<Walk>,
}

#[allow(dead_code)]
struct Walk {
    snapshot: Vec<ProjectKey>,
    index: usize,
}

#[derive(Clone, Copy)]
pub enum Direction {
    /// Toward less recently used projects.
    Previous,
    /// Toward more recently used projects, wrapping to the least recent.
    Next,
}

impl Ring {
    pub fn bring_to_front(&mut self, key: &ProjectKey) {
        unimplemented!("{key}")
    }

    /// Step to the nearest eligible project in `direction`, bring it to the
    /// front, and return it.
    pub fn step(
        &mut self,
        direction: Direction,
        is_eligible: impl Fn(&ProjectKey) -> bool,
    ) -> Option<ProjectKey> {
        let _ = (direction, is_eligible);
        unimplemented!()
    }

    pub fn release(&mut self) {
        unimplemented!()
    }
}

impl Deref for Ring {
    type Target = VecDeque<ProjectKey>;
    fn deref(&self) -> &Self::Target {
        &self.keys
    }
}

impl DerefMut for Ring {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(names: &str) -> Ring {
        Ring {
            keys: names.chars().map(key).collect(),
            walk: None,
        }
    }

    fn key(name: char) -> ProjectKey {
        ProjectKey::project(name.to_string())
    }

    fn order(ring: &Ring) -> String {
        ring.iter().map(|k| k.to_string()).collect()
    }

    fn step(ring: &mut Ring, direction: Direction) {
        ring.step(direction, |_| true);
    }

    #[test]
    fn bring_to_front_makes_the_current_project_second() {
        let mut r = ring("abcd");
        r.bring_to_front(&key('c'));
        assert_eq!(order(&r), "cabd");
    }

    #[test]
    fn switching_among_a_few_projects_keeps_them_at_the_front() {
        let mut r = ring("abcdef");
        for k in ['e', 'b', 'e', 'a', 'b'] {
            r.bring_to_front(&key(k));
        }
        assert_eq!(order(&r), "baecdf");
    }

    #[test]
    fn released_single_previous_steps_toggle() {
        let mut r = ring("abc");
        step(&mut r, Direction::Previous);
        r.release();
        assert_eq!(order(&r), "bac");
        step(&mut r, Direction::Previous);
        r.release();
        assert_eq!(order(&r), "abc");
    }

    #[test]
    fn steps_walk_the_snapshot_and_release_leaves_start_second() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "bacd");
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "cabd");
        r.release();
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "acbd");
    }

    #[test]
    fn next_wraps_to_least_recent_then_moves_toward_more_recent() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Next);
        assert_eq!(order(&r), "dabc");
        step(&mut r, Direction::Next);
        assert_eq!(order(&r), "cabd");
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
    fn bring_to_front_ends_a_walk() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Previous);
        r.bring_to_front(&key('d'));
        assert_eq!(order(&r), "dbac");
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "bdac");
    }

    #[test]
    fn walk_survives_projects_added_and_removed_meanwhile() {
        let mut r = ring("abcd");
        step(&mut r, Direction::Previous);
        r.retain(|k| *k != key('c'));
        r.push_front(key('e'));
        step(&mut r, Direction::Previous);
        assert_eq!(order(&r), "dabe");
    }
}
