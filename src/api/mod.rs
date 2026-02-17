pub mod auth;
pub mod middleware;
pub mod agents;
pub mod telegram;

use axum::{Router, Extension};
use sqlx::PgPool;

pub fn router(pool: PgPool) -> Router {
    let auth_routes = auth::router();
    let agent_routes = agents::router()
        .layer(axum::middleware::from_fn(middleware::auth_middleware));
    let telegram_routes = telegram::router();

    Router::new()
        .nest("/auth", auth_routes)
        .nest("/agents", agent_routes)
        .nest("/telegram", telegram_routes)
        .layer(Extension(pool))
}
