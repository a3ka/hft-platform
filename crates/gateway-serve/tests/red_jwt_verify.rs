//! RED M-28 GS-I-2 (sacred, architect-only) — stateless JWT-верификация (D6/VB-I-9b).
//!
//! `auth::verify_token(token, key)` берёт ТОЛЬКО (токен, ключ) — БЕЗ user-БД. Валидный не-истёкший →
//! `Ok`; подделка/чужой ключ/истёкший/мусор → `Err`. Токены выпускает Next.js (app-плоскость), мы только
//! верифицируем подпись. Анти-плацебо: always-`Ok`-impl падает на wrong-key/expired/malformed.
//! RED сейчас: `verify_token` = `unimplemented!()` → паника (тело — engine-dev task #2).

use gateway_serve::auth::{verify_token, Claims};
use jsonwebtoken::{encode, DecodingKey, EncodingKey, Header};

fn sign(secret: &[u8], exp: usize) -> String {
    let claims = Claims {
        sub: "user-1".to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .expect("encode")
}

const FUTURE: usize = 9_999_999_999; // ~год 2286 → не истёк

#[test]
fn valid_token_ok() {
    let secret = b"test-secret";
    let token = sign(secret, FUTURE);
    let claims = verify_token(&token, &DecodingKey::from_secret(secret)).expect("валидный → Ok");
    assert_eq!(claims.sub, "user-1");
}

#[test]
fn wrong_key_err() {
    let token = sign(b"test-secret", FUTURE);
    assert!(
        verify_token(&token, &DecodingKey::from_secret(b"other-secret")).is_err(),
        "чужой ключ (подделка подписи) → Err"
    );
}

#[test]
fn expired_token_err() {
    let secret = b"test-secret";
    let token = sign(secret, 1); // exp = 1s epoch → давно истёк
    assert!(
        verify_token(&token, &DecodingKey::from_secret(secret)).is_err(),
        "истёкший (exp в прошлом) → Err"
    );
}

#[test]
fn malformed_token_err() {
    assert!(
        verify_token("not.a.jwt", &DecodingKey::from_secret(b"test-secret")).is_err(),
        "мусор вместо JWT → Err (не паника)"
    );
}
