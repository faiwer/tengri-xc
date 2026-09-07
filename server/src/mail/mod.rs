//! Outgoing mail. Owns the SMTP transport, the allowlist that bounds what
//! markup an operator-authored template may contain, and the test-send route's
//! request handling.

mod sanitize;
mod send;
mod test_email;

pub use sanitize::sanitize_email_html;
pub use test_email::{TestEmailRequest, send_test_email};
