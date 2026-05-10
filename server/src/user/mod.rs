//! User-shaped concerns: permissions bitfield, source enum,
//! anything else we accumulate as the auth story grows. The HTTP
//! routes and DB writers live elsewhere; this module owns the *types*
//! that describe a user.

pub mod dto;
pub mod permissions;
pub mod preferences;
pub mod profile;
pub mod sex;
pub mod source;

pub use dto::{MeDto, UserDto, UserProfileDto, fetch_me, fetch_user};
pub use permissions::Permissions;
pub use preferences::{
    PreferencesDto, UpdatePreferencesRequest, apply_preferences_update, fetch_preferences,
    validate_preferences_update,
};
pub use profile::{UpdateProfileRequest, apply_profile_update, validate_profile_update};
pub use sex::UserSex;
pub use source::UserSource;
