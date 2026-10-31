//! The web server: pages, the JSON API and its routes. Every handler that touches a hub, project
//! or model asks Cedar first.

use crate::access::{Access, Target};
use crate::auth::{self, Me, Oidc};
use crate::config::Config;
use crate::folders;
use crate::keys;
use crate::kv::{self, Kv};
use crate::orderer::Orderers;
use crate::rows::{
    Element, Hub, Incoming, Item, Link, LinkKind, Model, Person, Project, Shape, Team,
    cells_in_view,
};
use crate::ws;
use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use object_store::{ObjectStore, ObjectStoreExt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock};

pub type Shared = Arc<StackState>;

pub struct StackState {
    pub config: Config,
    pub kv: Kv,
    pub access: Arc<Access>,
    pub orderers: Orderers,
    pub files: Arc<dyn ObjectStore>,
    pub oidc: Oidc,
}

impl StackState {
    pub async fn require(
        &self,
        person: &str,
        action: &str,
        target: Target<'_>,
    ) -> Result<(), ApiError> {
        if self.access.allowed(person, action, target, None).await? {
            Ok(())
        } else {
            Err(ApiError::Refused(
                "You don't have access to this.".to_owned(),
            ))
        }
    }
}

pub enum ApiError {
    SignIn,
    Refused(String),
    NotFound,
    Bad(String),
    Internal(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self::Internal(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::SignIn => (StatusCode::UNAUTHORIZED, "Please sign in.".to_owned()),
            Self::Refused(why) => (StatusCode::FORBIDDEN, why),
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found.".to_owned()),
            Self::Bad(why) => (StatusCode::BAD_REQUEST, why),
            Self::Internal(e) => {
                tracing::error!("{e:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Something went wrong.".to_owned(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub fn routes(state: Shared) -> Router {
    Router::new()
        .route("/", get(page))
        .route("/connect.js", get(connect_script))
        .route("/connect.css", get(connect_style))
        .route("/design/ribbon/ribbon.js", get(ribbon_script))
        .route("/design/ribbon/ribbon.css", get(ribbon_style))
        .route("/health", get(health))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/api/me", get(me))
        .route("/api/hubs/{hub}", get(hub))
        .route("/api/projects/{project}", get(project))
        .route(
            "/api/projects/{project}/bridge-targets",
            get(bridge_targets),
        )
        .route("/api/projects/{project}/models/{model}/area", get(area))
        .route("/api/projects/{project}/models/{model}/claims", get(claims))
        .route(
            "/api/projects/{project}/models/{model}/links",
            get(links).post(add_link),
        )
        .route(
            "/api/projects/{project}/models/{model}/record/{hash}",
            get(record),
        )
        .route(
            "/api/projects/{project}/models/{model}/shape/{hash}",
            get(shape),
        )
        .route(
            "/api/projects/{project}/teams/{team}/publish",
            post(publish),
        )
        .route(
            "/api/projects/{project}/teams/{team}/consume",
            post(consume),
        )
        .route(
            "/api/projects/{project}/teams/{team}/packages/{package}/bridge",
            post(bridge),
        )
        .route(
            "/api/projects/{project}/teams/{team}/folders/{folder}/packages/{package}/file",
            get(package_file),
        )
        .route("/ws/{project}", get(ws::upgrade))
        .with_state(state)
}

/// The Spare Construct demo's own page, unchanged, with the connector added. The page is the
/// layout: its ribbon, docks and views are not copied here, so they stay in step with the demo.
static PAGE: LazyLock<String> = LazyLock::new(|| {
    let demo = include_str!("../../../spare-construct/demo/index.html");
    let head = demo.find("</head>").expect("the demo page has a head");
    let body = demo.rfind("</body>").expect("the demo page has a body");
    format!(
        "{}{}{}{}{}",
        &demo[..head],
        include_str!("../web/head.html"),
        &demo[head..body],
        include_str!("../web/body.html"),
        &demo[body..],
    )
});

async fn page() -> Html<&'static str> {
    Html(PAGE.as_str())
}

const JAVASCRIPT: (header::HeaderName, &str) = (header::CONTENT_TYPE, "text/javascript");
const CSS: (header::HeaderName, &str) = (header::CONTENT_TYPE, "text/css");

async fn connect_script() -> impl IntoResponse {
    ([JAVASCRIPT], include_str!("../web/connect.js"))
}

async fn connect_style() -> impl IntoResponse {
    ([CSS], include_str!("../web/connect.css"))
}

/// The shared ribbon, at the address the demo's own page asks for.
async fn ribbon_script() -> impl IntoResponse {
    (
        [JAVASCRIPT],
        include_str!("../../../../design/ribbon/ribbon.js"),
    )
}

async fn ribbon_style() -> impl IntoResponse {
    ([CSS], include_str!("../../../../design/ribbon/ribbon.css"))
}

async fn health(State(st): State<Shared>) -> Result<&'static str, ApiError> {
    st.kv.get::<Value>(keys::hub("health-check")).await?;
    Ok("ok")
}

/// The hubs a person belongs to, home first.
async fn me(State(st): State<Shared>, Me(id): Me) -> Result<Json<Value>, ApiError> {
    let person: Person = st
        .kv
        .get(keys::person(&id))
        .await?
        .ok_or(ApiError::SignIn)?;
    let prefix = format!("{}hub/", keys::acl_person_prefix(&id));
    let mut hubs = Vec::new();
    for (key, role) in st.kv.scan(&prefix).await? {
        let hub_id = key.strip_prefix(&prefix).unwrap_or_default();
        let role: String = serde_json::from_slice(&role)?;
        let row: Option<Hub> = st.kv.get(keys::hub(hub_id)).await?;
        if let Some(row) = row {
            hubs.push(json!({ "id": row.id, "name": row.name, "role": role }));
        }
    }
    hubs.sort_by_key(|hub| hub["id"] != person.home_hub.as_str());
    Ok(Json(json!({ "person": person, "hubs": hubs })))
}

/// A hub's people, and the projects this person is on.
async fn hub(
    State(st): State<Shared>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    st.require(&me, "hub.open", Target::Hub(&id)).await?;
    let row: Hub = st.kv.get(keys::hub(&id)).await?.ok_or(ApiError::NotFound)?;
    let prefix = keys::acl_hub_user_prefix(&id);
    let mut users = Vec::new();
    let mut externals = Vec::new();
    for (key, role) in st.kv.scan(&prefix).await? {
        let person = key.strip_prefix(&prefix).unwrap_or_default();
        let role: String = serde_json::from_slice(&role)?;
        let row: Option<Person> = st.kv.get(keys::person(person)).await?;
        let Some(row) = row else { continue };
        if role == "external" {
            externals.push(row.name)
        } else {
            users.push(row.name)
        }
    }
    let prefix = keys::hub_project_prefix(&id);
    let mut projects = Vec::new();
    for (key, _) in st.kv.scan(&prefix).await? {
        let project = key.strip_prefix(&prefix).unwrap_or_default();
        if st
            .access
            .allowed(&me, "project.open", Target::Project(project), None)
            .await?
        {
            let row: Option<Project> = st.kv.get(keys::project(project)).await?;
            projects.extend(row);
        }
    }
    Ok(Json(
        json!({ "hub": row, "users": users, "externals": externals, "projects": projects }),
    ))
}

/// A project's teams, with only the folders this person may open.
async fn project(
    State(st): State<Shared>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    st.require(&me, "project.open", Target::Project(&id))
        .await?;
    let row: Project = st
        .kv
        .get(keys::project(&id))
        .await?
        .ok_or(ApiError::NotFound)?;
    let hub: Hub = st
        .kv
        .get(keys::hub(&row.hub))
        .await?
        .ok_or(ApiError::NotFound)?;
    let head: u64 = st.kv.get(keys::head(&id)).await?.unwrap_or(0);
    let models: Vec<Model> = st.kv.scan_rows(&keys::model_prefix(&id)).await?;
    let teams: Vec<Team> = st.kv.scan_rows(&keys::team_prefix(&id)).await?;
    let my_teams = format!("{}team/{id}/", keys::acl_person_prefix(&me));
    let my_teams: Vec<String> = st
        .kv
        .scan(&my_teams)
        .await?
        .into_iter()
        .map(|(key, _)| key.strip_prefix(&my_teams).unwrap_or_default().to_owned())
        .collect();

    let mut listed = Vec::new();
    for team in teams {
        let mut folders = Vec::new();
        for kind in ["wip", "shared", "consumed"] {
            if !st
                .access
                .allowed(
                    &me,
                    "folder.open",
                    Target::Folder(&id, &team.id, kind),
                    None,
                )
                .await?
            {
                continue;
            }
            let entry = if kind == "wip" {
                let mine: Vec<&Model> = models.iter().filter(|m| m.team == team.id).collect();
                json!({ "kind": kind, "models": mine })
            } else {
                let items: Vec<Item> = st
                    .kv
                    .scan_rows(&keys::item_prefix(&id, &team.id, kind))
                    .await?;
                json!({ "kind": kind, "packages": items })
            };
            folders.push(entry);
        }
        let member = my_teams.contains(&team.id);
        listed.push(
            json!({ "id": team.id, "name": team.name, "member": member, "folders": folders }),
        );
    }

    let waiting: Vec<Incoming> = st.kv.scan_rows(&keys::incoming_prefix(&id)).await?;
    let waiting: Vec<Incoming> = waiting
        .into_iter()
        .filter(|i| my_teams.contains(&i.to_team))
        .collect();
    Ok(Json(
        json!({ "project": row, "hub": hub, "head": head, "teams": listed, "incoming": waiting }),
    ))
}

async fn bridge_targets(
    State(st): State<Shared>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    st.require(&me, "project.open", Target::Project(&id))
        .await?;
    Ok(Json(folders::bridge_targets(&st, &id).await?))
}

#[derive(Deserialize)]
struct View {
    level: i32,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

/// The pieces in view: element ids and record hashes only. The browser fetches each record it
/// hasn't already got, and its own cache keeps them by hash.
async fn area(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, model)): Path<(String, String)>,
    Query(view): Query<View>,
) -> Result<Json<Value>, ApiError> {
    st.require(&me, "model.read", Target::Model(&project, &model))
        .await?;
    if (view.x1 - view.x0).abs() > 1_000_000 || (view.y1 - view.y0).abs() > 1_000_000 {
        return Err(ApiError::Bad("That view is too big.".to_owned()));
    }
    let mut snap = st.kv.snapshot().await?;
    let mut pieces = BTreeMap::new();
    for cell in cells_in_view(&model, view.level, view.x0, view.y0, view.x1, view.y1) {
        let prefix = keys::area_prefix(&project, &cell);
        for (key, hash) in kv::scan(&mut snap, &prefix).await? {
            let element = key.strip_prefix(&prefix).unwrap_or_default().to_owned();
            let hash: String = serde_json::from_slice(&hash)?;
            pieces.insert(element, hash);
        }
    }
    let head: u64 = kv::get(&mut snap, keys::head(&project)).await?.unwrap_or(0);
    Ok(Json(json!({ "head": head, "pieces": pieces })))
}

async fn claims(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    st.require(&me, "model.read", Target::Model(&project, &model))
        .await?;
    let prefix = keys::claim_prefix(&project, &model);
    let mut found = BTreeMap::new();
    for (key, value) in st.kv.scan(&prefix).await? {
        let element = key.strip_prefix(&prefix).unwrap_or_default().to_owned();
        found.insert(element, serde_json::from_slice::<Value>(&value)?);
    }
    Ok(Json(json!(found)))
}

/// A record is named by its hash, so it never changes and the browser may keep it for ever.
const FOREVER: (header::HeaderName, &str) = (
    header::CACHE_CONTROL,
    "private, max-age=31536000, immutable",
);

async fn record(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, model, hash)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    st.require(&me, "model.read", Target::Model(&project, &model))
        .await?;
    let row: Element = st
        .kv
        .get(keys::record(&project, &hash))
        .await?
        .ok_or(ApiError::NotFound)?;
    // A reader of one model must not reach another model's records by naming a hash.
    if row.model != model {
        return Err(ApiError::NotFound);
    }
    Ok(([FOREVER], Json(row)))
}

async fn shape(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, model, hash)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    st.require(&me, "model.read", Target::Model(&project, &model))
        .await?;
    let row: Shape = st
        .kv
        .get(keys::shape(&project, &hash))
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(([FOREVER], Json(row)))
}

#[derive(Deserialize)]
struct PublishBody {
    model: String,
}

async fn publish(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, team)): Path<(String, String)>,
    Json(body): Json<PublishBody>,
) -> Result<Json<Item>, ApiError> {
    Ok(Json(
        folders::publish(&st, &me, &project, &team, &body.model).await?,
    ))
}

#[derive(Deserialize)]
struct BridgeBody {
    to_project: String,
    to_team: String,
}

async fn bridge(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, team, package)): Path<(String, String, String)>,
    Json(body): Json<BridgeBody>,
) -> Result<Json<Incoming>, ApiError> {
    let sent = folders::bridge(
        &st,
        &me,
        &project,
        &team,
        &package,
        &body.to_project,
        &body.to_team,
    );
    Ok(Json(sent.await?))
}

#[derive(Deserialize)]
struct ConsumeBody {
    incoming: String,
}

async fn consume(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, team)): Path<(String, String)>,
    Json(body): Json<ConsumeBody>,
) -> Result<Json<Item>, ApiError> {
    Ok(Json(
        folders::consume(&st, &me, &project, &team, &body.incoming).await?,
    ))
}

async fn package_file(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, team, folder, package)): Path<(String, String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    st.require(
        &me,
        "package.read",
        Target::Package(&project, &team, &folder, &package),
    )
    .await?;
    let item: Item = st
        .kv
        .get(keys::item(&project, &team, &folder, &package))
        .await?
        .ok_or(ApiError::NotFound)?;
    let file = st
        .files
        .get(&object_store::path::Path::from(item.file.as_str()))
        .await?;
    Ok((
        [(header::CONTENT_TYPE, "application/json"), FOREVER],
        file.bytes().await?,
    ))
}

async fn links(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<Json<Vec<Link>>, ApiError> {
    st.require(&me, "model.read", Target::Model(&project, &model))
        .await?;
    Ok(Json(
        st.kv
            .scan_rows(&keys::link_prefix(&project, &model))
            .await?,
    ))
}

#[derive(Deserialize)]
struct LinkBody {
    kind: LinkKind,
    team: Option<String>,
    source: String,
}

async fn add_link(
    State(st): State<Shared>,
    Me(me): Me,
    Path((project, model)): Path<(String, String)>,
    Json(body): Json<LinkBody>,
) -> Result<Json<Link>, ApiError> {
    let team = body.team.as_deref();
    let link = folders::add_link(&st, &me, &project, &model, body.kind, team, &body.source);
    Ok(Json(link.await?))
}
