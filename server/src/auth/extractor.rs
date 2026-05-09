//! `Identity` extractor: read from request extensions, where
//! [`super::middleware::session_layer`] put it (or didn't).

use std::convert::Infallible;

use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts},
    http::request::Parts,
};

use crate::{AppError, user::Permissions};

use super::token::Claims;

#[derive(Debug, Clone)]
pub struct Identity {
    pub user_id: i32,
    pub name: String,
    pub permissions: Permissions,
    /// Original claims, kept for routes that want `iat`/`exp`.
    pub claims: Claims,
}

impl Identity {
    pub fn from_claims(claims: Claims) -> Self {
        let permissions = claims.permissions();
        Self {
            user_id: claims.sub,
            name: claims.name.clone(),
            permissions,
            claims,
        }
    }
}

/// Required form: 401 if the middleware didn't put an identity in.
impl<S> FromRequestParts<S> for Identity
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Identity>()
            .cloned()
            .ok_or(AppError::Unauthorized)
    }
}

/// Optional form for handlers taking `Option<Identity>`. Axum 0.8
/// requires this to be a separate trait impl.
impl<S> OptionalFromRequestParts<S> for Identity
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(parts.extensions.get::<Identity>().cloned())
    }
}
