//! Settings. Moving from one free server to three paid ones changes these and no code:
//! STACK_PD lists three addresses, and STACK_FILES_URL names S3 or Google Cloud Storage.

use clap::Args;

#[derive(Args, Clone, Debug)]
pub struct Settings {
    /// Where this server listens.
    #[arg(long, env = "STACK_BIND", default_value = "127.0.0.1:8080")]
    pub bind: String,
    /// The address people type, used for the sign-in return and the https-only cookie.
    #[arg(
        long,
        env = "STACK_PUBLIC_URL",
        default_value = "http://localhost:8080"
    )]
    pub public_url: String,
    /// PD addresses, comma separated. One on the free server, three in production.
    #[arg(
        long,
        env = "STACK_PD",
        default_value = "127.0.0.1:2379",
        value_delimiter = ','
    )]
    pub pd: Vec<String>,
    /// Where files go: s3://bucket, gs://bucket or file:///path.
    #[arg(long, env = "STACK_FILES_URL", default_value = "s3://spare-files")]
    pub files_url: String,
    /// Where backups go, best somewhere other than this server.
    #[arg(long, env = "STACK_BACKUP_URL", default_value = "s3://spare-backups")]
    pub backup_url: String,
    /// The OpenID Connect sign-in provider.
    #[arg(
        long,
        env = "OIDC_ISSUER",
        default_value = "https://accounts.google.com"
    )]
    pub oidc_issuer: String,
    #[arg(long, env = "OIDC_CLIENT_ID", default_value = "")]
    pub oidc_client_id: String,
    #[arg(long, env = "OIDC_CLIENT_SECRET", default_value = "")]
    pub oidc_client_secret: String,
    /// Maps people to the addresses that sign in as them: anna=someone@gmail.com,dan=...
    #[arg(long, env = "STACK_PEOPLE", default_value = "")]
    pub people: String,
    /// How often a server looks for edits another server ordered, in milliseconds.
    #[arg(long, env = "STACK_TAIL_MS", default_value_t = 30)]
    pub tail_ms: u64,
    /// How long an automatic claim lasts after the last edit, in milliseconds.
    #[arg(long, env = "STACK_CLAIM_MS", default_value_t = 30_000)]
    pub claim_ms: u64,
}
