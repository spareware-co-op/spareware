//! A stand-in for Google's sign-in, for local testing only. It speaks real OpenID Connect, with
//! signed ID tokens and PKCE, so the server's one sign-in path is exercised without a Google
//! account and without any test back door in the server. It lets anyone be anyone: never run it
//! anywhere but your own machine.

use axum::extract::{Form, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine;
use chrono::{Duration, Utc};
use openidconnect::core::{
    CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
    CoreRsaPrivateSigningKey,
};
use openidconnect::{
    Audience, EmptyAdditionalClaims, EndUserEmail, IssuerUrl, JsonWebKeyId, Nonce,
    PrivateSigningKey, StandardClaims, SubjectIdentifier,
};
use rsa::RsaPrivateKey;
use rsa::pkcs1::{EncodeRsaPrivateKey, LineEnding};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

const ISSUER: &str = "http://localhost:9000";
const PEOPLE: [&str; 5] = ["anna", "ben", "cara", "dan", "eve"];

struct Provider {
    key: CoreRsaPrivateSigningKey,
    codes: Mutex<HashMap<String, Code>>,
    used: Mutex<HashSet<String>>,
}

struct Code {
    email: String,
    client: String,
    nonce: String,
    challenge: String,
}

#[derive(Deserialize)]
struct Authorize {
    client_id: String,
    redirect_uri: String,
    state: String,
    nonce: String,
    code_challenge: String,
    login_hint: Option<String>,
}

/// Signs someone in at once if the login hint says who; otherwise asks.
async fn authorize(
    State(provider): State<Arc<Provider>>,
    Query(query): Query<Authorize>,
) -> impl IntoResponse {
    let Some(email) = query.login_hint.clone() else {
        let links: String = PEOPLE
            .iter()
            .map(|person| {
                let mut url = url::Url::parse(&format!("{ISSUER}/authorize")).expect("constant");
                url.query_pairs_mut()
                    .append_pair("client_id", &query.client_id)
                    .append_pair("redirect_uri", &query.redirect_uri)
                    .append_pair("state", &query.state)
                    .append_pair("nonce", &query.nonce)
                    .append_pair("code_challenge", &query.code_challenge)
                    .append_pair("login_hint", &format!("{person}@example.test"));
                format!("<p><a href=\"{url}\">{person}</a></p>")
            })
            .collect();
        return Html(format!("<h1>Local test sign-in</h1>{links}")).into_response();
    };
    let code = format!("code-{}", query.nonce);
    let row = Code {
        email,
        client: query.client_id,
        nonce: query.nonce,
        challenge: query.code_challenge,
    };
    provider
        .codes
        .lock()
        .expect("lock")
        .insert(code.clone(), row);
    let mut back = url::Url::parse(&query.redirect_uri).expect("redirect address");
    back.query_pairs_mut()
        .append_pair("code", &code)
        .append_pair("state", &query.state);
    Redirect::to(back.as_str()).into_response()
}

#[derive(Deserialize)]
struct TokenRequest {
    code: String,
    code_verifier: String,
}

async fn token(
    State(provider): State<Arc<Provider>>,
    Form(form): Form<TokenRequest>,
) -> impl IntoResponse {
    let refused = (
        axum::http::StatusCode::BAD_REQUEST,
        Json(json!({ "error": "invalid_grant" })),
    );
    if !provider
        .used
        .lock()
        .expect("lock")
        .insert(form.code.clone())
    {
        return refused.into_response();
    }
    let Some(code) = provider.codes.lock().expect("lock").remove(&form.code) else {
        return refused.into_response();
    };
    let hash = Sha256::digest(form.code_verifier.as_bytes());
    if base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash) != code.challenge {
        return refused.into_response();
    }
    let email = code.email.clone();
    let claims = CoreIdTokenClaims::new(
        IssuerUrl::new(ISSUER.to_owned()).expect("constant"),
        vec![Audience::new(code.client)],
        Utc::now() + Duration::minutes(5),
        Utc::now(),
        StandardClaims::new(SubjectIdentifier::new(format!("subject-of-{email}")))
            .set_email(Some(EndUserEmail::new(email)))
            .set_email_verified(Some(true)),
        EmptyAdditionalClaims {},
    )
    .set_nonce(Some(Nonce::new(code.nonce)));
    let algorithm = CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256;
    let id_token = CoreIdToken::new(claims, &provider.key, algorithm, None, None).expect("signing");
    Json(json!({
        "access_token": "local-test",
        "token_type": "Bearer",
        "expires_in": 300,
        "id_token": id_token,
    }))
    .into_response()
}

async fn discovery() -> Json<Value> {
    Json(json!({
        "issuer": ISSUER,
        "authorization_endpoint": format!("{ISSUER}/authorize"),
        "token_endpoint": format!("{ISSUER}/token"),
        "jwks_uri": format!("{ISSUER}/keys"),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
    }))
}

async fn keys(State(provider): State<Arc<Provider>>) -> Json<CoreJsonWebKeySet> {
    Json(CoreJsonWebKeySet::new(vec![
        provider.key.as_verification_key(),
    ]))
}

#[tokio::main]
async fn main() {
    let private = RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).expect("key");
    let pem = private.to_pkcs1_pem(LineEnding::LF).expect("key");
    let id = JsonWebKeyId::new("local".to_owned());
    let key = CoreRsaPrivateSigningKey::from_pem(&pem, Some(id)).expect("key");
    let provider = Arc::new(Provider {
        key,
        codes: Mutex::default(),
        used: Mutex::default(),
    });
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/keys", get(keys))
        .route("/authorize", get(authorize))
        .route("/token", post(token))
        .with_state(provider);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9000")
        .await
        .expect("port 9000");
    println!("local sign-in on {ISSUER}");
    axum::serve(listener, app).await.expect("serving");
}
