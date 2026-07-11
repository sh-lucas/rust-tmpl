use crate::api_response;
use crate::middlewares::AuthClaims;
use poem::web::Data;
use poem_openapi::{Object, OpenApi, payload::Json};
use sqlx::SqlitePool;

mod users_handlers;

#[derive(Debug, Object, serde::Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Object, serde::Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Object, serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Object, serde::Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Object, serde::Serialize)]
pub struct ErrorResponse {
    pub message: String,
}

api_response! {
    pub enum RegisterResponse {
        #[oai(status = 201)]
        Created(Json<UserResponse>),
        #[oai(status = 400)]
        BadRequest(Json<ErrorResponse>),
        #[oai(status = 409)]
        Conflict(Json<ErrorResponse>),
        #[oai(status = 500)]
        InternalServerError(Json<ErrorResponse>),
    }
}

api_response! {
    pub enum LoginApiResponse {
        #[oai(status = 200)]
        Ok(Json<LoginResponse>),
        #[oai(status = 400)]
        BadRequest(Json<ErrorResponse>),
        #[oai(status = 401)]
        Unauthorized(Json<ErrorResponse>),
        #[oai(status = 500)]
        InternalServerError(Json<ErrorResponse>),
    }
}

api_response! {
    pub enum MeResponse {
        #[oai(status = 200)]
        Ok(Json<UserResponse>),
        #[oai(status = 401)]
        Unauthorized(Json<ErrorResponse>),
        #[oai(status = 500)]
        InternalServerError(Json<ErrorResponse>),
    }
}

pub struct UserApi;

#[OpenApi]
impl UserApi {
    /// Register a new user
    #[oai(path = "/users/register", method = "post")]
    async fn register(
        &self,
        pool: Data<&SqlitePool>,
        body: Json<CreateUserRequest>,
    ) -> RegisterResponse {
        users_handlers::register(pool.0, body.0).await
    }

    /// Authenticate a user and return a JWT token
    #[oai(path = "/users/login", method = "post")]
    async fn login(
        &self,
        pool: Data<&SqlitePool>,
        config: Data<&crate::config::Config>,
        body: Json<LoginRequest>,
    ) -> LoginApiResponse {
        users_handlers::login(pool.0, config.0, body.0).await
    }

    /// Retrieve the current user's profile
    #[oai(path = "/users/me", method = "get")]
    async fn me(&self, pool: Data<&SqlitePool>, auth: AuthClaims) -> MeResponse {
        users_handlers::me(pool.0, auth).await
    }
}
