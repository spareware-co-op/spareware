//! The live channel. A browser subscribes to the models it has open, sends edits up, and gets
//! every ordered edit of those models down. After a dropped connection it sends the last log
//! number it saw, gets what it missed, and resends what wasn't confirmed; the orderer drops
//! repeats, so a restart of this server loses nothing and doubles nothing.

use crate::access::Target;
use crate::api::{ApiError, Shared};
use crate::auth::Me;
use crate::keys;
use crate::orderer::{Orderer, Reply, Submit, read_log};
use crate::rows::{ClientMark, Entry};
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;

/// How often a connection re-asks Cedar whether its person may still read what it's watching.
const RECHECK: Duration = Duration::from_secs(15);

pub async fn upgrade(
    State(st): State<Shared>,
    Me(person): Me,
    Path(project): Path<String>,
    headers: HeaderMap,
    socket: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    // A browser always sends Origin, so this stops another website opening a socket as its visitor.
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if origin.is_some_and(|o| o != st.config.public_url) {
        return Err(ApiError::Refused("Wrong site.".to_owned()));
    }
    st.require(&person, "project.open", Target::Project(&project))
        .await?;
    Ok(socket.on_upgrade(move |socket| async move {
        if let Err(e) = Live::new(st, person, project).await.run(socket).await {
            tracing::debug!("connection closed: {e:#}");
        }
    }))
}

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum FromBrowser {
    /// Starts or resumes: who this tab is, what it watches, and the last log number it saw.
    Hello {
        client: String,
        models: Vec<String>,
        since: u64,
    },
    /// Watches one more model, for a live link.
    Watch {
        model: String,
        since: u64,
    },
    Stop {
        model: String,
    },
    Op {
        counter: u64,
        base: u64,
        model: String,
        cmd: String,
        args: Value,
        author_ms: u64,
    },
}

struct Live {
    st: Shared,
    person: String,
    project: String,
    orderer: Arc<Orderer>,
    client: Option<String>,
    watching: HashSet<String>,
    /// The last log number looked at for this browser.
    cursor: u64,
}

impl Live {
    async fn new(st: Shared, person: String, project: String) -> Self {
        let orderer = st.orderers.get(&project).await;
        Self {
            st,
            person,
            project,
            orderer,
            client: None,
            watching: HashSet::new(),
            cursor: 0,
        }
    }

    async fn run(mut self, socket: WebSocket) -> Result<()> {
        let (mut sink, mut stream) = socket.split();
        let mut bus = self.orderer.subscribe();
        let (reply, mut replies) = mpsc::unbounded_channel();
        let mut recheck = tokio::time::interval(RECHECK);
        loop {
            let mut out: Vec<Value> = Vec::new();
            tokio::select! {
                message = stream.next() => {
                    let Some(message) = message else { return Ok(()) };
                    match message? {
                        Message::Text(text) => {
                            match serde_json::from_str(&text) {
                                Ok(request) => {
                                    self.handle(request, &reply, &mut out).await?;
                                }
                                Err(e) => out.push(json!({ "t": "error", "reason": e.to_string() })),
                            }
                        }
                        Message::Close(_) => return Ok(()),
                        _ => {}
                    }
                }
                entry = bus.recv() => {
                    match entry {
                        Ok(entry) => self.forward(&entry, &mut out),
                        Err(RecvError::Lagged(_)) => self.catch_up(&mut out).await?,
                        Err(RecvError::Closed) => return Ok(()),
                    }
                }
                done = replies.recv() => {
                    let Some(done) = done else { return Ok(()) };
                    out.push(match done {
                        Reply::Ack { counter, seq } => json!({ "t": "ack", "counter": counter, "seq": seq }),
                        Reply::Rejected { counter, reason } => {
                            json!({ "t": "rejected", "counter": counter, "reason": reason })
                        }
                        Reply::Retry { counter } => json!({ "t": "retry", "counter": counter }),
                    });
                }
                _ = recheck.tick() => self.recheck(&mut out).await?,
            }
            for message in out {
                sink.send(Message::Text(message.to_string().into())).await?;
            }
        }
    }

    async fn handle(
        &mut self,
        request: FromBrowser,
        reply: &mpsc::UnboundedSender<Reply>,
        out: &mut Vec<Value>,
    ) -> Result<()> {
        match request {
            FromBrowser::Hello {
                client,
                models,
                since,
            } => {
                let mark_key = keys::client(&self.project, &self.person, &client);
                let mark: Option<ClientMark> = self.st.kv.get(mark_key).await?;
                self.client = Some(client);
                for model in models {
                    self.watch(model, out).await?;
                }
                let head = self.head().await?;
                out.push(json!({
                    "t": "welcome",
                    "head": head,
                    "person": self.person,
                    "last_counter": mark.map_or(0, |m| m.counter),
                }));
                self.cursor = head;
                self.send_log(since, head, None, out).await?;
                out.push(json!({ "t": "caught_up", "head": head }));
            }
            FromBrowser::Watch { model, since } => {
                if self.watch(model.clone(), out).await? {
                    self.send_log(since, self.cursor, Some(&model), out).await?;
                }
            }
            FromBrowser::Stop { model } => {
                self.watching.remove(&model);
            }
            FromBrowser::Op {
                counter,
                base,
                model,
                cmd,
                args,
                author_ms,
            } => {
                let Some(client) = self.client.clone() else {
                    out.push(json!({ "t": "error", "reason": "Say hello first." }));
                    return Ok(());
                };
                let edit = Submit {
                    person: self.person.clone(),
                    client,
                    counter,
                    base,
                    model,
                    cmd,
                    args,
                    author_ms,
                    reply: reply.clone(),
                };
                self.orderer.submit(edit).await?;
            }
        }
        Ok(())
    }

    async fn watch(&mut self, model: String, out: &mut Vec<Value>) -> Result<bool> {
        let target = Target::Model(&self.project, &model);
        if self
            .st
            .access
            .allowed(&self.person, "model.read", target, None)
            .await?
        {
            self.watching.insert(model);
            Ok(true)
        } else {
            out.push(json!({ "t": "refused", "model": model }));
            Ok(false)
        }
    }

    async fn head(&self) -> Result<u64> {
        Ok(self
            .st
            .kv
            .get::<u64>(keys::head(&self.project))
            .await?
            .unwrap_or(0))
    }

    fn visible(&self, entry: &Entry) -> bool {
        let mine = self.client.as_deref() == Some(entry.client.as_str());
        self.watching.contains(&entry.model) && (entry.failed.is_none() || mine)
    }

    fn forward(&mut self, entry: &Entry, out: &mut Vec<Value>) {
        if entry.seq > self.cursor {
            self.cursor = entry.seq;
            if self.visible(entry) {
                out.push(json!({ "t": "entry", "entry": entry }));
            }
        }
    }

    async fn catch_up(&mut self, out: &mut Vec<Value>) -> Result<()> {
        let head = self.head().await?;
        let from = self.cursor;
        self.cursor = head;
        self.send_log(from, head, None, out).await
    }

    async fn send_log(
        &self,
        from: u64,
        to: u64,
        only: Option<&str>,
        out: &mut Vec<Value>,
    ) -> Result<()> {
        let mut snap = self.st.kv.snapshot().await?;
        for entry in read_log(&mut snap, &self.project, from, to).await? {
            if self.visible(&entry) && only.is_none_or(|m| m == entry.model) {
                out.push(json!({ "t": "entry", "entry": entry }));
            }
        }
        Ok(())
    }

    /// Someone taken off a team mid-session stops receiving that team's edits.
    async fn recheck(&mut self, out: &mut Vec<Value>) -> Result<()> {
        for model in self.watching.clone() {
            let target = Target::Model(&self.project, &model);
            if !self
                .st
                .access
                .allowed(&self.person, "model.read", target, None)
                .await?
            {
                self.watching.remove(&model);
                out.push(json!({ "t": "refused", "model": model }));
            }
        }
        Ok(())
    }
}
