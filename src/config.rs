use secrecy::SecretString;
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: SecretString,
    pub jwt_secret: SecretString,
}

impl Config {
    pub fn from_env() -> Self {
        let port: u16 = env::var("PORT")
            .expect("PORT not set in environment variables")
            .parse()
            .expect("PORT must be a valid u16");

        let database_url = SecretString::from(
            env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment variables"),
        );

        let jwt_secret = SecretString::from(
            env::var("JWT_SECRET").expect("JWT_SECRET must be set in environment variables"),
        );

        Self {
            port,
            database_url,
            jwt_secret,
        }
    }
}
