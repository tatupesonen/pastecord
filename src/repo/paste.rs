use std::error::Error;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn add_paste(pool: &PgPool, content: String) -> Result<Uuid, Box<dyn Error>> {
    let rec = sqlx::query!(
        r#"
INSERT INTO pastes (id, content)
VALUES ( $1, $2 )
RETURNING id
        "#,
        Uuid::new_v4(),
        content,
    )
    .fetch_one(pool)
    .await?;

    Ok(rec.id)
}

#[derive(sqlx::FromRow, Deserialize, Serialize)]
pub struct Paste {
    pub id: Uuid,
    pub content: String,
    pub created_at: NaiveDateTime,
		pub user_id: Option<i32>,
}

pub async fn get_paste(pool: &PgPool, id: Uuid) -> Result<Paste, Box<dyn Error>> {
    let paste = sqlx::query_as!(Paste, "SELECT * FROM pastes WHERE id = $1", id)
        .fetch_one(pool).await?;

		Ok(paste)
}
