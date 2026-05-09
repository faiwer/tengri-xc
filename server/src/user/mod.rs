//! User-shaped concerns: permissions bitfield, source enum,
//! anything else we accumulate as the auth story grows. The HTTP
//! routes and DB writers live elsewhere; this module owns the *types*
//! that describe a user.

pub mod permissions;
pub mod sex;
pub mod source;

pub use permissions::Permissions;
pub use sex::UserSex;
pub use source::UserSource;
