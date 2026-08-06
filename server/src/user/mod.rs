//! User-shaped concerns: account creation, permissions bitfield,
//! source enum, profile/preferences, and DTOs shared by HTTP routes
//! and CLI tooling.

pub mod account;
pub mod create;
pub mod dto;
pub mod permissions;
pub mod preferences;
pub mod profile;
pub mod sex;
pub mod source;

pub use account::{
    AccountUpdate, apply_account_update, blank_to_none, check_unique,
    should_clear_email_verification, validate_email, validate_name,
};
pub use create::{CreateUser, CreateUserPassword, CreatedUser, create_user, create_user_if_absent};
pub use dto::{MeDto, UserDto, UserProfileDto, fetch_me, fetch_user};
pub use permissions::Permissions;
pub use preferences::{
    PreferencesDto, UpdatePreferencesRequest, apply_preferences_update, fetch_preferences,
    validate_preferences_update,
};
pub use profile::{
    ProfileUpdate, UpdateProfileRequest, apply_profile_update, validate_profile_update,
};
pub use sex::UserSex;
pub use source::UserSource;
