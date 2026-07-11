use poem::{EndpointExt, Route, middleware::Cors};
use poem_openapi::{Object, OpenApi, OpenApiService, payload::Json};

use crate::api_response;
use crate::features::users;

#[derive(Debug, serde::Serialize, Object)]
pub struct Healthz {
    pub message: String,
}

api_response! {
    pub enum HealthResponse {
        #[oai(status = 200)]
        Ok(Json<Healthz>),
    }
}

pub struct SystemApi;

#[OpenApi]
impl SystemApi {
    /// Server health check
    #[oai(path = "/", method = "get")]
    #[allow(clippy::unused_async)]
    async fn healthz(&self) -> HealthResponse {
        HealthResponse::Ok(Json(Healthz {
            message: "server online".to_string(),
        }))
    }
}

pub fn with_routes(app: Route) -> Route {
    let api_service = OpenApiService::new((users::UserApi, SystemApi), "App Rest API", "1.0");

    let swagger_ui = api_service.swagger_ui();
    let redoc_ui = api_service.redoc();

    app.nest("/", api_service.with(Cors::new()))
        .nest("/docs", swagger_ui)
        .nest("/redoc", redoc_ui)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use poem::test::TestClient;

    #[tokio::test]
    async fn test_healthz_route() {
        let app = with_routes(Route::new());
        let cli = TestClient::new(app);
        let resp = cli.get("/").send().await;

        resp.assert_status(poem::http::StatusCode::OK);
        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(body.contains(r#""message":"server online""#));
    }
}
