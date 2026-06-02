//! User-shaped concerns: account creation, permissions bitfield,
//! source enum, profile/preferences, and DTOs shared by HTTP routes
//! and CLI tooling.

pub mod create;
pub mod dto;
pub mod permissions;
pub mod preferences;
pub mod profile;
pub mod sex;
pub mod source;

pub use create::{CreateUser, CreateUserPassword, CreatedUser, create_user, create_user_if_absent};
pub use dto::{MeDto, UserDto, UserProfileDto, fetch_me, fetch_user};
pub use permissions::Permissions;
pub use preferences::{
    PreferencesDto, UpdatePreferencesRequest, apply_preferences_update, fetch_preferences,
    validate_preferences_update,
};
pub use profile::{UpdateProfileRequest, apply_profile_update, validate_profile_update};
pub use sex::UserSex;
pub use source::UserSource;
