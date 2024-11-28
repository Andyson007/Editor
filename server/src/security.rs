use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use sqlx::SqlitePool;
use std::str;
use tokio_tungstenite::tungstenite::http::HeaderValue;

pub(crate) async fn auth_check(value: &HeaderValue, pool: &SqlitePool) -> Option<String> {
    let (credential_type, credentials) = value.to_str().unwrap().split_once(' ')?;
    if credential_type != "Basic" {
        return None;
    }
    let base64 = BASE64_STANDARD.decode(credentials).ok()?;
    let raw = str::from_utf8(base64.as_slice()).ok()?;
    let (username, password) = raw.split_once(':')?;
    let phc: (String,) = sqlx::query_as("SELECT phc FROM users WHERE username=$1")
        .bind(username)
        .fetch_optional(pool)
        .await
        .unwrap()?;

    Argon2::default()
        .verify_password(password.as_bytes(), &PasswordHash::new(&phc.0).unwrap())
        .ok()?;

    Some(username.to_string())
}

pub(crate) async fn create_tables(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY NOT NULL AUTOINCREMENT,
            username TEXT NOT NULL,
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
