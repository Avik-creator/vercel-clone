use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use rand::Rng;
use uuid::Uuid;
use crate::{
    AppState,
    errors::AppResult,
    models::{ApiKey, CreateApiKeyRequest},
};

pub async fn list(state: &AppState, user_id: Uuid) -> AppResult<Vec<ApiKey>> {
    let keys = sqlx::query_as::<_, ApiKey>(
        "SELECT * FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC"
    )
    .bind(user_id)
    .fetch_all(&*state.db)
    .await?;
    Ok(keys)
}
pub async fn create(
    state: &AppState,
    user_id: Uuid,
    req: CreateApiKeyRequest,
) -> AppResult<ApiKey> {
    // Generate key: cp_<48 random hex chars>
    let raw: String = (0..48)
        .map(|_| format!("{:x}", rand::thread_rng().gen_range(0..256)))
        .collect();
    let plain = format!("cp_{raw}");

    let prefix = &plain[..20]; // prefix stored for lookup efficiency

    let salt = SaltString::generate(&mut OsRng);
    let key_hash = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| crate::errors::AppError::Internal(anyhow::anyhow!("hash error: {e}")))?;

    let expires_at = req.expires_in_days.map(|d| chrono::Utc::now() + chrono::Duration::days(d));
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let mut key = sqlx::query_as::<_, ApiKey>(
        r#"
        INSERT INTO api_keys (id, user_id, name, key_hash, key_prefix, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#
    )
    .bind(id)
    .bind(user_id)
    .bind(&req.name)
    .bind(&key_hash)
    .bind(prefix)
    .bind(expires_at)
    .bind(now)
    .fetch_one(&*state.db)
    .await?;

    // Only time the plain key is returned
    key.key_plain = Some(plain);
    Ok(key)
}

pub async fn revoke(state: &AppState, user_id: Uuid, id: Uuid) -> AppResult<()> {
    sqlx::query!(
        "DELETE FROM api_keys WHERE id = $1 AND user_id = $2",
        id, user_id
    )
    .execute(&*state.db)
    .await?;
    Ok(())
}
