//! Differ golden tests. The historical `common` / `patch_tests` /
//! `reattach_tests` modules were never committed (audit H13), which broke the
//! crate's build. Rebuilt in Phase 2 (T-212, T-213) as inline `#[cfg(test)]`
//! modules next to the code they cover.
