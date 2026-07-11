# Simple, Data-Driven and Predictable

Use flat structures, avoid unnecessary abstraction layers, and trust SQLx and SQLite for the heavy lifting.

Splitting domain logic and data access is fine, but the database should remain the source of truth. SQLite is the primary choice for local execution and seamless testability.
The main use cases for repositories are:
- sharing query/mutation logic across multiple handlers, workers, or tests
- adding execution-time or security-related behavior that the database cannot provide (fan-out patterns, advanced validations, security policies...)


## API, Project Structure and Handlers

- **Repository layer only when it adds value**: `sqlx::query!` is for 80% of use cases, keep it simple, add a repository when in's and out's differ or the query might be shared across different features.
- **DTO separation where it makes sense**: It is perfectly fine to have a single struct with `Serialize`, `Deserialize`, `Object`, and `sqlx::FromRow` annotations if their shapes match. Separate input/output/storage structs (like `CreateUserRequest`, database row, and `UserResponse`) are only needed when their payloads or invariants differ (e.g. password hashing vs. plaintext).
- **Macros for OpenAPI responses**: Define `ApiResponse` enums using the `crate::api_response!` macro to reduce boilerplate while maintaining schema documentation.
- **Feature organization**: Group domain features inside `src/features/<feature>/`:
  - `mod.rs`: Structs/DTOs, OpenAPI route wiring, and API response definitions.
  - `<feature>_handlers.rs`: Single flat file for handlers and DB operations and tests.
- **Cross-feature infrastructure at their folders**: Cross-cutting utilities (auth, configuration, generic background worker orchestrators) belong in the crate root (e.g., `src/<module>/*.rs`), not inside feature folders.
- **Database-first**: The database schema is the source of truth for the application state. All mutations must go through sqlx, and we should avoid custom ORMs or abstraction layers that hide the underlying SQL logic.


## Testing Strategy and Code Style

- **Testing levels**:
  - **Integration tests over unit tests**: Prefer testing features end-to-end to verify that database, routes, and logic work together correctly.
  - **Unit tests for complex logic**: Keep unit tests focused on isolated, heavy logical parts (e.g., mathematical calculations, complex parser logic, state machines).
  - **Test location**:
    - Place feature-specific integration tests inside the respective feature module/file (e.g. under `#[cfg(test)]` in `<feature>_handlers.rs`).
    - Place multi-feature or global integration tests in the standard Rust `/tests` directory at the project root.
- **Clean and readable code**:
  - Keep documentation and comments minimal and clean, focusing on *why* something is done rather than *what* the code does. Follow standard Rust documentation style.
  - Use Poem's OpenAPI handler format with explicit errors and responses. Use generic types or custom responses when it improves code readability and reduces verbosity.


## Respect and Politeness
- Do not commit for yourself (as an agent) unless explicitly asked in the current message.  
- Try to keep the linter and other automated tools happy. You should not add clippy-ignores unless explicitly allowed.
- Do not modify this file unless explicitly asked.
- Do not over-engineer solutions, always try to discuss what's preferable by the user before making significant architectural decisions.
- Return information quickly and concisely, ask for opinions and clarifications. Even on minor things, it's better to be sure then to assume things and write incorrect code.
- Assume the user is in fact modifying and opiniating on the project constantly, you are not the code's owner. A lot of times the user will be already running the binary.
