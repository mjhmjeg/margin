//! The network (desktop only): blocking HTTPS through `ureq` with rustls. Call these from a
//! background thread, never the UI thread.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ureq::Agent;

use crate::wire::{self, Request};
use crate::{AssistantError, ChatReply, ChatRequest, Endpoint, Model};

/// Biggest answer read (a model list from OpenRouter is a few MB).
const MAX_BODY: u64 = 32 * 1024 * 1024;
/// Most pages of models followed (Anthropic pages by 1000).
const MAX_PAGES: usize = 10;
/// A streamed answer may run this long in total.
const MAX_STREAM: Duration = Duration::from_secs(600);

fn agent(timeout: Duration, stream: bool) -> Agent {
    let builder = Agent::config_builder()
        .http_status_as_error(false)
        .user_agent(concat!("Margin/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(timeout))
        .timeout_recv_response(Some(timeout));
    let builder = if stream { builder.timeout_recv_body(Some(MAX_STREAM)) } else { builder.timeout_global(Some(timeout)) };
    Agent::new_with_config(builder.build())
}

fn transport_error(e: ureq::Error, ep: &Endpoint, timeout: Duration) -> AssistantError {
    let where_ = crate::host(&ep.base_url);
    let unreachable = || if ep.local { AssistantError::NotRunning(ep.base_url.clone()) } else { AssistantError::NoNetwork(where_.clone()) };
    match e {
        ureq::Error::Timeout(_) => AssistantError::Timeout(timeout.as_secs().max(1)),
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed => unreachable(),
        ureq::Error::Io(io) => match io.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => AssistantError::Timeout(timeout.as_secs().max(1)),
            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::AddrNotAvailable
            | std::io::ErrorKind::HostUnreachable
            | std::io::ErrorKind::NetworkUnreachable
            | std::io::ErrorKind::NetworkDown => unreachable(),
            _ => AssistantError::NoNetwork(format!("{where_} ({})", wire::scrub(&io.to_string(), None))),
        },
        ureq::Error::BodyExceedsLimit(_) => AssistantError::Protocol("the answer was too large".into()),
        ureq::Error::BadUri(_) => AssistantError::Setup("that address isn't a valid URL".into()),
        ureq::Error::Tls(_) => AssistantError::NoNetwork(format!("{where_} (secure connection failed)")),
        // Other variants never carry request headers; shown short, the key taken out regardless.
        other => AssistantError::NoNetwork(format!("{where_} ({})", wire::scrub(&other.to_string(), ep.api_key.as_deref()))),
    }
}

type Response = ureq::http::Response<ureq::Body>;

/// Send `req`; the status and the response.
fn send(agent: &Agent, req: &Request, ep: &Endpoint, timeout: Duration) -> Result<(u16, Response), AssistantError> {
    let r = if req.method == "POST" {
        let mut b = agent.post(&req.url);
        for (k, v) in &req.headers {
            b = b.header(*k, v.as_str());
        }
        b.send(req.body.as_deref().unwrap_or_default())
    } else {
        let mut b = agent.get(&req.url);
        for (k, v) in &req.headers {
            b = b.header(*k, v.as_str());
        }
        b.call()
    };
    let resp = r.map_err(|e| transport_error(e, ep, timeout))?;
    Ok((resp.status().as_u16(), resp))
}

fn read_body(resp: &mut Response, ep: &Endpoint, timeout: Duration) -> Result<String, AssistantError> {
    resp.body_mut().with_config().limit(MAX_BODY).read_to_string().map_err(|e| transport_error(e, ep, timeout))
}

/// One request: its body when the status is success, the provider's error otherwise.
fn exchange(agent: &Agent, req: &Request, ep: &Endpoint, timeout: Duration) -> Result<String, AssistantError> {
    let (status, mut resp) = send(agent, req, ep, timeout)?;
    let body = read_body(&mut resp, ep, timeout)?;
    if (200..300).contains(&status) { Ok(body) } else { Err(wire::status_error(status, &body, ep.api_key.as_deref())) }
}

/// The models `ep` offers (also the "Test connection" call), sorted by name.
pub fn list_models(ep: &Endpoint, timeout: Duration) -> Result<Vec<Model>, AssistantError> {
    let agent = agent(timeout, false);
    let mut all = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let body = exchange(&agent, &wire::models_request(ep, after.as_deref()), ep, timeout)?;
        let (page, next) = wire::parse_models(ep.connector, &body)?;
        all.extend(page);
        match next {
            Some(n) if all.len() < wire::MAX_MODELS => after = Some(n),
            _ => break,
        }
    }
    all.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id)));
    all.dedup_by(|a, b| a.id == b.id);
    Ok(all)
}

/// A plain chat call, answered in one piece.
pub fn chat(ep: &Endpoint, req: &ChatRequest, timeout: Duration) -> Result<ChatReply, AssistantError> {
    let body = exchange(&agent(timeout, false), &wire::chat_request(ep, req, false), ep, timeout)?;
    wire::parse_chat(ep.connector, &body)
}

/// A chat call streamed: `on_text` gets each piece of text as it arrives. `timeout` covers
/// connecting and the start of the answer; the whole answer may take up to ten minutes.
pub fn chat_stream(ep: &Endpoint, req: &ChatRequest, timeout: Duration, on_text: &mut dyn FnMut(&str)) -> Result<ChatReply, AssistantError> {
    chat_stream_until(ep, req, timeout, &AtomicBool::new(false), on_text)
}

/// [`chat_stream`] that ends with [`AssistantError::Stopped`] once `stop` is set (checked
/// between the lines of the answer; the connection is dropped).
pub fn chat_stream_until(
    ep: &Endpoint,
    req: &ChatRequest,
    timeout: Duration,
    stop: &AtomicBool,
    on_text: &mut dyn FnMut(&str),
) -> Result<ChatReply, AssistantError> {
    if stop.load(Ordering::Relaxed) {
        return Err(AssistantError::Stopped);
    }
    let agent = agent(timeout, true);
    // A server that refuses JSON mode or the token number is asked once more without it.
    let mut req = req.clone();
    let mut json_refused = false;
    let mut resp = None;
    for _ in 0..3 {
        let (status, mut r) = send(&agent, &wire::chat_request(ep, &req, true), ep, timeout)?;
        if (200..300).contains(&status) {
            resp = Some(r);
            break;
        }
        let body = read_body(&mut r, ep, timeout)?;
        if req.json && wire::refuses_json_mode(status, &body) {
            log::info!("assistant: the server refused JSON mode; asking again without it");
            req.json = false;
            json_refused = true;
        } else if req.max_tokens.is_some() && wire::refuses_max_tokens(status, &body) {
            log::info!("assistant: the server refused the output budget; asking again without it");
            req.max_tokens = None;
        } else {
            return Err(wire::status_error(status, &body, ep.api_key.as_deref()));
        }
        if stop.load(Ordering::Relaxed) {
            return Err(AssistantError::Stopped);
        }
    }
    let Some(mut resp) = resp else { return Err(AssistantError::Protocol("the server refused the request".into())) };
    let reader = resp.body_mut().with_config().limit(MAX_BODY).reader();
    let mut parser = wire::StreamParser::new(ep.connector);
    for line in BufReader::new(reader).lines() {
        if stop.load(Ordering::Relaxed) {
            return Err(AssistantError::Stopped);
        }
        let line = line.map_err(|e| transport_error(ureq::Error::Io(e), ep, timeout))?;
        if let Some(t) = parser.line(&line)? {
            on_text(&t);
        }
        if parser.done() {
            break;
        }
    }
    let mut reply = parser.finish();
    reply.json_refused = json_refused;
    Ok(reply)
}
