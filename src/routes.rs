use poem::endpoint::StaticFilesEndpoint;
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
    #[oai(path = "/healthz", method = "get")]
    #[allow(clippy::unused_async)]
    async fn healthz(&self) -> HealthResponse {
        HealthResponse::Ok(Json(Healthz {
            message: "server online".to_string(),
        }))
    }
}

/// Directory holding the rendered mdBook served at `/docs`.
fn book_root() -> String {
    std::env::var("BOOK_ROOT").unwrap_or_else(|_| "book".to_string())
}

pub fn with_routes(app: Route) -> Route {
    with_routes_using_book_root(app, book_root())
}

pub fn with_routes_using_book_root(app: Route, root: String) -> Route {
    let api_service = OpenApiService::new((users::UserApi, SystemApi), "App Rest API", "1.0");

    let swagger_ui = api_service.swagger_ui();
    let redoc_ui = api_service.redoc();
    let book = StaticFilesEndpoint::new(root)
        .index_file("index.html")
        .redirect_to_slash_directory();

    app.nest("/", api_service.with(Cors::new()))
        .nest("/docs", book)
        .nest("/swagger", swagger_ui)
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
        let resp = cli.get("/healthz").send().await;

        resp.assert_status(poem::http::StatusCode::OK);
        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(body.contains(r#""message":"server online""#));
    }

    #[tokio::test]
    async fn docs_serves_the_rendered_book() {
        let root = std::env::temp_dir().join(format!("rust-tmpl-book-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("index.html"), "<h1>Rust Template</h1>").unwrap();

        let cli = TestClient::new(with_routes_using_book_root(
            Route::new(),
            root.to_string_lossy().into_owned(),
        ));
        let resp = cli.get("/docs/index.html").send().await;

        resp.assert_status(poem::http::StatusCode::OK);
        let body = resp.0.into_body().into_string().await.unwrap();
        assert!(body.contains("Rust Template"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
