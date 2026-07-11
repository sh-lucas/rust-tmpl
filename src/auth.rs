use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use poem::http;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TokenType {
    Access,
    Refresh,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub exp: chrono::DateTime<chrono::Utc>,
    pub aud: TokenType,
}

pub fn gen_auth_token(user_id: i64, token_type: TokenType, exp_hours: u64, secret: &str) -> String {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(
            i64::try_from(exp_hours).expect("exp_hours overflow"),
        ))
        .expect("Invalid expiration time");

    let claims = Claims {
        sub: user_id,
        exp: expiration,
        aud: token_type,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("Could not generate token")
}

pub fn parse_auth_token(token: &str, secret: &str) -> Result<Claims, poem::Error> {
    let mut validation = Validation::default();
    validation.validate_aud = false;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| poem::Error::from_string(e.to_string(), http::StatusCode::UNAUTHORIZED))?;

    // `aud` is already required by the Claims type, but we double-check that
    // it deserialized to one of the known variants (defense in depth).
    match data.claims.aud {
        TokenType::Access | TokenType::Refresh => {}
    }

    Ok(data.claims)
}
