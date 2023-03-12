use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_macros::debug_handler;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use dotenv::dotenv;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;
use std::{error::Error, net::SocketAddr, sync::Arc, time::Duration};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

mod repo;

struct AppSettings {
    max_content_length: usize,
    database_url: String,
}

struct AppState {
    db: PgPool,
    settings: AppSettings,
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let settings = AppSettings {
        max_content_length: env::var("MAX_CONTENT_LENGTH")
            .unwrap_or("32768".into())
            .parse()
            .expect("Unable to parse MAX_CONTENT_LENGTH setting"),
        database_url: env::var("DATABASE_URL")
            .unwrap_or("postgres://postgres:secret@localhost/pastecord".into()),
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .test_before_acquire(true)
        .connect(&settings.database_url)
        .await
        .expect("Unable to connect to postgres");
    tracing::info!("Connected to database");

    let state = AppState { db: pool, settings };

    // build our application with a route
    let app = Router::new()
        .route("/documents", post(documents_post))
        .route("/documents/:id", get(documents_get))
        .route("/raw/:id", get(documents_get_raw))
        .nest_service(
            "/",
            ServeDir::new("static").fallback(ServeFile::new("static/index.html")),
        )
        .with_state(Arc::new(state))
        .layer(TraceLayer::new_for_http());

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("pastecord backend listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Serialize)]
struct Created {
    key: String,
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
struct Paste {
    id: Uuid,
    content: String,
    created_at: NaiveDateTime,
}

#[debug_handler]
async fn documents_post(State(state): State<Arc<AppState>>, body: String) -> impl IntoResponse {
    // Validate body length
    if body.len() > state.settings.max_content_length || body.len() < 1 {
        return (
            StatusCode::BAD_REQUEST,
            "Content length must be between 1 and {state.settings.max_content_length}",
        )
            .into_response();
    }
    match repo::paste::add_paste(&state.db, body).await {
        Ok(created) => Json(Created {
            key: created.to_string(),
        })
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Internal server error: {}", e),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
struct FoundPaste {
    key: Uuid,
    data: String,
}

async fn documents_get(
    Path(id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let item = repo::paste::get_paste(&state.db, id).await;

    match item {
        Ok(found) => Json(FoundPaste {
            data: found.content,
            key: found.id,
        })
        .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Paste not found").into_response(),
    }
}

async fn documents_get_raw(
    Path(id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let item = repo::paste::get_paste(&state.db, id).await;

    match item {
        Ok(found) => found.content.into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Paste not found").into_response(),
    }
}
