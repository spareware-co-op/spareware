//! Sign-in with OpenID Connect, and sessions. The local test runs against a stand-in provider and
//! the server against Google; the only difference is a setting, so there is no test back door.
//!
//! Sign-in state lives in the store but not in a log (note K08): sessions are secrets, and they
//! expire. Keeping them in the store means a restart signs nobody out, and any server can serve
//! any browser.

use crate::people::SYSTEM;
use base64::Engine;
use kit_core::Id;
use kit_ops::{Edit, sha256_hex};
use kit_store::{Kv, Place, Reader, StoreError, Txn, read_row, to_row};
use kit_sync::{Clock, Commands, Orderers, Reply};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreIdTokenClaims, CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope as OidcScope,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::OnceCell;

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

/// The sign-in settings.
#[derive(Clone, Debug)]
pub struct SignInSettings {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    /// The address people type, without a trailing slash.
    pub public_url: String,
}

/// Why a sign-in didn't work, in words a person can act on.
#[derive(Debug)]
pub enum SignInError {
    /// The link ran out or was used already.
    Expired,
    /// The account isn't on this server.
    Unknown(String),
    /// Setting up with the provider failed.
    Provider(String),
    Store(StoreError),
}

impl From<StoreError> for SignInError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

#[derive(Serialize, Deserialize)]
struct Pending {
    nonce: String,
    verifier: String,
    until_ms: u64,
}

#[derive(Serialize, Deserialize)]
struct Session {
    #[serde(with = "kit_ops::id")]
    person: Id,
    until_ms: u64,
}

/// The HTTP client the sign-in library calls. Redirects stay off, as its documentation asks.
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

/// The hash an address is known by.
#[must_use]
pub fn email_hash(address: &str) -> String {
    sha256_hex(address.trim().to_lowercase().as_bytes())
}

fn id_of_hash(hash: &str) -> Result<Id, StoreError> {
    Id::parse(hash).map_err(|e| StoreError::Damaged(e.to_string()))
}

/// Accounts: sign-in, sessions and server-run directory edits. `C` is the program's directory
/// commands, which run [`crate::people`]'s alongside its own.
pub struct Accounts<K: Kv, C: Commands<K>> {
    kv: K,
    clock: Arc<dyn Clock>,
    settings: SignInSettings,
    client: OnceCell<Client>,
    directory: Orderers<K, C>,
}

impl<K: Kv, C: Commands<K>> Accounts<K, C> {
    pub fn new(
        kv: K,
        clock: Arc<dyn Clock>,
        settings: SignInSettings,
        directory: Orderers<K, C>,
    ) -> Self {
        Self {
            kv,
            clock,
            settings,
            client: OnceCell::new(),
            directory,
        }
    }

    /// Runs one directory edit. `author` must be someone Cedar lets make it.
    pub async fn edit_directory(
        &self,
        author: &Id,
        cmd: &str,
        args: Value,
    ) -> Result<(), StoreError> {
        let mut random = [0u8; 10];
        rand::rng().fill_bytes(&mut random);
        let now = self.clock.now_ms();
        let client = Id::from_time_and_random(now, random);
        let edit = Edit {
            person: author.clone(),
            client,
            counter: 1,
            base: 0,
            target: author.clone(),
            cmd: cmd.to_owned(),
            args,
            author_ms: now,
        };
        let scope = kit_store::Scope::Directory;
        loop {
            match self.directory.run(&scope, edit.clone()).await? {
                Reply::Ack { .. } => return Ok(()),
                Reply::Rejected { reason, .. } => return Err(StoreError::Unavailable(reason)),
                Reply::Retry { .. } => {}
            }
        }
    }

    /// Starts an empty directory. Does nothing to one already started.
    pub async fn start_directory(&self) -> Result<(), StoreError> {
        let system = id_of_hash(SYSTEM)?;
        let mut snapshot = self.kv.snapshot().await?;
        if snapshot
            .get(&Place::Admin(system.clone()).key())
            .await?
            .is_some()
        {
            return Ok(());
        }
        self.edit_directory(&system, "directory.start", json!({}))
            .await
    }

    /// Applies the `STACK_PEOPLE` setting: which addresses sign in as which people.
    pub async fn apply_people(&self, setting: &str) -> Result<(), StoreError> {
        let system = id_of_hash(SYSTEM)?;
        for pair in setting.split(',').filter(|pair| !pair.trim().is_empty()) {
            let (person, address) = pair.split_once('=').ok_or_else(|| {
                StoreError::Unavailable("STACK_PEOPLE wants person=address".to_owned())
            })?;
            let (person, hash) = (person.trim(), email_hash(address));
            let place = Place::Email(id_of_hash(&hash)?);
            let mut snapshot = self.kv.snapshot().await?;
            let owner: Option<String> = read_row(&mut snapshot, &place.key()).await?;
            if owner.as_deref() != Some(person) {
                let args = json!({ "person": person, "email_hash": hash });
                self.edit_directory(&system, "person.email", args).await?;
            }
        }
        Ok(())
    }

    async fn client(&self) -> Result<&Client, SignInError> {
        self.client
            .get_or_try_init(|| async {
                let bad = |e: &dyn std::fmt::Display| SignInError::Provider(e.to_string());
                if self.settings.client_id.is_empty() {
                    return Err(SignInError::Provider("OIDC_CLIENT_ID isn't set".to_owned()));
                }
                let issuer = IssuerUrl::new(self.settings.issuer.clone()).map_err(|e| bad(&e))?;
                let provider = CoreProviderMetadata::discover_async(issuer, &http)
                    .await
                    .map_err(|e| bad(&e))?;
                let back = format!("{}/auth/callback", self.settings.public_url);
                let client = CoreClient::from_provider_metadata(
                    provider,
                    ClientId::new(self.settings.client_id.clone()),
                    Some(ClientSecret::new(self.settings.client_secret.clone())),
                )
                .set_redirect_uri(RedirectUrl::new(back).map_err(|e| bad(&e))?);
                Ok(client)
            })
            .await
    }

    /// Where to send a browser to sign in. `hint` is OpenID Connect's `login_hint`.
    pub async fn begin(&self, hint: Option<String>) -> Result<String, SignInError> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut request = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(OidcScope::new("email".to_owned()))
            .add_scope(OidcScope::new("profile".to_owned()))
            .set_pkce_challenge(challenge);
        if let Some(hint) = hint {
            request = request.add_extra_param("login_hint", hint);
        }
        let (url, state, nonce) = request.url();
        let pending = Pending {
            nonce: nonce.secret().clone(),
            verifier: verifier.secret().clone(),
            until_ms: self.clock.now_ms() + PENDING_MS,
        };
        let key = Place::Pending(id_of_hash(&sha256_hex(state.secret().as_bytes()))?).key();
        let mut txn = self.kv.begin().await?;
        txn.put(key, to_row(&pending)?).await?;
        txn.commit().await?;
        Ok(url.to_string())
    }

    /// Finishes a sign-in and returns the new session's cookie value.
    pub async fn finish(&self, code: String, state: &str) -> Result<String, SignInError> {
        let key = Place::Pending(id_of_hash(&sha256_hex(state.as_bytes()))?).key();
        let mut snapshot = self.kv.snapshot().await?;
        let pending: Option<Pending> = read_row(&mut snapshot, &key).await?;
        let now = self.clock.now_ms();
        let Some(pending) = pending.filter(|p| p.until_ms > now) else {
            return Err(SignInError::Expired);
        };
        let mut txn = self.kv.begin().await?;
        txn.delete(key).await?;
        txn.commit().await?;

        let claims = self.exchange(code, &pending).await?;
        let Some(person) = self.person_for(&claims).await? else {
            let address = claims
                .email()
                .map_or("That account", |e| e.as_str())
                .to_owned();
            return Err(SignInError::Unknown(address));
        };

        let mut token = [0u8; 32];
        rand::rng().fill_bytes(&mut token);
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token);
        let session = Session {
            person,
            until_ms: self.clock.now_ms() + SESSION_MS,
        };
        let place = Place::Session(id_of_hash(&sha256_hex(token.as_bytes()))?);
        let mut txn = self.kv.begin().await?;
        txn.put(place.key(), to_row(&session)?).await?;
        txn.commit().await?;
        Ok(token)
    }

    async fn exchange(
        &self,
        code: String,
        pending: &Pending,
    ) -> Result<CoreIdTokenClaims, SignInError> {
        let bad = |e: &dyn std::fmt::Display| SignInError::Provider(e.to_string());
        let client = self.client().await?;
        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|e| bad(&e))?
            .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier.clone()))
            .request_async(&http)
            .await
            .map_err(|e| bad(&e))?;
        let id_token = token.extra_fields().id_token();
        let id_token = id_token.ok_or_else(|| SignInError::Provider("no ID token".to_owned()))?;
        let claims = id_token
            .claims(
                &client.id_token_verifier(),
                &Nonce::new(pending.nonce.clone()),
            )
            .map_err(|e| bad(&e))?;
        Ok(claims.clone())
    }

    /// Finds the person a login belongs to. A login is bound by its subject the first time it
    /// signs in, from a verified address only, so one login stays one person even if the address
    /// changes.
    async fn person_for(&self, claims: &CoreIdTokenClaims) -> Result<Option<Id>, StoreError> {
        let subject = format!("{}\n{}", self.settings.issuer, claims.subject().as_str());
        let login = id_of_hash(&sha256_hex(subject.as_bytes()))?;
        let mut snapshot = self.kv.snapshot().await?;
        let bound: Option<String> =
            read_row(&mut snapshot, &Place::Login(login.clone()).key()).await?;
        if let Some(person) = bound {
            return id_of_hash(&person).map(Some);
        }
        let Some(address) = claims
            .email()
            .filter(|_| claims.email_verified() == Some(true))
        else {
            return Ok(None);
        };
        let hash = email_hash(address.as_str());
        let place = Place::Email(id_of_hash(&hash)?);
        let found: Option<String> = read_row(&mut snapshot, &place.key()).await?;
        let Some(person) = found else { return Ok(None) };
        let person = id_of_hash(&person)?;
        let args = json!({ "login": login.as_str(), "email_hash": hash });
        self.edit_directory(&person, "login.bind", args).await?;
        Ok(Some(person))
    }

    /// Who a session cookie belongs to.
    pub async fn session(&self, token: &str) -> Result<Option<Id>, StoreError> {
        let place = Place::Session(id_of_hash(&sha256_hex(token.as_bytes()))?);
        let mut snapshot = self.kv.snapshot().await?;
        let session: Option<Session> = read_row(&mut snapshot, &place.key()).await?;
        let now = self.clock.now_ms();
        Ok(session.filter(|s| s.until_ms > now).map(|s| s.person))
    }

    /// Ends a session.
    pub async fn sign_out(&self, token: &str) -> Result<(), StoreError> {
        let place = Place::Session(id_of_hash(&sha256_hex(token.as_bytes()))?);
        let mut txn = self.kv.begin().await?;
        txn.delete(place.key()).await?;
        txn.commit().await
    }
}
