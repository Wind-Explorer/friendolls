use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sqlx::sqlite::SqlitePoolOptions;

use super::{KeypairError, URL_SAFE_NO_PAD, prepare};
use crate::db::AppDatabase;

async fn database() -> AppDatabase {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect to in-memory SQLite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run database migrations");
    AppDatabase::new(pool)
}

#[tokio::test]
async fn prepare_persists_one_stable_plaintext_keypair() {
    let database = database().await;

    let first_keypair = prepare(&database).await.expect("prepare keypair");
    let second_keypair = prepare(&database).await.expect("load keypair");
    let (row_count, stored_public_key, stored_secret_key): (i64, String, String) =
        sqlx::query_as("SELECT COUNT(*), public_key, secret_key FROM keypair")
            .fetch_one(database.pool())
            .await
            .expect("read stored keypair");

    assert_eq!(first_keypair.public_key(), second_keypair.public_key());
    assert_eq!(row_count, 1);
    assert_eq!(stored_public_key, first_keypair.public_key());
    assert_eq!(URL_SAFE_NO_PAD.decode(stored_public_key).unwrap().len(), 32);
    assert_eq!(URL_SAFE_NO_PAD.decode(stored_secret_key).unwrap().len(), 32);
}

#[tokio::test]
async fn sign_produces_a_signature_verifiable_by_the_entity_id() {
    let database = database().await;
    let payload = b"payload sent to a friend";

    let keypair = prepare(&database).await.expect("prepare keypair");
    let signature = keypair.sign(payload);

    let public_key: [u8; 32] = URL_SAFE_NO_PAD
        .decode(keypair.public_key())
        .unwrap()
        .try_into()
        .unwrap();
    let signature = Signature::from_slice(&URL_SAFE_NO_PAD.decode(signature).unwrap()).unwrap();
    let verifying_key = VerifyingKey::from_bytes(&public_key).unwrap();

    assert!(verifying_key.verify(payload, &signature).is_ok());
    assert!(
        verifying_key
            .verify(b"a different payload", &signature)
            .is_err()
    );
}

#[tokio::test]
async fn prepare_rejects_a_public_key_that_does_not_match_the_secret() {
    let database = database().await;
    prepare(&database).await.expect("prepare keypair");
    let different_public_key = URL_SAFE_NO_PAD.encode([0_u8; 32]);
    sqlx::query("UPDATE keypair SET public_key = ?1 WHERE id = 1")
        .bind(different_public_key)
        .execute(database.pool())
        .await
        .expect("corrupt stored public key");

    let error = match prepare(&database).await {
        Ok(_) => panic!("accepted mismatched public and secret keys"),
        Err(error) => error,
    };

    assert!(matches!(error, KeypairError::PublicKeyMismatch));
}
