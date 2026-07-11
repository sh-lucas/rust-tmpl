/// `is_unique_err` checks if the error is a unique constraint violation
pub fn is_unique_err(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .as_deref()
        == Some("2067")
}

/// Helper macro that wraps `#[derive(poem_openapi::ApiResponse)]` to reduce boilerplate.
/// Note: This is equivalent to declaring a standard Poem `OpenAPI` response enum with
/// `#[derive(poem_openapi::ApiResponse)]` directly.
#[macro_export]
macro_rules! api_response {
    (
        $(#[$outer:meta])*
        pub enum $name:ident {
            $(
                #[oai(status = $status:literal)]
                $variant:ident $( ( $payload:ty ) )?,
            )*
        }
    ) => {
        #[derive(poem_openapi::ApiResponse)]
        $(#[$outer])*
        pub enum $name {
            $(
                #[oai(status = $status)]
                $variant $( ( $payload ) )?,
            )*
        }
    };
}
