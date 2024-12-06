use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use sqlx::SqlitePool;
use std::str;
use tokio_tungstenite::tungstenite::http::HeaderValue;

pub(crate) async fn auth_check(
    username: &str,
    password: &str,
    pool: &SqlitePool,
) -> Option<()> {
    let phc: (String,) = sqlx::query_as("SELECT phc FROM users WHERE username=$1")
        .bind(username)
        .fetch_optional(pool)
        .await
        .unwrap()?;

    Argon2::default()
        .verify_password(password.as_bytes(), &PasswordHash::new(&phc.0).unwrap())
        .ok()?;

    Some(())
}

pub(crate) async fn create_tables(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY NOT NULL,
            username TEXT NOT NULL UNIQUE,
            phc TEXT NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_user(pool: &SqlitePool, username: &str, password: &str) {
    create_tables(pool).await;
    let phc = Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .unwrap()
        .to_string();
    println!("{phc}");
    sqlx::query("INSERT INTO users (username, phc) VALUES ($1, $2)")
        .bind(username)
        .bind(phc)
        .execute(pool)
        .await
        .unwrap();
}
