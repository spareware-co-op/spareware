//! What the server logged for each request, kept a little while, so a person's report can carry the
//! server's own lines for their requests (piece A100, note A100-report-a-problem.md).
//!
//! Every request gets an ID ([`with_request_ids`]: tower-http's request-id layers, MIT), answered
//! in the `x-request-id` header, and runs in a span `request` holding it, the method, the path and,
//! once [`crate::http::Me`] has read the session, the signed-in account's ID. The live channel's
//! connection runs in a span `live` with its upgrade request's ID (`ws.rs`), which the page is told
//! in the welcome. [`layer`] keeps the last [`KEPT`] lines logged in such a span, each with its
//! request's ID and account, and [`lines_for`] gives a person back only the lines of their own
//! requests: nothing in a line names anyone but by account ID.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request};
use std::collections::VecDeque;
use std::fmt::{self, Write as _};
use std::sync::{LazyLock, Mutex, PoisonError};
use tower_http::request_id::{
    MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer,
};
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Span, Subscriber, span};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// The header a request's ID travels in, both ways.
pub const HEADER: &str = "x-request-id";
/// How many lines are kept, across every request.
pub const KEPT: usize = 4096;
/// The longest line kept, in characters.
const LINE_CHARS: usize = 300;

/// A request's ID: eight random bytes in hex. Not a secret: it only finds lines, and only the
/// lines of the person asking.
#[derive(Clone, Copy, Default)]
struct NewId;

impl MakeRequestId for NewId {
    fn make_request_id<B>(&mut self, _: &Request<B>) -> Option<RequestId> {
        let bytes: [u8; 8] = rand::random();
        HeaderValue::from_str(&hex::encode(bytes))
            .ok()
            .map(RequestId::new)
    }
}

/// The request's span: its ID, the account once it is known, the method and the path.
fn span_of(request: &Request<Body>) -> Span {
    let id = request
        .headers()
        .get(HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    tracing::info_span!(
        "request",
        request = id,
        person = tracing::field::Empty,
        method = %request.method(),
        path = request.uri().path(),
    )
}

/// `router` with an ID for every request, its span, and a line for each answer.
pub fn with_request_ids(router: Router) -> Router {
    // The last layer added is the outermost: the ID is set first, then the span opens with it.
    router
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| span_of(request))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(SetRequestIdLayer::x_request_id(NewId))
}

struct Line {
    request: String,
    person: Option<String>,
    text: String,
}

static LINES: LazyLock<Mutex<VecDeque<Line>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(KEPT)));

fn keep(line: Line) {
    let mut lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    if lines.len() >= KEPT {
        lines.pop_front();
    }
    lines.push_back(line);
}

/// The kept lines of `person`'s own requests among `requests`, oldest first, each starting with
/// its request's ID; at most `most`.
#[must_use]
pub fn lines_for(person: &str, requests: &[String], most: usize) -> Vec<String> {
    let lines = LINES.lock().unwrap_or_else(PoisonError::into_inner);
    let mine: Vec<String> = lines
        .iter()
        .filter(|line| {
            line.person.as_deref() == Some(person) && requests.contains(&line.request)
        })
        .map(|line| format!("{} {}", line.request, line.text))
        .collect();
    let skip = mine.len().saturating_sub(most);
    mine.into_iter().skip(skip).collect()
}

/// A span's request ID and account, as the layer keeps them.
struct Tags {
    request: String,
    person: Option<String>,
}

/// Reads a span's `request` and `person` fields.
#[derive(Default)]
struct Fields {
    request: Option<String>,
    person: Option<String>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "request" if !value.is_empty() => self.request = Some(value.to_owned()),
            "person" => self.person = Some(value.to_owned()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let text = format!("{value:?}");
        self.record_str(field, text.trim_matches('"'));
    }
}

/// An event's words: its message, then its other fields as `name=value`.
#[derive(Default)]
struct Words {
    message: String,
    rest: String,
}

impl Visit for Words {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            let _ = write!(self.rest, " {}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            let _ = write!(self.rest, " {}={value}", field.name());
        }
    }
}

/// The layer that keeps the lines.
struct Recent;

impl<S> Layer<S> for Recent
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let mut found = Fields::default();
        attrs.record(&mut found);
        if let (Some(request), Some(span)) = (found.request, ctx.span(id)) {
            span.extensions_mut().insert(Tags {
                request,
                person: found.person,
            });
        }
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let mut found = Fields::default();
        values.record(&mut found);
        if let (Some(person), Some(span)) = (found.person, ctx.span(id))
            && let Some(tags) = span.extensions_mut().get_mut::<Tags>()
        {
            tags.person = Some(person);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let Some(scope) = ctx.event_scope(event) else {
            return;
        };
        let Some((request, person)) = scope.into_iter().find_map(|span| {
            span.extensions()
                .get::<Tags>()
                .map(|tags| (tags.request.clone(), tags.person.clone()))
        }) else {
            return;
        };
        let mut words = Words::default();
        event.record(&mut words);
        let meta = event.metadata();
        let text = format!(
            "{} {}: {}{}",
            meta.level(),
            meta.target(),
            words.message,
            words.rest
        );
        keep(Line {
            request,
            person,
            text: text.chars().take(LINE_CHARS).collect(),
        });
    }
}

/// The layer to add to the server's subscriber: it keeps the lines of the server's own code at
/// INFO and above, the request lines tower-http writes, and every other crate's warnings and
/// errors, whatever the printed log's own filter says.
#[must_use]
pub fn layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    Recent.with_filter(
        Targets::new()
            .with_target("spare_server", Level::INFO)
            .with_target("tower_http", Level::INFO)
            .with_default(Level::WARN),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt as _;

    #[test]
    fn a_person_gets_back_only_the_lines_of_their_own_requests() {
        let subscriber = tracing_subscriber::registry().with(layer());
        tracing::subscriber::with_default(subscriber, || {
            let mine = tracing::info_span!(
                "request",
                request = "r-mine",
                person = tracing::field::Empty
            );
            mine.in_scope(|| {
                // As the sign-in records it, once the session is read.
                Span::current().record("person", "anna");
                tracing::info!(counter = 3, "an edit was turned back");
            });
            let theirs = tracing::info_span!("request", request = "r-theirs", person = "ben");
            theirs.in_scope(|| tracing::warn!("something else"));
            tracing::info!("outside any request");
        });
        let asked = ["r-mine".to_owned(), "r-theirs".to_owned()];
        assert_eq!(
            lines_for("anna", &asked, 10),
            ["r-mine INFO spare_server::recent::tests: an edit was turned back counter=3"]
        );
        assert_eq!(lines_for("ben", &asked[..1], 10), Vec::<String>::new());
        assert_eq!(
            lines_for("ben", &asked, 10),
            ["r-theirs WARN spare_server::recent::tests: something else"]
        );
        assert_eq!(lines_for("anna", &asked, 0), Vec::<String>::new());
    }
}
