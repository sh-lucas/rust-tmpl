use bcrypt::{DEFAULT_COST, hash, verify};
use poem_openapi::payload::Json;
use secrecy::ExposeSecret;
use sqlx::SqlitePool;

use super::{
    CreateUserRequest, ErrorResponse, LoginApiResponse, LoginRequest, LoginResponse, MeResponse,
    RegisterResponse, UserResponse,
};
use crate::auth::{TokenType, gen_auth_token};
use crate::helpers::is_unique_err;
use crate::middlewares::AuthClaims;

#[tracing::instrument(skip(pool, req))]
pub async fn register(pool: &SqlitePool, req: CreateUserRequest) -> RegisterResponse {
    if req.email.trim().is_empty() || req.password.len() < 6 {
        return RegisterResponse::BadRequest(Json(ErrorResponse {
            message: "Invalid email or password (min 6 characters)".to_string(),
        }));
    }

    let Ok(password_hash) = hash(&req.password, DEFAULT_COST) else {
        return RegisterResponse::InternalServerError(Json(ErrorResponse {
            message: "Failed to hash password".to_string(),
        }));
    };

    let result = sqlx::query!(
        "INSERT INTO users (email, password_hash) VALUES (?, ?) RETURNING id, email, created_at",
        req.email,
        password_hash
    )
    .fetch_one(pool)
    .await;

    match result {
        Ok(row) => RegisterResponse::Created(Json(UserResponse {
            id: row.id,
            email: row.email,
            created_at: row.created_at,
        })),
        Err(e) => {
            if is_unique_err(&e) {
                RegisterResponse::Conflict(Json(ErrorResponse {
                    message: "Email is already registered".to_string(),
                }))
            } else {
                RegisterResponse::InternalServerError(Json(ErrorResponse {
                    message: format!("Database error: {e}"),
                }))
            }
        }
    }
}

#[tracing::instrument(skip(pool, config, req))]
pub async fn login(
    pool: &SqlitePool,
    config: &crate::config::Config,
    req: LoginRequest,
) -> LoginApiResponse {
    let row = sqlx::query!(
        "SELECT id, email, password_hash, created_at FROM users WHERE email = ?",
        req.email
    )
    .fetch_optional(pool)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            return LoginApiResponse::Unauthorized(Json(ErrorResponse {
                message: "Invalid email or password".to_string(),
            }));
        }
        Err(e) => {
            return LoginApiResponse::InternalServerError(Json(ErrorResponse {
                message: format!("Database error: {e}"),
            }));
        }
    };

    match verify(&req.password, &row.password_hash) {
        Ok(true) => {
            let token = gen_auth_token(
                row.id,
                TokenType::Access,
                24,
                config.jwt_secret.expose_secret(),
            );
            LoginApiResponse::Ok(Json(LoginResponse {
                token,
                user: UserResponse {
                    id: row.id,
                    email: row.email,
                    created_at: row.created_at,
                },
            }))
        }
        _ => LoginApiResponse::Unauthorized(Json(ErrorResponse {
            message: "Invalid email or password".to_string(),
        })),
    }
}

#[tracing::instrument(skip(pool, auth))]
pub async fn me(pool: &SqlitePool, auth: AuthClaims) -> MeResponse {
    let user_id = auth.0.sub;

    let row = sqlx::query!(
        "SELECT id, email, created_at FROM users WHERE id = ?",
        user_id
    )
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(r)) => MeResponse::Ok(Json(UserResponse {
            id: r.id,
            email: r.email,
            created_at: r.created_at,
        })),
        Ok(None) => MeResponse::Unauthorized(Json(ErrorResponse {
            message: "User not found".to_string(),
        })),
        Err(e) => MeResponse::InternalServerError(Json(ErrorResponse {
            message: format!("Database error: {e}"),
        })),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use poem::{EndpointExt, Route, test::TestClient};
    use serde_json::json;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn test_user_flow() {
        // Spin up in-memory sqlite for test isolation
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap();

        let config = crate::config::Config {
            port: 3000,
            database_url: secrecy::SecretString::from("sqlite::memory:".to_string()),
            jwt_secret: secrecy::SecretString::from("test-secret-key-1234567890".to_string()),
            observability: crate::config::ObservabilityConfig {
                service_name: "rust-tmpl-test".to_string(),
                service_version: "test".to_string(),
                deployment_environment: "test".to_string(),
                otlp_endpoint: None,
                slow_query_threshold: std::time::Duration::from_millis(250),
            },
        };

        // Build app
        let app = crate::routes::with_routes(Route::new())
            .with(poem::middleware::AddData::new(pool))
            .with(poem::middleware::AddData::new(config));

        let cli = TestClient::new(app);

        // 1. Register a new user
        let resp = cli
            .post("/users/register")
            .body_json(&json!({
                "email": "test@example.com",
                "password": "password123"
            }))
            .send()
            .await;

        resp.assert_status(poem::http::StatusCode::CREATED);
        let body = resp
            .0
            .into_body()
            .into_json::<serde_json::Value>()
            .await
            .unwrap();
        assert_eq!(body["email"], "test@example.com");
        assert!(body["id"].is_number());

        // 2. Register duplicate user -> Expect Conflict (409)
        let resp = cli
            .post("/users/register")
            .body_json(&json!({
                "email": "test@example.com",
                "password": "password123"
            }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::CONFLICT);

        // 3. Login
        let resp = cli
            .post("/users/login")
            .body_json(&json!({
                "email": "test@example.com",
                "password": "password123"
            }))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::OK);
        let body = resp
            .0
            .into_body()
            .into_json::<serde_json::Value>()
            .await
            .unwrap();
        let token = body["token"].as_str().unwrap();
        assert_eq!(body["user"]["email"], "test@example.com");

        // 4. Retrieve Profile via /users/me
        let resp = cli
            .get("/users/me")
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::OK);
        let body = resp
            .0
            .into_body()
            .into_json::<serde_json::Value>()
            .await
            .unwrap();
        assert_eq!(body["email"], "test@example.com");
    }
}
