//! The socket behind the live channel. The rules are in `kit_sync::live` and `Service`; this file
//! only moves messages.

use crate::http::{ApiError, Me, Shared, id};
use crate::service::Refusal;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use kit_core::Id;
use kit_ops::Edit;
use kit_store::StoreError;
use kit_sync::live::{FromBrowser, Watcher, entry_message, reply_message};
use kit_sync::{Orderer, Submit};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

/// How often a connection re-asks Cedar whether its person may still read what it's watching.
const RECHECK: Duration = Duration::from_secs(15);

pub async fn upgrade(
    State(app): State<Shared>,
    Me(person): Me,
    Path(project): Path<String>,
    headers: HeaderMap,
    socket: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    // A browser always sends Origin, so this stops another website opening a socket as its visitor.
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if origin.is_some_and(|o| o != app.settings.public_url) {
        return Err(ApiError(Refusal::Refused("Wrong site.".to_owned())));
    }
    let project = id(&project)?;
    app.service.may_open_project(&person, &project).await?;
    Ok(socket.on_upgrade(move |socket| async move {
        let orderer = app.service.orderer(&project).await;
        let live = Live {
            app,
            person,
            project,
            orderer,
            watcher: Watcher::new(),
        };
        if let Err(e) = live.run(socket).await {
            tracing::debug!("connection closed: {e}");
        }
    }))
}

struct Live {
    app: Shared,
    person: Id,
    project: Id,
    orderer: Arc<Orderer>,
    watcher: Watcher,
}

fn gone<E: std::fmt::Display>(e: E) -> StoreError {
    StoreError::Unavailable(e.to_string())
}

/// A connection whose person has lost the project ends there.
fn refused(refusal: Refusal) -> StoreError {
    match refusal {
        Refusal::Failed(e) => e,
        other => StoreError::Unavailable(other.to_string()),
    }
}

impl Live {
    async fn run(mut self, socket: WebSocket) -> Result<(), StoreError> {
        let (mut sink, mut stream) = socket.split();
        let mut bus = self.orderer.subscribe();
        let (reply, mut replies) = mpsc::unbounded_channel();
        let mut recheck = tokio::time::interval(RECHECK);
        loop {
            let mut out: Vec<Value> = Vec::new();
            tokio::select! {
                message = stream.next() => {
                    let Some(message) = message else { return Ok(()) };
                    match message.map_err(gone)? {
                        Message::Text(text) => match serde_json::from_str(&text) {
                            Ok(request) => self.handle(request, &reply, &mut out).await?,
                            Err(e) => out.push(json!({ "t": "error", "reason": e.to_string() })),
                        },
                        Message::Close(_) => return Ok(()),
                        _ => {}
                    }
                }
                entry = bus.recv() => match entry {
                    Ok(entry) => out.extend(self.watcher.next(&entry)),
                    Err(RecvError::Lagged(_)) => self.catch_up(&mut out).await?,
                    Err(RecvError::Closed) => return Ok(()),
                },
                done = replies.recv() => {
                    let Some(done) = done else { return Ok(()) };
                    out.push(reply_message(&done));
                }
                _ = recheck.tick() => self.recheck(&mut out).await?,
            }
            for message in out {
                sink.send(Message::Text(message.to_string().into()))
                    .await
                    .map_err(gone)?;
            }
        }
    }

    async fn handle(
        &mut self,
        request: FromBrowser,
        reply: &mpsc::UnboundedSender<kit_sync::Reply>,
        out: &mut Vec<Value>,
    ) -> Result<(), StoreError> {
        match request {
            FromBrowser::Hello {
                client,
                models,
                since,
            } => {
                let Ok(client) = Id::parse(&client) else {
                    out.push(json!({ "t": "error", "reason": "That isn't a client ID." }));
                    return Ok(());
                };
                let service = &self.app.service;
                let last_counter = service
                    .last_counter(&self.person, &self.project, &client)
                    .await?;
                self.watcher.set_client(client);
                for model in models {
                    self.watch(&model, out).await?;
                }
                let head = self.head().await?;
                out.push(json!({
                    "t": "welcome",
                    "head": head,
                    "person": self.person.as_str(),
                    "last_counter": last_counter,
                }));
                self.watcher.set_cursor(head);
                self.send_log(since, head, None, out).await?;
                out.push(json!({ "t": "caught_up", "head": head }));
            }
            FromBrowser::Watch { model, since } => {
                if let Some(model) = self.watch(&model, out).await? {
                    self.send_log(since, self.watcher.cursor(), Some(&model), out)
                        .await?;
                }
            }
            FromBrowser::Stop { model } => {
                if let Ok(model) = Id::parse(&model) {
                    self.watcher.stop(&model);
                }
            }
            FromBrowser::Op {
                counter,
                base,
                model,
                cmd,
                args,
                author_ms,
            } => {
                let (Some(client), Ok(target)) = (self.watcher.client(), Id::parse(&model)) else {
                    out.push(json!({ "t": "error", "reason": "Say hello first." }));
                    return Ok(());
                };
                let edit = Edit {
                    person: self.person.clone(),
                    client: client.clone(),
                    counter,
                    base,
                    target,
                    cmd,
                    args,
                    author_ms,
                };
                self.orderer
                    .submit(Submit {
                        edit,
                        reply: reply.clone(),
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn watch(&mut self, model: &str, out: &mut Vec<Value>) -> Result<Option<Id>, StoreError> {
        let refused = |out: &mut Vec<Value>| out.push(json!({ "t": "refused", "model": model }));
        let Ok(id) = Id::parse(model) else {
            refused(out);
            return Ok(None);
        };
        if self
            .app
            .service
            .may_watch(&self.person, &self.project, &id)
            .await?
        {
            self.watcher.watch(id.clone());
            Ok(Some(id))
        } else {
            refused(out);
            Ok(None)
        }
    }

    async fn head(&self) -> Result<u64, StoreError> {
        self.app
            .service
            .head(&self.person, &self.project)
            .await
            .map_err(refused)
    }

    async fn catch_up(&mut self, out: &mut Vec<Value>) -> Result<(), StoreError> {
        let head = self.head().await?;
        let from = self.watcher.cursor();
        self.watcher.set_cursor(head);
        self.send_log(from, head, None, out).await
    }

    async fn send_log(
        &self,
        from: u64,
        to: u64,
        only: Option<&Id>,
        out: &mut Vec<Value>,
    ) -> Result<(), StoreError> {
        let entries = self
            .app
            .service
            .entries(&self.person, &self.project, from, to)
            .await;
        for entry in entries.map_err(refused)? {
            if self.watcher.sees(&entry) && only.is_none_or(|model| *model == entry.target) {
                out.push(entry_message(&entry));
            }
        }
        Ok(())
    }

    /// Someone taken off a team mid-session stops receiving that team's edits.
    async fn recheck(&mut self, out: &mut Vec<Value>) -> Result<(), StoreError> {
        for model in self.watcher.watched() {
            if !self
                .app
                .service
                .may_watch(&self.person, &self.project, &model)
                .await?
            {
                self.watcher.stop(&model);
                out.push(json!({ "t": "refused", "model": model.as_str() }));
            }
        }
        Ok(())
    }
}
