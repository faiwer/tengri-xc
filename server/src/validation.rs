//! Per-field validation errors. The shape that lands on the wire is
//! deliberately flat (one map keyed by dotted paths) so the FE can
//! drive AntD `Form` directly: `setFields([{ name: ['profile',
//! 'country'], errors: ['…'] }])`.
//!
//! Endpoints that compose multiple editable sections (e.g. `PATCH
//! /users/me` updating both `profile` and `preferences`) accumulate
//! into a shared [`FieldErrors`] and use [`FieldErrors::merge_prefixed`]
//! so each section's validator stays unaware of the surrounding
//! envelope. A standalone admin endpoint can call the same section
//! validator with no prefix and get bare field names.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::AppError;

/// Bag of `field-path → message` pairs. Order doesn't matter on the
/// wire (the FE keys into the map by field name), but `BTreeMap`
/// gives stable iteration so test snapshots stay deterministic.
#[derive(Debug, Default, Serialize)]
pub struct FieldErrors {
    pub fields: BTreeMap<String, String>,
}

impl FieldErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Record an error against `field`. Last write wins per field —
    /// validators shouldn't be calling twice for the same key, but if
    /// they do the later message is the more specific one.
    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.fields.insert(field.into(), message.into());
    }

    /// Move `other`'s entries into `self`, prefixing each key with
    /// `prefix.`. Used when a section validator returns bare field
    /// names (`country`, `civl_id`) and the composing endpoint wants
    /// them under a section namespace (`profile.country`).
    pub fn merge_prefixed(&mut self, prefix: &str, other: FieldErrors) {
        for (key, message) in other.fields {
            self.fields.insert(format!("{prefix}.{key}"), message);
        }
    }

    /// `Ok(())` when empty, otherwise [`AppError::Validation`]. The
    /// usual call site is `errors.into_result()?` at the end of a
    /// composed endpoint's validation pass.
    pub fn into_result(self) -> Result<(), AppError> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(self))
        }
    }
}
