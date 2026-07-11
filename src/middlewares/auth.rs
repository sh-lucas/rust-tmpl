use crate::auth::{Claims, parse_auth_token};
use poem::http;
use poem::{FromRequest, Request, RequestBody, Result};
use secrecy::ExposeSecret;

/// Extracts and validates a Bearer token from the `Authorization` header.
pub struct AuthClaims(pub Claims);

impl<'a> FromRequest<'a> for AuthClaims {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let config = req.data::<crate::config::Config>().ok_or_else(|| {
            poem::Error::from_string(
                "Config not found in request data",
                http::StatusCode::INTERNAL_SERVER_ERROR,
            )
        })?;

        let header = req.header("Authorization").ok_or_else(|| {
            poem::Error::from_string(
                "Missing Authorization header",
                http::StatusCode::UNAUTHORIZED,
            )
        })?;

        let token = header.strip_prefix("Bearer ").ok_or_else(|| {
            poem::Error::from_string(
                "Authorization header must use the Bearer scheme",
                http::StatusCode::UNAUTHORIZED,
            )
        })?;

        Ok(AuthClaims(parse_auth_token(
            token,
            config.jwt_secret.expose_secret(),
        )?))
    }
}
