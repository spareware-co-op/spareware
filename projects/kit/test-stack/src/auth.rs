//! Sign-in with OpenID Connect. Locally this talks to a stand-in provider (examples/mock_oidc.rs),
//! on the server to Google; the only difference is OIDC_ISSUER, so there is no test back door in
//! this program. Sessions live in TiKV, so restarting a server signs nobody out.

use crate::api::{ApiError, Shared};
use crate::keys;
use crate::orderer::now_ms;
use crate::rows::{Person, sha256_hex};
use anyhow::{Context as _, Result, anyhow, bail};
use axum::extract::{FromRequestParts, Query, State};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use base64::Engine;
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreIdTokenClaims, CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

const COOKIE: &str = "spare_session";
const SESSION_MS: u64 = 14 * 24 * 3600 * 1000;
const PENDING_MS: u64 = 10 * 60 * 1000;

type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

/// The sign-in provider, found the first time someone signs in, so the server still starts
/// before a Google client exists.
#[derive(Default)]
pub struct Oidc {
    client: OnceCell<Client>,
}

#[derive(Serialize, Deserialize)]
struct Pending {
    nonce: String,
    verifier: String,
    until_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct Session {
    person: String,
    until_ms: u64,
}

/// The HTTP client the sign-in library calls. Redirects stay off, as the library's docs ask.
async fn http(request: HttpRequest) -> Result<HttpResponse, reqwest::Error> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client.execute(request.try_into()?).await?;
    let mut built = HttpResponse::new(Vec::new());
    *built.status_mut() = response.status();
    *built.headers_mut() = response.headers().clone();
    *built.body_mut() = response.bytes().await?.to_vec();
    Ok(built)
}

impl Oidc {
    async fn client(&self, st: &Shared) -> Result<&Client> {
        self.client
            .get_or_try_init(|| async {
                if st.config.oidc_client_id.is_empty() {
                    bail!("OIDC_CLIENT_ID isn't set");
                }
                let issuer = IssuerUrl::new(st.config.oidc_issuer.clone())?;
                let provider = CoreProviderMetadata::discover_async(issuer, &http)
                    .await
                    .map_err(|e| anyhow!("finding the sign-in provider: {e}"))?;
                let back = format!("{}/auth/callback", st.config.public_url);
                let client = CoreClient::from_provider_metadata(
                    provider,
                    ClientId::new(st.config.oidc_client_id.clone()),
                    Some(ClientSecret::new(st.config.oidc_client_secret.clone())),
                )
                .set_redirect_uri(RedirectUrl::new(back)?);
                Ok(client)
            })
            .await
    }
}

#[derive(Deserialize)]
pub struct LoginQuery {
    /// Passed on as OpenID Connect's login_hint: which account to offer first.
    hint: Option<String>,
}

pub async fn login(
    State(st): State<Shared>,
    Query(query): Query<LoginQuery>,
) -> Result<Response, ApiError> {
    let client = st.oidc.client(&st).await.map_err(ApiError::Internal)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".to_owned()))
        .add_scope(Scope::new("profile".to_owned()))
        .set_pkce_challenge(challenge);
    if let Some(hint) = query.hint {
        request = request.add_extra_param("login_hint", hint);
    }
    let (url, state, nonce) = request.url();
    let pending = Pending {
        nonce: nonce.secret().clone(),
        verifier: verifier.secret().clone(),
        until_ms: now_ms() + PENDING_MS,
    };
    st.kv.put(keys::pending(state.secret()), &pending).await?;
    Ok(Redirect::to(url.as_str()).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

pub async fn callback(
    State(st): State<Shared>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, ApiError> {
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return Err(ApiError::Bad("Sign-in was cancelled.".to_owned()));
    };
    let key = keys::pending(&state);
    let pending: Option<Pending> = st.kv.get(key.clone()).await?;
    let Some(pending) = pending.filter(|p| p.until_ms > now_ms()) else {
        return Err(ApiError::Bad(
            "That sign-in ran out. Please try again.".to_owned(),
        ));
    };
    st.kv.delete(key).await?;

    let claims = exchange(&st, code, &pending)
        .await
        .map_err(ApiError::Internal)?;
    let Some(person) = person_for(&st, &claims).await? else {
        let email = claims.email().map_or("That account", |e| e.as_str());
        return Err(ApiError::Refused(format!(
            "{email} isn't on this test server yet."
        )));
    };

    let mut token = [0u8; 32];
    rand::rng().fill_bytes(&mut token);
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token);
    let session = Session {
        person,
        until_ms: now_ms() + SESSION_MS,
    };
    st.kv
        .put(keys::session(&sha256_hex(token.as_bytes())), &session)
        .await?;
    let cookie = Cookie::build((COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(st.config.public_url.starts_with("https://"))
        .max_age(time::Duration::milliseconds(SESSION_MS as i64))
        .build();
    Ok((jar.add(cookie), Redirect::to("/")).into_response())
}

async fn exchange(st: &Shared, code: String, pending: &Pending) -> Result<CoreIdTokenClaims> {
    let client = st.oidc.client(st).await?;
    let token = client
        .exchange_code(AuthorizationCode::new(code))?
        .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier.clone()))
        .request_async(&http)
        .await
        .map_err(|e| anyhow!("exchanging the sign-in code: {e}"))?;
    let id_token = token
        .extra_fields()
        .id_token()
        .context("no ID token came back")?;
    let claims = id_token
        .claims(
            &client.id_token_verifier(),
            &Nonce::new(pending.nonce.clone()),
        )
        .map_err(|e| anyhow!("checking the ID token: {e}"))?;
    Ok(claims.clone())
}

/// Finds the person a login belongs to. A login is bound by its subject the first time it signs
/// in, from a verified address only, so one login stays one person even if the address changes.
async fn person_for(st: &Shared, claims: &CoreIdTokenClaims) -> Result<Option<String>> {
    let subject = format!("{}\n{}", st.config.oidc_issuer, claims.subject().as_str());
    let login_key = keys::login(&sha256_hex(subject.as_bytes()));
    if let Some(person) = st.kv.get::<String>(login_key.clone()).await? {
        return Ok(Some(person));
    }
    let Some(email) = claims
        .email()
        .filter(|_| claims.email_verified() == Some(true))
    else {
        return Ok(None);
    };
    let found: Option<String> = st
        .kv
        .get(keys::email(&email.as_str().to_lowercase()))
        .await?;
    if let Some(person) = &found {
        st.kv.put(login_key, person).await?;
    }
    Ok(found)
}

/// Writes the STACK_PEOPLE setting: which real addresses sign in as which test people.
pub async fn apply_people(st: &Shared) -> Result<()> {
    for pair in st.config.people.split(',').filter(|p| !p.trim().is_empty()) {
        let (person, email) = pair
            .split_once('=')
            .context("STACK_PEOPLE wants person=address")?;
        let person = person.trim();
        let row: Option<Person> = st.kv.get(keys::person(person)).await?;
        if row.is_none() {
            bail!("STACK_PEOPLE names {person}, who isn't in the seed data");
        }
        st.kv
            .put(keys::email(&email.trim().to_lowercase()), &person)
            .await?;
    }
    Ok(())
}

pub async fn logout(State(st): State<Shared>, jar: CookieJar) -> Result<Response, ApiError> {
    if let Some(cookie) = jar.get(COOKIE) {
        st.kv
            .delete(keys::session(&sha256_hex(cookie.value().as_bytes())))
            .await?;
    }
    Ok((
        jar.remove(Cookie::build(COOKIE).path("/").build()),
        Redirect::to("/"),
    )
        .into_response())
}

/// The signed-in person. Any handler that takes this refuses everyone else.
pub struct Me(pub String);

impl FromRequestParts<Shared> for Me {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, st: &Shared) -> Result<Self, ApiError> {
        let jar = CookieJar::from_headers(&parts.headers);
        let cookie = jar.get(COOKIE).ok_or(ApiError::SignIn)?;
        let session: Option<Session> = st
            .kv
            .get(keys::session(&sha256_hex(cookie.value().as_bytes())))
            .await?;
        match session {
            Some(session) if session.until_ms > now_ms() => Ok(Me(session.person)),
            _ => Err(ApiError::SignIn),
        }
    }
}
