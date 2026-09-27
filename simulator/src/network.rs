//! Asynchronous desktop HTTP. No Lua or UI objects cross the worker boundary.
use mlua::{Lua, Value};
use serde_json::Value as Json;
use std::{
    sync::{LazyLock, mpsc},
    time::Duration,
};
static EXECUTOR: LazyLock<std::io::Result<tokio::runtime::Runtime>> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("lingclaw-http")
        .build()
});
// Built on a worker, including loading platform certificate roots.
static CLIENT: LazyLock<Result<reqwest::Client, reqwest::Error>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
});
pub enum Response {
    Complete {
        status: u16,
        content_type: Vec<u8>,
        body: Vec<u8>,
    },
    Error(&'static str),
}
impl Response {
    pub fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        let table = lua.create_table()?;
        match self {
            Self::Complete {
                status,
                content_type,
                body,
            } => {
                table.set("status", status)?;
                table.set("content_type", lua.create_string(&content_type)?)?;
                table.set("body", lua.create_string(&body)?)?;
            }
            Self::Error(code) => {
                let error = lua.create_table()?;
                error.set("code", code)?;
                error.set(
                    "message",
                    match code {
                        "timeout" => "HTTP request timed out",
                        "response_too_large" => "Response exceeds requested limit",
                        "unavailable" => "HTTP client unavailable",
                        _ => "HTTP request failed",
                    },
                )?;
                table.set("error", error)?;
            }
        }
        Ok(Value::Table(table))
    }
}
struct Request {
    options: Json,
    max: usize,
    timeout: Duration,
    body: Option<Vec<u8>>,
}
/// I/O begins on the first normal tick, after startup has been accepted.
/// Dropping a transfer cancels both its task and callback delivery.
pub struct Transfer {
    request: Option<Request>,
    job: Option<tokio::task::JoinHandle<()>>,
    receiver: Option<mpsc::Receiver<Response>>,
    response: Option<Response>,
}
impl Transfer {
    pub fn new(options: Json, max: usize, timeout_ms: u64, body: Option<Vec<u8>>) -> Self {
        Self {
            request: Some(Request {
                options,
                max,
                timeout: Duration::from_millis(timeout_ms),
                body,
            }),
            job: None,
            receiver: None,
            response: None,
        }
    }
    pub fn poll(&mut self) {
        if let Some(request) = self.request.take() {
            match &*EXECUTOR {
                Ok(executor) => {
                    let (sender, receiver) = mpsc::sync_channel(1);
                    self.receiver = Some(receiver);
                    self.job = Some(executor.spawn(async move {
                        let response =
                            match tokio::time::timeout(request.timeout, execute(request)).await {
                                Ok(response) => response,
                                Err(_) => Response::Error("timeout"),
                            };
                        let _ = sender.send(response);
                    }));
                }
                Err(_) => self.response = Some(Response::Error("unavailable")),
            }
        }
        if self.response.is_none()
            && let Some(receiver) = &self.receiver
        {
            match receiver.try_recv() {
                Ok(response) => self.response = Some(response),
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.response = Some(Response::Error("network_error"))
                }
                Err(mpsc::TryRecvError::Empty) => (),
            }
        }
    }
    pub fn ready(&self) -> bool {
        self.response.is_some()
    }
    pub fn take_response(&mut self) -> Response {
        self.response.take().expect("ready HTTP response")
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            job.abort();
        }
    }
}
async fn execute(request: Request) -> Response {
    let Ok(client) = &*CLIENT else {
        return Response::Error("unavailable");
    };
    let opts = request.options;
    let method = if opts["method"].as_str().unwrap_or("GET") == "POST" {
        reqwest::Method::POST
    } else {
        reqwest::Method::GET
    };
    let mut builder = client
        .request(method, opts["url"].as_str().unwrap())
        .timeout(request.timeout);
    if let Some(headers) = opts["headers"].as_object() {
        for (name, value) in headers {
            builder = builder.header(name, value.as_str().unwrap());
        }
    }
    if let Some(body) = request.body {
        builder = builder.body(body);
    }
    let mut response = match builder.send().await {
        Ok(response) => response,
        Err(e) => {
            return Response::Error(if e.is_timeout() {
                "timeout"
            } else {
                "network_error"
            });
        }
    };
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .map(|v| v.as_bytes().iter().take(127).copied().collect())
        .unwrap_or_default();
    if response
        .content_length()
        .is_some_and(|len| len > request.max as u64)
    {
        return Response::Error("response_too_large");
    }
    let mut body = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if chunk.len() > request.max - body.len() {
                    return Response::Error("response_too_large");
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => {
                return Response::Error(if e.is_timeout() {
                    "timeout"
                } else {
                    "network_error"
                });
            }
        }
    }
    Response::Complete {
        status,
        content_type,
        body,
    }
}
