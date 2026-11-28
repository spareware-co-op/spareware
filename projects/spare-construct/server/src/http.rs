//! The web layer: pages and the JSON API, nothing else. Each handler turns a request into one
//! call on [`Service`], which holds every rule. There is no store in reach here.

use crate::service::{Refusal, Service, View};
use crate::settings::Settings;
use crate::ws;
use axum::extract::{FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use construct_model::rows::LinkKind;
use kit_accounts::SignInError;
use kit_core::Id;
use kit_store_tikv::{Files, Tikv};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, LazyLock};

/// What every handler reaches: the service and the settings.
pub struct App {
    /// Every rule, and the only way to the store.
    pub service: Service<Tikv, Files>,
    /// The server's settings.
    pub settings: Settings,
}

/// The app, shared between handlers.
pub type Shared = Arc<App>;

const COOKIE: &str = "spare_session";
const SESSION_DAYS: i64 = 14;

/// A refusal, answered as an HTTP status and a message a person can read.
pub struct ApiError(pub Refusal);

impl<E: Into<Refusal>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            Refusal::SignIn => StatusCode::UNAUTHORIZED,
            Refusal::Refused(_) => StatusCode::FORBIDDEN,
            Refusal::NotFound => StatusCode::NOT_FOUND,
            Refusal::Bad(_) => StatusCode::BAD_REQUEST,
            Refusal::Failed(e) => {
                tracing::error!("{e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let message = match self.0 {
            Refusal::Failed(_) => "Something went wrong.".to_owned(),
            refusal => refusal.to_string(),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

/// Reads an ID from a web address. Anything else can't name anything, so it isn't found.
///
/// # Errors
///
/// Not found, when the text isn't an ID.
pub fn id(text: &str) -> Result<Id, ApiError> {
    Id::parse(text).map_err(|_| ApiError(Refusal::NotFound))
}

/// The signed-in person. Any handler that takes this refuses everyone else.
pub struct Me(pub Id);

impl FromRequestParts<Shared> for Me {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &Shared) -> Result<Self, ApiError> {
        let jar = CookieJar::from_headers(&parts.headers);
        let cookie = jar.get(COOKIE).ok_or(ApiError(Refusal::SignIn))?;
        let person = app.service.accounts.session(cookie.value()).await?;
        person.map(Me).ok_or(ApiError(Refusal::SignIn))
    }
}

/// The routes that list a model's types of the layered kinds, for the page's tools.
fn type_routes() -> Router<Shared> {
    Router::new()
        .route(
            "/api/projects/{project}/models/{model}/wall-types",
            get(wall_types),
        )
        .route(
            "/api/projects/{project}/models/{model}/floor-types",
            get(floor_types),
        )
        .route(
            "/api/projects/{project}/models/{model}/roof-types",
            get(roof_types),
        )
}

/// Every page and API route, each a handler over the app.
#[allow(clippy::too_many_lines, reason = "one entry per route")]
pub fn routes(app: Shared) -> Router {
    Router::new()
        .route("/", get(page))
        .route("/connect.js", get(connect_script))
        .route("/view3d.js", get(view_script))
        .route("/connect.css", get(connect_style))
        .route("/shapes/construct_shapes.js", get(shapes_script))
        .route("/shapes/construct_shapes_bg.wasm", get(shapes_wasm))
        .merge(kernel_shapes())
        .merge(type_routes())
        .route("/workshop/", get(workshop_page))
        .route("/workshop/js/{file}", get(workshop_script))
        .route(
            "/workshop/pkg/construct_workshop.js",
            get(workshop_kernel_script),
        )
        .route(
            "/workshop/pkg/construct_workshop_bg.wasm",
            get(workshop_kernel_wasm),
        )
        .route("/design/ribbon/ribbon.js", get(ribbon_script))
        .route("/design/ribbon/ribbon.css", get(ribbon_style))
        .route("/health", get(health))
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/hubs/{hub}", get(hub))
        .route("/api/projects/{project}", get(project))
        .route(
            "/api/projects/{project}/bridge-targets",
            get(bridge_targets),
        )
        .route("/api/projects/{project}/models/{model}/area", get(area))
        .route(
            "/api/projects/{project}/models/{model}/kind/{kind}",
            get(kind),
        )
        .route("/api/projects/{project}/models/{model}/claims", get(claims))
        .route("/api/projects/{project}/models/{model}/undo", get(undo))
        .route(
            "/api/projects/{project}/models/{model}/parameters/{element}",
            get(parameters),
        )
        .route(
            "/api/projects/{project}/models/{model}/catalog",
            get(catalog),
        )
        .route(
            "/api/projects/{project}/models/{model}/project-parameters",
            get(project_parameters),
        )
        .route("/api/projects/{project}/models/{model}/types", get(types))
        .route("/api/projects/{project}/models/{model}/levels", get(levels))
        .merge(view_routes())
        .merge(schedule_routes())
        .route(
            "/api/projects/{project}/models/{model}/families",
            get(families),
        )
        .route("/api/projects/{project}/models/{model}/rooms", get(rooms))
        .route(
            "/api/projects/{project}/models/{model}/warnings",
            get(warnings),
        )
        .route(
            "/api/projects/{project}/models/{model}/filled/{element}",
            get(filled),
        )
        .route(
            "/api/projects/{project}/models/{model}/links",
            get(links).post(add_link),
        )
        .route(
            "/api/projects/{project}/models/{model}/record/{hash}",
            get(record),
        )
        .route(
            "/api/projects/{project}/models/{model}/shape/{element}",
            get(shape),
        )
        .route(
            "/api/projects/{project}/models/{model}/export/ifc",
            get(crate::ifc_export::export_ifc),
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
        .with_state(app)
}

/// A model's views: the list, what one shows, and its plan as the server draws it; and its sheets.
fn view_routes() -> Router<Shared> {
    Router::new()
        .route("/api/projects/{project}/models/{model}/views", get(views))
        .route(
            "/api/projects/{project}/models/{model}/views/{view}",
            get(view),
        )
        .route(
            "/api/projects/{project}/models/{model}/views/{view}/plan",
            get(plan),
        )
        .route(
            "/api/projects/{project}/models/{model}/views/{view}/graphics",
            get(graphics),
        )
        .route("/api/projects/{project}/models/{model}/sheets", get(sheets))
        .route(
            "/api/projects/{project}/models/{model}/sheets/{sheet}",
            get(sheet),
        )
}

/// A model's schedules: the list, and one with its rows (read only).
fn schedule_routes() -> Router<Shared> {
    Router::new()
        .route(
            "/api/projects/{project}/models/{model}/schedules",
            get(schedules),
        )
        .route(
            "/api/projects/{project}/models/{model}/schedules/{schedule}",
            get(schedule),
        )
        .route(
            "/api/projects/{project}/models/{model}/schedule-categories",
            get(schedule_categories),
        )
}

async fn schedule_categories(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .schedule_categories(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn schedules(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .schedules(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn schedule(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, schedule)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .schedule(&person, &id(&project)?, &id(&model)?, &id(&schedule)?)
            .await?,
    ))
}

/// The Spare Construct demo's own page, unchanged, with the connector added. The page is the
/// layout: its ribbon, docks and views are not copied here, so they stay in step with the demo.
static PAGE: LazyLock<String> = LazyLock::new(|| {
    let demo = include_str!("../../demo/index.html");
    let head = demo.find("</head>").unwrap_or(0);
    let body = demo.rfind("</body>").unwrap_or(demo.len());
    format!(
        "{}{}{}{}{}",
        &demo[..head],
        include_str!("../../app/head.html"),
        &demo[head..body],
        include_str!("../../app/body.html"),
        &demo[body..],
    )
});

async fn page() -> Html<&'static str> {
    Html(PAGE.as_str())
}

const JAVASCRIPT: (header::HeaderName, &str) = (header::CONTENT_TYPE, "text/javascript");
const CSS: (header::HeaderName, &str) = (header::CONTENT_TYPE, "text/css");
/// A record is named by its hash, so it never changes and the browser may keep it for ever.
const FOREVER: (header::HeaderName, &str) = (
    header::CACHE_CONTROL,
    "private, max-age=31536000, immutable",
);

async fn connect_script() -> impl IntoResponse {
    ([JAVASCRIPT], include_str!("../../app/connect.js"))
}

/// What the 3D view draws, as a module connect.js loads when the view opens.
async fn view_script() -> impl IntoResponse {
    ([JAVASCRIPT], include_str!("../../app/view3d.js"))
}

async fn connect_style() -> impl IntoResponse {
    ([CSS], include_str!("../../app/connect.css"))
}

/// The geometry the server builds with, compiled for the browser.
async fn shapes_script() -> impl IntoResponse {
    (
        [JAVASCRIPT],
        include_str!("../../app/pkg/construct_shapes.js"),
    )
}

async fn shapes_wasm() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/wasm")],
        &include_bytes!("../../app/pkg/construct_shapes_bg.wasm")[..],
    )
}

/// The geometry kernel's builder for the browser, which the page loads for a document on the
/// kernel's edition: only a server built with the `kernel` feature builds such a document, so only
/// it serves the package. It is read from where `scripts/build-shapes-kernel.sh` put it (the
/// `shapes_kernel` setting), since it isn't committed; when it isn't there, the page can't build
/// such a document and shows the stored shapes, as for any edition it can't run.
#[cfg(feature = "kernel")]
fn kernel_shapes() -> Router<Shared> {
    Router::new().route("/shapes/kernel/{file}", get(kernel_shapes_file))
}

#[cfg(not(feature = "kernel"))]
fn kernel_shapes() -> Router<Shared> {
    Router::new()
}

#[cfg(feature = "kernel")]
async fn kernel_shapes_file(State(app): State<Shared>, Path(file): Path<String>) -> Response {
    let kind = match file.as_str() {
        "construct_shapes_kernel.js" => "text/javascript",
        "construct_shapes_kernel_bg.wasm" => "application/wasm",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match tokio::fs::read(app.settings.shapes_kernel.join(&file)).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, kind)], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Workshop, the discipline for parts, assemblies and drawings: the demo's own page, which the
/// demo's frame opens at `workshop/`, unchanged, with its scripts and the geometry kernel's builder
/// compiled for the browser (`construct-workshop`).
async fn workshop_page() -> Html<&'static str> {
    Html(include_str!("../../demo/workshop/index.html"))
}

const WORKSHOP_SCRIPTS: [(&str, &str); 6] = [
    ("build.js", include_str!("../../demo/workshop/js/build.js")),
    (
        "drawing.js",
        include_str!("../../demo/workshop/js/drawing.js"),
    ),
    (
        "kernel.js",
        include_str!("../../demo/workshop/js/kernel.js"),
    ),
    ("model.js", include_str!("../../demo/workshop/js/model.js")),
    ("ui.js", include_str!("../../demo/workshop/js/ui.js")),
    (
        "view3d.js",
        include_str!("../../demo/workshop/js/view3d.js"),
    ),
];

async fn workshop_script(Path(file): Path<String>) -> Response {
    WORKSHOP_SCRIPTS
        .iter()
        .find(|(name, _)| *name == file)
        .map_or_else(
            || StatusCode::NOT_FOUND.into_response(),
            |(_, text)| ([JAVASCRIPT], *text).into_response(),
        )
}

async fn workshop_kernel_script() -> impl IntoResponse {
    (
        [JAVASCRIPT],
        include_str!("../../demo/workshop/pkg/construct_workshop.js"),
    )
}

async fn workshop_kernel_wasm() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/wasm")],
        &include_bytes!("../../demo/workshop/pkg/construct_workshop_bg.wasm")[..],
    )
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

async fn health(State(app): State<Shared>) -> Result<&'static str, ApiError> {
    app.service.healthy().await?;
    Ok("ok")
}

#[derive(Deserialize)]
struct LoginQuery {
    /// Passed on as OpenID Connect's `login_hint`: which account to offer first.
    hint: Option<String>,
}

fn sign_in_failed(error: SignInError) -> ApiError {
    ApiError(match error {
        SignInError::Expired => Refusal::Bad("That sign-in ran out. Please try again.".to_owned()),
        SignInError::Unknown(who) => Refusal::Refused(format!("{who} isn't on this server yet.")),
        SignInError::Provider(why) => Refusal::Failed(kit_store::StoreError::Unavailable(why)),
        SignInError::Store(e) => Refusal::Failed(e),
    })
}

async fn login(
    State(app): State<Shared>,
    Query(query): Query<LoginQuery>,
) -> Result<Response, ApiError> {
    let url = app
        .service
        .accounts
        .begin(query.hint)
        .await
        .map_err(sign_in_failed)?;
    Ok(Redirect::to(&url).into_response())
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

async fn callback(
    State(app): State<Shared>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, ApiError> {
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return Err(ApiError(Refusal::Bad("Sign-in was cancelled.".to_owned())));
    };
    let token = app
        .service
        .accounts
        .finish(code, &state)
        .await
        .map_err(sign_in_failed)?;
    let cookie = Cookie::build((COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(app.settings.public_url.starts_with("https://"))
        .max_age(time::Duration::days(SESSION_DAYS))
        .build();
    Ok((jar.add(cookie), Redirect::to("/")).into_response())
}

async fn logout(State(app): State<Shared>, jar: CookieJar) -> Result<Response, ApiError> {
    if let Some(cookie) = jar.get(COOKIE) {
        app.service.accounts.sign_out(cookie.value()).await?;
    }
    Ok((
        jar.remove(Cookie::build(COOKIE).path("/").build()),
        Redirect::to("/"),
    )
        .into_response())
}

async fn me(State(app): State<Shared>, Me(person): Me) -> Result<Json<Value>, ApiError> {
    Ok(Json(app.service.me(&person).await?))
}

async fn hub(
    State(app): State<Shared>,
    Me(person): Me,
    Path(hub): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(app.service.hub(&person, &id(&hub)?).await?))
}

async fn project(
    State(app): State<Shared>,
    Me(person): Me,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(app.service.project(&person, &id(&project)?).await?))
}

async fn bridge_targets(
    State(app): State<Shared>,
    Me(person): Me,
    Path(project): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        app.service.bridge_targets(&person, &id(&project)?).await?,
    ))
}

#[derive(Deserialize)]
struct ViewQuery {
    level: String,
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

async fn area(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
    Query(q): Query<ViewQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let view = View {
        level: id(&q.level)?,
        x0: q.x0,
        y0: q.y0,
        x1: q.x1,
        y1: q.y1,
    };
    Ok(Json(
        app.service
            .area(&person, &id(&project)?, &id(&model)?, &view)
            .await?,
    ))
}

async fn kind(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, kind)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .kind(&person, &id(&project)?, &id(&model)?, &id(&kind)?)
            .await?,
    ))
}

async fn parameters(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, element)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .parameters(&person, &id(&project)?, &id(&model)?, &id(&element)?)
            .await?,
    ))
}

async fn catalog(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .catalog(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn project_parameters(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .project_parameters(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

#[derive(Deserialize)]
struct TypesQuery {
    category: Option<u32>,
}

async fn types(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
    Query(q): Query<TypesQuery>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .types(&person, &id(&project)?, &id(&model)?, q.category)
            .await?,
    ))
}

async fn levels(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .levels(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn wall_types(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .wall_types(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn families(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .families(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn views(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .views(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn graphics(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, view)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .graphics(&person, &id(&project)?, &id(&model)?, &id(&view)?)
            .await?,
    ))
}

async fn sheets(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .sheets(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn sheet(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, sheet)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .sheet(&person, &id(&project)?, &id(&model)?, &id(&sheet)?)
            .await?,
    ))
}

async fn plan(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, view)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .plan(&person, &id(&project)?, &id(&model)?, &id(&view)?)
            .await?,
    ))
}

async fn floor_types(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .floor_types(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn roof_types(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .roof_types(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn rooms(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .rooms(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn warnings(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .warnings(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn filled(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, element)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .filled(&person, &id(&project)?, &id(&model)?, &id(&element)?)
            .await?,
    ))
}

async fn view(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, view)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .view(&person, &id(&project)?, &id(&model)?, &id(&view)?)
            .await?,
    ))
}

async fn undo(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .undo(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn claims(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        app.service
            .claims(&person, &id(&project)?, &id(&model)?)
            .await?,
    ))
}

async fn record(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, hash)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let row = app
        .service
        .record(&person, &id(&project)?, &id(&model)?, &id(&hash)?)
        .await?;
    Ok(([FOREVER], Json(row)))
}

#[derive(Deserialize)]
struct ShapeQuery {
    lod: Option<u32>,
}

async fn shape(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model, element)): Path<(String, String, String)>,
    Query(q): Query<ShapeQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let row = app
        .service
        .shape(&person, &id(&project)?, &id(&model)?, &id(&element)?, q.lod)
        .await?;
    Ok(Json(row))
}

#[derive(Deserialize)]
struct PublishBody {
    model: String,
}

async fn publish(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, team)): Path<(String, String)>,
    Json(body): Json<PublishBody>,
) -> Result<impl IntoResponse, ApiError> {
    let item = app
        .service
        .publish(&person, &id(&project)?, &id(&team)?, &id(&body.model)?)
        .await?;
    Ok(Json(item))
}

#[derive(Deserialize)]
struct BridgeBody {
    to_project: String,
    to_team: String,
}

async fn bridge(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, team, package)): Path<(String, String, String)>,
    Json(body): Json<BridgeBody>,
) -> Result<Json<Value>, ApiError> {
    let (project, team, package) = (id(&project)?, id(&team)?, id(&package)?);
    let (to_project, to_team) = (id(&body.to_project)?, id(&body.to_team)?);
    app.service
        .bridge(&person, &project, &team, &package, &to_project, &to_team)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct ConsumeBody {
    incoming: String,
}

async fn consume(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, team)): Path<(String, String)>,
    Json(body): Json<ConsumeBody>,
) -> Result<impl IntoResponse, ApiError> {
    let item = app
        .service
        .consume(&person, &id(&project)?, &id(&team)?, &id(&body.incoming)?)
        .await?;
    Ok(Json(item))
}

async fn package_file(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, team, folder, package)): Path<(String, String, String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    let (project, team, folder, package) = (id(&project)?, id(&team)?, id(&folder)?, id(&package)?);
    let bytes = app
        .service
        .package_file(&person, &project, &team, &folder, &package)
        .await?;
    Ok(([(header::CONTENT_TYPE, "application/json"), FOREVER], bytes))
}

async fn links(
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        app.service
            .links(&person, &id(&project)?, &id(&model)?)
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
    State(app): State<Shared>,
    Me(person): Me,
    Path((project, model)): Path<(String, String)>,
    Json(body): Json<LinkBody>,
) -> Result<Json<Value>, ApiError> {
    let team = body.team.as_deref().map(id).transpose()?;
    let source = id(&body.source)?;
    app.service
        .add_link(
            &person,
            &id(&project)?,
            &id(&model)?,
            body.kind,
            team.as_ref(),
            &source,
        )
        .await?;
    Ok(Json(json!({ "ok": true })))
}
