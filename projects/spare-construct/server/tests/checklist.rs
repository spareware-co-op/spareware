//! The done checklist, run against the real stack: TiKV, RustFS, the real server program, and
//! OpenID Connect sign-in through examples/mock_oidc.rs. It starts and kills servers itself.
//! Run it with scripts/run-checklist.sh, which wipes the local test data first.

use futures::{SinkExt, StreamExt};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::{Client, StatusCode, Url};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

const BIN: &str = env!("CARGO_BIN_EXE_spare-server");
const FIRST: &str = "127.0.0.1:8080";
const SECOND: &str = "127.0.0.1:8081";

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn pass(label: &str) {
    println!("PASS  {label}");
}

fn check(ok: bool, label: &str) {
    assert!(ok, "FAILED: {label}");
    pass(label);
}

/// One running copy of the server.
struct Server {
    child: Child,
    bind: &'static str,
}

impl Server {
    async fn start(bind: &'static str) -> Self {
        let port = bind.rsplit(':').next().unwrap();
        let mut child = Starting(Some(
            Command::new(BIN)
                .arg("serve")
                .env("STACK_BIND", bind)
                .env("STACK_PUBLIC_URL", format!("http://localhost:{port}"))
                .stdout(Stdio::null())
                .spawn()
                .expect("starting the server"),
        ));
        let health = format!("http://{bind}/health");
        // A minute, because a server starting just after a TiKV died waits for new Raft leaders.
        for _ in 0..1200 {
            if reqwest::get(&health)
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                let child = child.0.take().expect("still held");
                return Self { child, bind };
            }
            sleep(Duration::from_millis(50)).await;
        }
        panic!("the server did not start");
    }

    /// Stops it the hard way: no warning, no tidying up.
    fn kill(&mut self) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
    }

    fn base(&self) -> String {
        format!("http://localhost:{}", self.bind.rsplit(':').next().unwrap())
    }
}

/// Kills what a failed start left, so a test that panics leaves no server behind.
struct Starting(Option<Child>);

impl Drop for Starting {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One person in one browser.
struct Browser {
    http: Client,
    jar: Arc<Jar>,
}

impl Browser {
    fn new() -> Self {
        let jar = Arc::new(Jar::default());
        let http = Client::builder()
            .cookie_provider(jar.clone())
            .build()
            .unwrap();
        Self { http, jar }
    }

    async fn sign_in(base: &str, person: &str) -> Self {
        let browser = Self::new();
        let url = format!("{base}/auth/login?hint={person}@example.test");
        let response = browser.http.get(url).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "{person} should sign in");
        browser
    }

    async fn get(&self, url: String) -> (StatusCode, Value) {
        let response = self.http.get(url).send().await.unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    async fn post(&self, url: String, body: Value) -> (StatusCode, Value) {
        let response = self.http.post(url).json(&body).send().await.unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    fn cookie(&self, base: &str) -> String {
        let url = Url::parse(base).unwrap();
        self.jar.cookies(&url).unwrap().to_str().unwrap().to_owned()
    }
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// One browser tab's live channel.
struct Live {
    socket: Socket,
    client: String,
    counter: u64,
    /// The log entries it was sent on the way in.
    replayed: Vec<Value>,
    /// What a wait read past, so a later wait can still find it: an entry can arrive while the
    /// test waits for an earlier edit's reply (issue #498, on a slow runner).
    seen: Vec<Value>,
}

impl Live {
    async fn open(
        browser: &Browser,
        base: &str,
        project: &str,
        client: &str,
        documents: &[&str],
        since: u64,
    ) -> Result<(Self, Value), String> {
        let ws = base.replace("http://", "ws://");
        let mut request = format!("{ws}/ws/{project}").into_client_request().unwrap();
        request
            .headers_mut()
            .insert("cookie", browser.cookie(base).parse().unwrap());
        request
            .headers_mut()
            .insert("origin", base.parse().unwrap());
        let (socket, _) = connect_async(request).await.map_err(|e| e.to_string())?;
        let mut live = Self {
            socket,
            client: client.to_owned(),
            counter: 0,
            replayed: Vec::new(),
            seen: Vec::new(),
        };
        let hello =
            json!({ "t": "hello", "client": client, "documents": documents, "since": since });
        live.send(hello).await.map_err(|e| e.to_string())?;
        let welcome = live
            .until(|m| m["t"] == "welcome")
            .await
            .ok_or("no welcome")?;
        loop {
            let message = live.next().await.ok_or("no catch-up")?;
            match message["t"].as_str() {
                Some("caught_up") => return Ok((live, welcome)),
                Some("entry") => live.replayed.push(message),
                _ => {}
            }
        }
    }

    async fn send(&mut self, message: Value) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        self.socket.send(Message::text(message.to_string())).await
    }

    fn edit(&mut self, document: &str, cmd: &str, args: Value) -> Value {
        self.counter += 1;
        json!({
            "t": "op", "counter": self.counter, "base": 0, "document": document, "cmd": cmd,
            "args": args, "author_ms": now_ms(),
        })
    }

    async fn op(&mut self, model: &str, cmd: &str, args: Value) -> u64 {
        let message = self.edit(model, cmd, args);
        self.send(message).await.unwrap();
        self.counter
    }

    async fn next(&mut self) -> Option<Value> {
        loop {
            match timeout(Duration::from_secs(40), self.socket.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => return serde_json::from_str(&text).ok(),
                Ok(Some(Ok(_))) => {}
                _ => return None,
            }
        }
    }

    async fn until(&mut self, wanted: impl Fn(&Value) -> bool) -> Option<Value> {
        loop {
            let message = self.next().await?;
            if wanted(&message) {
                return Some(message);
            }
            self.seen.push(message);
        }
    }

    /// The first message `wanted` accepts, among those a wait read past or still to come.
    async fn find(&mut self, wanted: impl Fn(&Value) -> bool) -> Option<Value> {
        if let Some(i) = self.seen.iter().position(&wanted) {
            return Some(self.seen.remove(i));
        }
        self.until(wanted).await
    }

    async fn reply_to(&mut self, counter: u64) -> Value {
        let wanted = |m: &Value| {
            (m["t"] == "ack" || m["t"] == "rejected") && m["counter"].as_u64() == Some(counter)
        };
        self.until(wanted).await.expect("a reply to the edit")
    }
}

/// An element's new place, as an edit gives it.
fn at(element: &str, x: i64, y: i64) -> Value {
    json!({ "element": element, "placement": { "x": x, "y": y, "z": 0 } })
}

/// The log entries people made after a number. The server's own checkpoints go in the same log.
async fn edits_since(browser: &Browser, base: &str, document: &str, from: u64) -> u64 {
    let tab = format!("count-{}", now_ms());
    let (live, _) = Live::open(browser, base, "example-1", &tab, &[document], from)
        .await
        .unwrap();
    live.replayed
        .iter()
        .filter(|m| m["entry"]["author"] != "system")
        .count() as u64
}

/// The pieces in a view, once regeneration has reached a log number.
async fn settled_area(browser: &Browser, url: String, head: u64) -> Value {
    for _ in 0..1000 {
        let (_, view) = browser.get(url.clone()).await;
        if view["through"].as_u64().unwrap_or(0) >= head {
            return view;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("regeneration never reached {head}");
}

fn percentile(sorted: &[u64], percent: usize) -> u64 {
    sorted[(sorted.len() - 1) * percent / 100]
}

/// Sends edits from one tab and times how long each takes to reach the other.
async fn measure(
    from: &mut Live,
    to: &mut Live,
    model: &str,
    element: &str,
    count: u64,
) -> Vec<u64> {
    let mut times = Vec::new();
    for step in 0..count {
        let args = at(element, 2000 + step as i64 * 50, 1000);
        let counter = from.op(model, "element.move", args).await;
        let client = from.client.clone();
        let wanted = |m: &Value| {
            m["t"] == "entry"
                && m["entry"]["client"] == client.as_str()
                && m["entry"]["counter"].as_u64() == Some(counter)
        };
        let entry = to.until(wanted).await.expect("the edit should arrive");
        let sent = entry["entry"]["author_ms"].as_u64().unwrap();
        times.push(now_ms().saturating_sub(sent));
        from.reply_to(counter).await;
        sleep(Duration::from_millis(10)).await;
    }
    times.sort_unstable();
    times
}

fn report(label: &str, times: &[u64]) {
    let (median, p95, worst) = (
        percentile(times, 50),
        percentile(times, 95),
        *times.last().unwrap(),
    );
    println!(
        "      {label}: {} edits, median {median} ms, 95% {p95} ms, worst {worst} ms",
        times.len()
    );
    check(
        worst < 200,
        &format!("{label}: every edit arrived in under 200 ms"),
    );
}

fn folder_kinds(project: &Value, team: &str) -> Vec<String> {
    let teams = project["teams"].as_array().unwrap();
    let team = teams.iter().find(|t| t["id"] == team).expect("team listed");
    team["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["kind"].as_str().unwrap().to_owned())
        .collect()
}

fn packages(project: &Value, team: &str, kind: &str) -> Vec<Value> {
    let teams = project["teams"].as_array().unwrap();
    let team = teams.iter().find(|t| t["id"] == team).expect("team listed");
    let folders = team["folders"].as_array().unwrap();
    let folder = folders
        .iter()
        .find(|f| f["kind"] == kind)
        .expect("folder listed");
    folder["packages"].as_array().unwrap().clone()
}

fn stack(arguments: &[&str]) -> String {
    let output = Command::new(BIN)
        .args(arguments)
        .output()
        .expect("running spare-server");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "spare-server {arguments:?}: {text}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    text
}

fn script(name: &str) {
    let path = format!("{}/scripts/{name}", env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("bash")
        .arg(path)
        .status()
        .expect("running a script");
    assert!(status.success(), "{name} failed");
}

async fn one_signs_in_and_lands_in_their_hub(base: &str) {
    println!("\n1. Anna logs in and lands in Company 1");
    let anna = Browser::sign_in(base, "anna").await;
    let (_, me) = anna.get(format!("{base}/api/me")).await;
    check(
        me["person"]["name"] == "Anna",
        "the signed-in person is Anna",
    );
    check(
        me["person"]["home_hub"] == "company-1",
        "her home hub is Company 1",
    );
    check(
        me["person"].get("email").is_none(),
        "no email address is kept",
    );
    check(
        me["hubs"][0]["name"] == "Company 1",
        "Company 1 comes first",
    );
    check(
        me["hubs"].as_array().unwrap().len() == 1,
        "she has no other hub",
    );

    let stranger = Browser::new();
    let url = format!("{base}/auth/login?hint=stranger@example.test");
    let status = stranger.http.get(url).send().await.unwrap().status();
    check(
        status == StatusCode::FORBIDDEN,
        "someone not on the server is refused",
    );
    let (status, _) = stranger.get(format!("{base}/api/me")).await;
    check(
        status == StatusCode::UNAUTHORIZED,
        "nothing opens without signing in",
    );
}

async fn two_switches_hubs(base: &str) {
    println!("\n2. Dan switches between Company 2 and Company 1");
    let dan = Browser::sign_in(base, "dan").await;
    let (_, me) = dan.get(format!("{base}/api/me")).await;
    let names: Vec<_> = me["hubs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["name"].clone())
        .collect();
    check(
        names == [json!("Company 2"), json!("Company 1")],
        "he has both hubs, home first",
    );
    check(
        me["hubs"][1]["role"] == "external",
        "he is external in Company 1",
    );

    let (_, home) = dan.get(format!("{base}/api/hubs/company-2")).await;
    check(
        home["projects"][0]["name"] == "Example model 1 (bridged)",
        "Company 2 shows his project",
    );
    let (_, away) = dan.get(format!("{base}/api/hubs/company-1")).await;
    let projects = away["projects"].as_array().unwrap();
    check(
        projects.len() == 1 && projects[0]["name"] == "Example model 1",
        "Company 1 shows only the project he's on",
    );
    check(
        away["externals"] == json!(["Dan", "Eve"]),
        "Company 1 lists its external users",
    );

    let anna = Browser::sign_in(base, "anna").await;
    let (status, _) = anna.get(format!("{base}/api/hubs/company-2")).await;
    check(status == StatusCode::FORBIDDEN, "Anna can't open Company 2");
    let (status, _) = dan.get(format!("{base}/api/projects/riverside")).await;
    check(
        status == StatusCode::FORBIDDEN,
        "Dan can't open Riverside Flats",
    );
}

async fn three_live_edits(first: &str, second: &str) {
    println!("\n3. Two browsers each see the other's edit in under 200 ms");
    let anna = Browser::sign_in(first, "anna").await;
    let ben = Browser::sign_in(first, "ben").await;
    let model = "arch-model";
    let (mut a, _) = Live::open(&anna, first, "example-1", "tab-a", &[model], 0)
        .await
        .unwrap();
    let (mut b, _) = Live::open(&ben, first, "example-1", "tab-b", &[model], 0)
        .await
        .unwrap();

    let there = measure(&mut a, &mut b, model, "D-101", 200).await;
    report("Anna to Ben", &there);
    let back = measure(&mut b, &mut a, model, "D-103", 200).await;
    report("Ben to Anna", &back);

    let (mut a2, _) = Live::open(&anna, second, "example-1", "tab-a2", &[model], 0)
        .await
        .unwrap();
    let across = measure(&mut b, &mut a2, model, "D-103", 100).await;
    println!(
        "      across two servers: {} edits, median {} ms, 95% {} ms, worst {} ms",
        across.len(),
        percentile(&across, 50),
        percentile(&across, 95),
        across.last().unwrap(),
    );
    check(
        percentile(&across, 95) < 200,
        "across two servers, 95% of edits arrive in under 200 ms",
    );

    println!("\n   Automatic claims");
    let c = a
        .op(model, "element.claim", json!({ "element": "W-101" }))
        .await;
    check(
        a.reply_to(c).await["t"] == "ack",
        "Anna claims a wall by starting to work on it",
    );
    let c = b.op(model, "element.move", at("W-101", 1, 1)).await;
    let reply = b.reply_to(c).await;
    check(
        reply["t"] == "rejected",
        "Ben's edit of that wall is turned back",
    );
    check(
        reply["reason"] == "Anna is working on this.",
        "and he is told why",
    );
    let c = a
        .op(model, "element.release", json!({ "element": "W-101" }))
        .await;
    a.reply_to(c).await;
    let c = b.op(model, "element.move", at("W-101", 0, 0)).await;
    check(
        b.reply_to(c).await["t"] == "ack",
        "once she lets go, Ben can take it",
    );
    let c = b
        .op(model, "element.release", json!({ "element": "W-101" }))
        .await;
    b.reply_to(c).await;
}

/// An edit's inputs go in the log, the server builds the shape from them, and says what it built.
async fn shapes_follow_the_log(base: &str) {
    println!("\n   Shapes are rebuilt from recipes, and checked");
    let anna = Browser::sign_in(base, "anna").await;
    let dan = Browser::sign_in(base, "dan").await;
    let model = "arch-model";
    let url = format!("{base}/api/projects/example-1/models/{model}");
    let (mut live, _) = Live::open(&anna, base, "example-1", "tab-shapes", &[model], 0)
        .await
        .unwrap();
    let recipe = json!({ "owner": "construct", "schema": 1, "op": "wall.straight", "op_version": 1,
        "args": { "length": 2400, "thickness": 150, "height": 2700 } });
    let create = json!({ "element": "W-900", "kind": "wall", "level": 1, "recipe": recipe,
        "placement": { "x": 3000, "y": 3000, "z": 0 } });
    let c = live.op(model, "element.create", create).await;
    check(
        live.reply_to(c).await["t"] == "ack",
        "a wall goes in as a recipe",
    );
    let entry = live
        .until(|m| {
            m["t"] == "entry"
                && m["entry"]["cmd"] == "document.checkpoint"
                && m["entry"]["args"]["bodies"].get("W-900").is_some()
        })
        .await
        .expect("a checkpoint naming the wall");
    let body = entry["entry"]["args"]["bodies"]["W-900"].clone();
    check(
        entry["entry"]["author"] == "system",
        "the server says what it built",
    );

    let (status, shape) = anna.get(format!("{url}/shape/W-900?lod=0")).await;
    check(
        status == StatusCode::OK && shape["built"]["result"]["shape"]["body"] == body,
        "and the stored shape is that one",
    );
    check(
        shape["display"]["mesh"]["indices"]
            .as_array()
            .is_some_and(|i| !i.is_empty()),
        "with a mesh to draw",
    );
    let (status, _) = dan.get(format!("{url}/shape/W-900")).await;
    check(status == StatusCode::FORBIDDEN, "Dan can't read it");

    let c = live
        .op(model, "element.move", at("W-900", 3100, 3000))
        .await;
    let moved = live.reply_to(c).await["seq"]
        .as_u64()
        .expect("the move's place in the log");
    let c = live
        .op(model, "element.release", json!({ "element": "W-900" }))
        .await;
    live.reply_to(c).await;
    // The checkpoint can come before the release's reply, so look at what that wait read past
    // too; it must reach the move, so an earlier checkpoint naming the wall can't stand in.
    let entry = live
        .find(|m| {
            m["t"] == "entry"
                && m["entry"]["cmd"] == "document.checkpoint"
                && m["entry"]["args"]["through"].as_u64() >= Some(moved)
                && m["entry"]["args"]["bodies"].get("W-900").is_some()
        })
        .await
        .expect("a checkpoint after the move");
    check(
        entry["entry"]["args"]["bodies"]["W-900"] == body,
        "moving it keeps the same body",
    );

    let c = live
        .op(model, "element.delete", json!({ "element": "W-900" }))
        .await;
    live.reply_to(c).await;
}

/// One tab editing one wall as fast as its edits are confirmed, as a busy person's browser does.
async fn busy_tab(browser: &Browser, base: &str, tab: &str, element: &str, edits: u64) -> Vec<u64> {
    let model = "arch-model";
    let (mut live, _) = Live::open(browser, base, "example-1", tab, &[], 0)
        .await
        .unwrap();
    let mut times = Vec::new();
    for step in 0..edits {
        let args = at(element, step as i64, 0);
        let message = live.edit(model, "element.move", args);
        let counter = live.counter;
        let sent = Instant::now();
        loop {
            live.send(message.clone()).await.unwrap();
            let wanted = |m: &Value| {
                ["ack", "rejected", "retry"].contains(&m["t"].as_str().unwrap_or(""))
                    && m["counter"].as_u64() == Some(counter)
            };
            let reply = live.until(wanted).await.expect("a reply");
            if reply["t"] == "ack" {
                break;
            }
            assert_eq!(reply["t"], "retry", "{reply}");
        }
        times.push(sent.elapsed().as_millis() as u64);
    }
    let c = live
        .op(model, "element.release", json!({ "element": element }))
        .await;
    live.reply_to(c).await;
    times
}

/// The reason for TiKV: many people writing to one project at once, through two servers.
async fn many_writers(first: &str, second: &str) {
    println!("\n   Many writers at once");
    const EDITS: u64 = 200;
    let anna = Browser::sign_in(first, "anna").await;
    let ben = Browser::sign_in(second, "ben").await;
    let project = format!("{first}/api/projects/example-1");
    let (_, before) = anna.get(project.clone()).await;
    let started = Instant::now();
    let results = futures::future::join_all([
        busy_tab(&anna, first, "busy-1", "W-102", EDITS),
        busy_tab(&anna, first, "busy-2", "W-103", EDITS),
        busy_tab(&anna, first, "busy-3", "W-104", EDITS),
        busy_tab(&anna, first, "busy-4", "W-105", EDITS),
        busy_tab(&ben, second, "busy-5", "W-201", EDITS),
        busy_tab(&ben, second, "busy-6", "W-202", EDITS),
        busy_tab(&ben, second, "busy-7", "W-203", EDITS),
        busy_tab(&ben, second, "busy-8", "W-204", EDITS),
    ])
    .await;
    let seconds = started.elapsed().as_secs_f64();
    let mut times: Vec<u64> = results.into_iter().flatten().collect();
    times.sort_unstable();
    let added = edits_since(&anna, first, "arch-model", before["head"].as_u64().unwrap()).await;
    println!(
        "      8 tabs on 2 servers, one project: {} edits/s, confirmed in median {} ms, 95% {} ms, worst {} ms",
        (times.len() as f64 / seconds).round(),
        percentile(&times, 50),
        percentile(&times, 95),
        times.last().unwrap(),
    );
    // Each tab also lets go of its wall at the end.
    check(
        added == 8 * EDITS + 8,
        "every edit from every tab went into one unbroken log",
    );
    stack(&["verify"]);
    pass("and the log replays to exactly the stored model");
}

async fn four_access_and_bridges(base: &str) {
    println!("\n4. WIP stays private; Arch v12 is shared, bridged and consumed");
    let anna = Browser::sign_in(base, "anna").await;
    let dan = Browser::sign_in(base, "dan").await;
    let cara = Browser::sign_in(base, "cara").await;
    let project = format!("{base}/api/projects/example-1");
    let model = format!("{project}/models/arch-model");

    let (_, seen) = dan.get(project.clone()).await;
    check(
        folder_kinds(&seen, "arch") == ["shared", "consumed"],
        "Dan sees the Architecture team's Shared and Consumed, not its WIP",
    );
    let (status, _) = dan
        .get(format!("{model}/area?level=1&x0=0&y0=0&x1=9000&y1=9000"))
        .await;
    check(
        status == StatusCode::FORBIDDEN,
        "he can't load the WIP model's pieces",
    );
    let (_, pieces) = anna
        .get(format!("{model}/area?level=1&x0=0&y0=0&x1=9000&y1=9000"))
        .await;
    let hash = pieces["pieces"]["W-101"].as_str().unwrap();
    let (status, _) = dan.get(format!("{model}/record/{hash}")).await;
    check(
        status == StatusCode::FORBIDDEN,
        "nor a record, even knowing its hash",
    );
    let (mut live, _) = Live::open(&dan, base, "example-1", "tab-dan", &[], 0)
        .await
        .unwrap();
    live.send(json!({ "t": "watch", "document": "arch-model", "since": 0 }))
        .await
        .unwrap();
    let answer = live
        .until(|m| m["t"] == "refused" || m["t"] == "entry")
        .await
        .unwrap();
    check(answer["t"] == "refused", "he can't watch its live edits");
    let c = live
        .op("arch-model", "element.move", at("W-101", 5, 5))
        .await;
    check(
        live.reply_to(c).await["t"] == "rejected",
        "his edit of it is turned back",
    );
    let (status, _) = cara.get(project.clone()).await;
    check(
        status == StatusCode::FORBIDDEN,
        "Cara, not on this project, can't open it",
    );

    let url = format!("{project}/teams/arch/publish");
    let (status, _) = dan
        .post(url.clone(), json!({ "model": "arch-model" }))
        .await;
    check(
        status == StatusCode::FORBIDDEN,
        "Dan can't publish the Architecture team's model",
    );
    let (status, package) = anna.post(url, json!({ "model": "arch-model" })).await;
    check(
        package["entry"].as_u64().unwrap_or(0) > 0,
        "sharing is an entry in the project's log",
    );
    check(status == StatusCode::OK, "Anna shares the Arch model");
    check(package["name"] == "Arch package v12", "as Arch package v12");
    let (_, seen) = dan.get(project.clone()).await;
    let shared = packages(&seen, "arch", "shared");
    check(
        shared.iter().any(|p| p["name"] == "Arch package v12"),
        "Dan sees it in Shared",
    );

    let bridge = format!("{project}/teams/arch/packages/arch-package-v12/bridge");
    let body = json!({ "to_project": "riverside-bridged", "to_team": "landscape" });
    let (status, _) = anna.post(bridge.clone(), body).await;
    check(
        status == StatusCode::FORBIDDEN,
        "a bridge to Company 3 is refused: only one hub permits it",
    );
    let body = json!({ "to_project": "example-1-bridged", "to_team": "structure" });
    let (status, _) = dan.post(bridge.clone(), body.clone()).await;
    check(
        status == StatusCode::FORBIDDEN,
        "Dan can't bridge someone else's package",
    );
    let (status, _) = anna.post(bridge, body).await;
    check(
        status == StatusCode::OK,
        "Anna bridges it to Company 2: both hubs permit it",
    );

    let dan_project = format!("{base}/api/projects/example-1-bridged");
    let (_, seen) = dan.get(dan_project.clone()).await;
    let waiting = seen["incoming"].as_array().unwrap();
    check(waiting.len() == 1, "it waits at the Structure team's door");
    let incoming = waiting[0]["id"].as_str().unwrap();
    let body = json!({ "incoming": incoming });
    let consume = format!("{dan_project}/teams/structure/consume");
    let (status, _) = anna.post(consume.clone(), body.clone()).await;
    check(
        status == StatusCode::FORBIDDEN,
        "Anna can't take it in for them",
    );
    let (status, taken) = dan.post(consume, body).await;
    check(status == StatusCode::OK, "Dan takes it into Consumed");
    let (_, seen) = dan.get(dan_project.clone()).await;
    let consumed = packages(&seen, "structure", "consumed");
    check(
        consumed[0]["name"] == "Arch package v12" && consumed[0]["from"]["hub_name"] == "Company 1",
        "Consumed shows Arch package v12 from Company 1",
    );
    let item = taken["id"].as_str().unwrap();
    let file = format!("{dan_project}/teams/structure/folders/consumed/packages/{item}/file");
    let (status, file) = dan.get(file).await;
    check(
        status == StatusCode::OK && file["elements"].as_array().unwrap().len() == 17,
        "Dan reads all 17 elements of it",
    );

    println!("\n   The three kinds of link");
    let links = format!("{dan_project}/models/structure-model/links");
    let (status, _) = dan
        .post(links, json!({ "kind": "consumed", "source": item }))
        .await;
    check(
        status == StatusCode::OK,
        "Dan links v12 into the Structure model from Consumed",
    );
    let (_, links) = anna.get(format!("{model}/links")).await;
    let kinds: Vec<_> = links
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["kind"].clone())
        .collect();
    for kind in ["live", "shared", "consumed"] {
        check(
            kinds.contains(&json!(kind)),
            &format!("the Arch model has a {kind} link"),
        );
    }
    let url = format!("{project}/models/interiors-model/links");
    let (status, _) = dan
        .post(
            url.clone(),
            json!({ "kind": "live", "source": "arch-model" }),
        )
        .await;
    check(
        status == StatusCode::FORBIDDEN,
        "someone outside the team can't make a live link",
    );
    let (status, _) = anna
        .post(
            format!("{project}/models/site-setout/links"),
            json!({ "kind": "live", "source": "arch-model" }),
        )
        .await;
    check(
        status == StatusCode::FORBIDDEN,
        "a live link can't cross teams",
    );

    let (mut watcher, _) = Live::open(
        &anna,
        base,
        "example-1",
        "tab-watch",
        &["arch-model", "interiors-model"],
        0,
    )
    .await
    .unwrap();
    let ben = Browser::sign_in(base, "ben").await;
    let (mut editor, _) = Live::open(&ben, base, "example-1", "tab-int", &["interiors-model"], 0)
        .await
        .unwrap();
    let c = editor
        .op("interiors-model", "element.move", at("B-101", 800, 800))
        .await;
    let got = watcher
        .until(|m| m["t"] == "entry" && m["entry"]["target"] == "interiors-model")
        .await;
    check(got.is_some(), "a live link updates as they edit");
    editor.reply_to(c).await;
    let c = editor
        .op(
            "interiors-model",
            "element.release",
            json!({ "element": "B-101" }),
        )
        .await;
    editor.reply_to(c).await;

    let (_, seen) = anna.get(project).await;
    let sha = packages(&seen, "arch", "shared")
        .iter()
        .find(|p| p["id"] == "arch-package-v12")
        .unwrap()["sha256"]
        .clone();
    check(
        sha == consumed[0]["sha256"],
        "what was consumed is exactly what was shared",
    );
}

/// Moves every Raft leader onto the TiKV node about to die. Left to chance, a run can lose a node
/// leading nothing and prove nothing: that's how `tikv-client` 0.4.0, which can't get past a dead
/// leader, passed this check until the day the leaders happened to be there.
async fn leaders_onto_the_node_that_dies() {
    let pd = "http://127.0.0.1:2379/pd/api/v1";
    let http = Client::new();
    let read = |url: String| {
        let http = http.clone();
        async move {
            let reply = http.get(url).send().await.unwrap();
            reply.json::<Value>().await.unwrap()
        }
    };
    let stores = read(format!("{pd}/stores")).await;
    let dying = stores["stores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["store"]["address"] == "127.0.0.1:20162")
        .expect("the third TiKV");
    let store = dying["store"]["id"].as_u64().unwrap();
    let grant = json!({ "name": "grant-leader-scheduler", "store_id": store });
    let schedulers = format!("{pd}/schedulers");
    http.post(&schedulers).json(&grant).send().await.unwrap();
    let mut gathered = false;
    for _ in 0..600 {
        let status = read(format!("{pd}/store/{store}")).await;
        let status = &status["status"];
        gathered = status["region_count"].as_u64().is_some_and(|n| n > 0)
            && status["leader_count"] == status["region_count"];
        if gathered {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
    let done = format!("{schedulers}/grant-leader-scheduler");
    http.delete(&done).send().await.unwrap();
    check(gathered, "every Raft leader is on the node about to die");
}

async fn five_kill_mid_edit(server: &mut Server) {
    println!("\n5. Killing and restarting the server mid-edit loses nothing");
    if std::env::var("STACK_NODES").as_deref() == Ok("3") {
        leaders_onto_the_node_that_dies().await;
    }
    let base = server.base();
    const EDITS: u64 = 400;
    let model = "arch-model";
    let anna = Browser::sign_in(&base, "anna").await;
    let project = format!("{base}/api/projects/example-1");
    let (_, before) = anna.get(project.clone()).await;
    let head_before = before["head"].as_u64().unwrap();

    let (mut live, _) = Live::open(&anna, &base, "example-1", "tab-kill", &[model], 0)
        .await
        .unwrap();
    let mut unconfirmed: BTreeMap<u64, Value> = BTreeMap::new();
    let mut kills = 0;
    let mut sent = 0;
    let mut replies = Vec::new();
    let started = Instant::now();
    while sent < EDITS || !unconfirmed.is_empty() {
        if sent < EDITS {
            sent += 1;
            let args = at("D-201", 2000 + sent as i64, 1000);
            let message = live.edit(model, "element.move", args);
            unconfirmed.insert(live.counter, message.clone());
            if sent == 150 || sent == 300 {
                // Edits are flying when the server dies.
                let _ = live.send(message).await;
                server.kill();
                kills += 1;
                *server = Server::start(FIRST).await;
                let client = live.client.clone();
                let counter = live.counter;
                let (fresh, welcome) = loop {
                    match Live::open(&anna, &base, "example-1", &client, &[model], 0).await {
                        Ok(open) => break open,
                        Err(_) => sleep(Duration::from_millis(50)).await,
                    }
                };
                live = fresh;
                live.counter = counter;
                replies.push(welcome["last_counter"].as_u64().unwrap());
                for edit in unconfirmed.values() {
                    live.send(edit.clone()).await.unwrap();
                }
                continue;
            }
            let _ = live.send(message).await;
            if sent == 225 && std::env::var("STACK_NODES").as_deref() == Ok("3") {
                let killed = Command::new("pkill")
                    .args(["-9", "-f", "addr=127.0.0.1:20162"])
                    .status();
                assert!(
                    killed.unwrap().success(),
                    "a TiKV node should have been killed"
                );
                println!("      killed one of the three TiKV nodes at edit 225");
            }
        }
        let waiting = sent >= EDITS || unconfirmed.len() > 32;
        if waiting {
            let reply = live
                .until(|m| m["t"] == "ack" || m["t"] == "retry")
                .await
                .expect("a reply");
            if reply["t"] == "ack" {
                unconfirmed.remove(&reply["counter"].as_u64().unwrap());
            } else {
                let edit = unconfirmed[&reply["counter"].as_u64().unwrap()].clone();
                live.send(edit).await.unwrap();
            }
        }
    }
    println!(
        "      {EDITS} edits, {kills} hard kills, {:.1} s; the log had reached {replies:?} at each restart",
        started.elapsed().as_secs_f32()
    );
    let (_, after) = anna.get(project).await;
    let added = edits_since(&anna, &base, model, head_before).await;
    check(
        added == EDITS,
        &format!("exactly {EDITS} edits went into the log: none lost, none doubled"),
    );

    let area = format!(
        "{base}/api/projects/example-1/models/{model}/area?level=2&x0=0&y0=0&x1=9000&y1=9000"
    );
    let view = settled_area(&anna, area, after["head"].as_u64().unwrap()).await;
    let hash = view["pieces"]["D-201"].as_str().expect("the door is there");
    let (_, record) = anna
        .get(format!(
            "{base}/api/projects/example-1/models/{model}/record/{hash}"
        ))
        .await;
    check(
        record["placement"]["x"].as_i64() == Some(2000 + EDITS as i64),
        "the door stands where the last edit put it",
    );
    let next = live.edit(model, "element.release", json!({ "element": "D-201" }));
    let counter = next["counter"].as_u64().unwrap();
    let mut ahead = next.clone();
    ahead["counter"] = json!(counter + 5);
    live.send(ahead).await.unwrap();
    let reply = live
        .until(|m| m["counter"].as_u64() == Some(counter + 5))
        .await
        .unwrap();
    check(
        reply["t"] == "retry",
        "an edit that jumps ahead of an unsent one waits its turn",
    );
    live.send(next).await.unwrap();
    live.reply_to(counter).await;
    let verified = stack(&["verify"]);
    print!(
        "{}",
        verified
            .lines()
            .map(|l| format!("      {l}\n"))
            .collect::<String>()
    );
    pass("replaying every project's log gives exactly the stored model");
    pass("and every stored shape is what its recipe builds");
    // Just after a TiKV dies a region can be between leaders. `collect` says so and stops, and a
    // running server tries again on its timer, so this does too.
    let mut collected = false;
    for _ in 0..120 {
        collected = Command::new(BIN).arg("collect").output().unwrap().status.success();
        if collected {
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }
    check(collected, "every region has a leader again");
    stack(&["verify"]);
    pass("clearing old versions out of TiKV leaves the model and its log whole");
}

async fn six_backup_restores(server: &mut Server) {
    println!("\n6. A backup restores");
    server.kill();
    let digest_before = stack(&["digest"]);
    let name = format!("checklist-{}", now_ms());
    print!("      {}", stack(&["backup", &name]));
    script("stack-wipe.sh");
    let empty = stack(&["digest"]);
    check(empty != digest_before, "the store really is gone");
    print!("      {}", stack(&["restore", &name]));
    check(
        stack(&["digest"]) == digest_before,
        "every key and file came back, byte for byte",
    );
    let refused = Command::new(BIN).args(["restore", &name]).output().unwrap();
    check(
        !refused.status.success(),
        "a restore won't overwrite a store with data in it",
    );
    stack(&["verify"]);
    pass("the restored logs replay to the restored model");

    *server = Server::start(FIRST).await;
    let base = server.base();
    let anna = Browser::sign_in(&base, "anna").await;
    let (_, seen) = anna.get(format!("{base}/api/projects/example-1")).await;
    check(
        packages(&seen, "arch", "shared")
            .iter()
            .any(|p| p["name"] == "Arch package v12"),
        "Anna finds Arch package v12",
    );
    let (mut live, _) = Live::open(&anna, &base, "example-1", "tab-after", &["arch-model"], 0)
        .await
        .unwrap();
    let c = live
        .op("arch-model", "element.move", at("D-202", 6000, 3000))
        .await;
    check(live.reply_to(c).await["t"] == "ack", "and work carries on");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the running stack: use scripts/run-checklist.sh"]
async fn checklist() {
    let mut first = Server::start(FIRST).await;
    let second = Server::start(SECOND).await;
    let (base, other) = (first.base(), second.base());
    one_signs_in_and_lands_in_their_hub(&base).await;
    two_switches_hubs(&base).await;
    three_live_edits(&base, &other).await;
    shapes_follow_the_log(&base).await;
    many_writers(&base, &other).await;
    four_access_and_bridges(&base).await;
    drop(second);
    five_kill_mid_edit(&mut first).await;
    six_backup_restores(&mut first).await;
    println!("\nAll six checks passed.");
}
