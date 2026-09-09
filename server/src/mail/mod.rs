//! Outgoing mail. Owns the SMTP transport, the allowlist that bounds what
//! markup an operator-authored template may contain, and the composition of
//! each message the server sends.

mod compose;
mod confirm;
mod reset;
mod sanitize;
mod send;
mod test_email;

pub use confirm::{ConfirmRecipient, send_confirmation_email};
pub use reset::{ResetRecipient, send_password_reset_email};
pub use sanitize::sanitize_email_html;
pub use send::is_smtp_configured;
pub use test_email::{TestEmailRequest, send_test_email};
