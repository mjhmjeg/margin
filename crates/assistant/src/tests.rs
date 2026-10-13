//! Connector tests against a scripted HTTP server on 127.0.0.1 (no real network).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{Value, json};

use crate::keys::{KeyStore, MemoryKeys};
use crate::wire::{self, StreamParser};
use crate::*;

const KEY: &str = "sk-test-0123456789abcd";

/// What the mock server saw.
#[derive(Debug, Default, Clone)]
pub struct Seen {
    pub line: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// One scripted answer: status, content type, body, and a delay before answering.
struct Answer(u16, &'static str, String, Duration);

fn ok_json(v: Value) -> Answer {
    Answer(200, "application/json", v.to_string(), Duration::ZERO)
}

/// A server answering one connection per answer, in order; returns the base URL and the requests.
fn serve(answers: Vec<Answer>) -> (String, JoinHandle<Vec<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}/v1", listener.local_addr().unwrap().port());
    let h = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for Answer(status, ctype, body, delay) in answers {
            let Ok((stream, _)) = listener.accept() else { break };
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut r = BufReader::new(stream.try_clone().unwrap());
            let mut s = Seen::default();
            r.read_line(&mut s.line).unwrap();
            let mut len = 0usize;
            loop {
                let mut l = String::new();
                if r.read_line(&mut l).unwrap_or(0) == 0 || l == "\r\n" {
                    break;
                }
                if let Some((k, v)) = l.trim_end().split_once(':') {
                    if k.eq_ignore_ascii_case("content-length") {
                        len = v.trim().parse().unwrap_or(0);
                    }
                    s.headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            let mut b = vec![0; len];
            r.read_exact(&mut b).unwrap();
            s.body = String::from_utf8_lossy(&b).into_owned();
            seen.push(s);
            std::thread::sleep(delay);
            let mut stream = stream;
            let _ =
                write!(stream, "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            let _ = stream.flush();
        }
        seen
    });
    (base, h)
}

fn endpoint(connector: Connector, base: &str, key: Option<&str>) -> Endpoint {
    Endpoint { connector, base_url: base.to_string(), api_key: key.map(str::to_string), local: false }
}

fn chat_req() -> ChatRequest {
    ChatRequest {
        model: "m1".into(),
        system: Some("Be brief.".into()),
        messages: vec![Message { role: Role::User, content: "Hi".into() }],
        max_tokens: None,
    }
}

#[test]
fn presets_are_in_order_with_mistral_first() {
    let ids: Vec<&str> = ProviderId::ALL.iter().map(|p| p.id()).collect();
    assert_eq!(ids, ["mistral", "anthropic", "openai", "gemini", "openrouter", "local", "other"]);
    assert_eq!(ProviderId::Anthropic.preset().connector, Connector::Anthropic);
    for p in ProviderId::ALL {
        assert_eq!(ProviderId::from_id(p.id()), Some(p));
        let pr = p.preset();
        assert_eq!(pr.id, p);
        if p != ProviderId::Anthropic {
            assert_eq!(pr.connector, Connector::OpenAi, "{p:?}");
        }
        if !pr.editable_url {
            assert!(pr.base_url.starts_with("https://") && pr.needs_key, "{p:?}");
        }
    }
    assert_eq!(ProviderId::Local.preset().base_url, "http://localhost:11434/v1");
    assert!(!ProviderId::Local.preset().needs_key);
}

#[test]
fn endpoints_check_the_address() {
    let e = Endpoint::new(ProviderId::Mistral, Some("http://evil.example"), Some("k".into())).unwrap();
    assert_eq!(e.base_url, "https://api.mistral.ai/v1", "fixed presets ignore a URL");
    assert_eq!(e.url("/models"), "https://api.mistral.ai/v1/models");
    let l = Endpoint::new(ProviderId::Local, None, None).unwrap();
    assert!(l.local && l.api_key.is_none());
    let o = Endpoint::new(ProviderId::Other, Some(" https://eu.example.com/v1/ "), Some(" key ".into())).unwrap();
    assert_eq!(o.base_url, "https://eu.example.com/v1");
    assert_eq!(o.api_key.as_deref(), Some("key"));
    assert!(Endpoint::new(ProviderId::Other, None, None).is_err(), "Other needs an address");
    assert!(Endpoint::new(ProviderId::Other, Some("ftp://x"), None).is_err());
    assert!(Endpoint::new(ProviderId::Other, Some("https://"), None).is_err());
    // A key never goes over plain http to another computer; to this one it may.
    assert!(Endpoint::new(ProviderId::Other, Some("http://example.com/v1"), Some("k".into())).is_err());
    assert!(Endpoint::new(ProviderId::Other, Some("http://example.com/v1"), None).is_ok());
    assert!(Endpoint::new(ProviderId::Other, Some("http://127.0.0.1:8080/v1"), Some("k".into())).unwrap().local);
    assert!(is_loopback("http://[::1]:1234/v1") && is_loopback("http://localhost/v1") && !is_loopback("https://localhost.example.com"));
}

#[test]
fn debug_and_errors_never_show_the_key() {
    let e = endpoint(Connector::OpenAi, "https://x.example/v1", Some(KEY));
    assert!(!format!("{e:?}").contains(KEY));
    let err = wire::status_error(401, &json!({"error": {"message": format!("Incorrect API key provided: {KEY}")}}).to_string(), Some(KEY));
    assert_eq!(err, AssistantError::BadKey);
    assert!(!err.to_string().contains(KEY));
    let err = wire::status_error(400, &json!({"error": {"message": format!("bad {KEY}")}}).to_string(), Some(KEY));
    assert!(!err.to_string().contains(KEY), "{err}");
    assert_eq!(masked(KEY), "••••abcd");
    assert_eq!(masked("ab"), "••••ab");
}

#[test]
fn keys_are_cleaned() {
    assert_eq!(clean_key("  abc-123\n").unwrap(), "abc-123");
    assert!(clean_key("   ").is_err());
    assert!(clean_key("two words").is_err());
    assert!(clean_key(&"x".repeat(MAX_KEY_CHARS + 1)).is_err());
}

#[test]
fn the_memory_store_keeps_one_key_per_provider() {
    let s = MemoryKeys::default();
    assert!(s.supported());
    assert_eq!(s.get("mistral").unwrap(), None);
    s.set("mistral", "a").unwrap();
    s.set("anthropic", "b").unwrap();
    assert_eq!(s.get("mistral").unwrap().as_deref(), Some("a"));
    s.remove("mistral").unwrap();
    s.remove("mistral").unwrap();
    assert_eq!(s.get("mistral").unwrap(), None);
    assert_eq!(s.get("anthropic").unwrap().as_deref(), Some("b"));
}

/// The real keychain: run by hand (`cargo test -p wordcraft-assistant -- --ignored`). Ignored in
/// CI because it writes to the login keychain of whoever runs it and may show a macOS prompt.
#[test]
#[ignore = "touches the real system keychain"]
fn the_system_keychain_round_trips() {
    let s = crate::keys::SystemKeys;
    let account = "test-wordcraft-assistant";
    s.set(account, KEY).unwrap();
    assert_eq!(s.get(account).unwrap().as_deref(), Some(KEY));
    s.remove(account).unwrap();
    assert_eq!(s.get(account).unwrap(), None);
}

#[test]
fn openai_compatible_models_are_listed_with_a_bearer_key() {
    let (base, h) = serve(vec![ok_json(json!({"object": "list", "data": [
        {"id": "mistral-small-latest", "object": "model"},
        {"id": "models/gemini-2.5-flash", "name": "Gemini Flash"},
        {"id": ""},
        {"object": "model"}
    ]}))]);
    let models = http::list_models(&endpoint(Connector::OpenAi, &base, Some(KEY)), Duration::from_secs(5)).unwrap();
    let seen = h.join().unwrap();
    assert_eq!(seen[0].line.trim(), "GET /v1/models HTTP/1.1");
    assert_eq!(seen[0].header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    assert_eq!(seen[0].header("x-api-key"), None);
    assert_eq!(
        models,
        vec![
            Model { id: "gemini-2.5-flash".into(), name: "Gemini Flash".into() },
            Model { id: "mistral-small-latest".into(), name: "mistral-small-latest".into() }
        ]
    );
}

#[test]
fn a_local_server_needs_no_key() {
    let (base, h) = serve(vec![ok_json(json!({"data": [{"id": "llama3.2"}]}))]);
    let ep = Endpoint::new(ProviderId::Local, Some(&base), None).unwrap();
    assert_eq!(http::list_models(&ep, Duration::from_secs(5)).unwrap().len(), 1);
    assert_eq!(h.join().unwrap()[0].header("authorization"), None);
}

#[test]
fn anthropic_models_follow_the_pages() {
    let (base, h) = serve(vec![
        ok_json(json!({"data": [{"id": "claude-b", "display_name": "Claude B", "type": "model"}], "has_more": true, "last_id": "claude-b"})),
        ok_json(json!({"data": [{"id": "claude-a", "display_name": "Claude A"}], "has_more": false, "last_id": "claude-a"})),
    ]);
    let models = http::list_models(&endpoint(Connector::Anthropic, &base, Some(KEY)), Duration::from_secs(5)).unwrap();
    let seen = h.join().unwrap();
    assert_eq!(seen[0].line.trim(), "GET /v1/models?limit=1000 HTTP/1.1");
    assert_eq!(seen[1].line.trim(), "GET /v1/models?limit=1000&after_id=claude-b HTTP/1.1");
    assert_eq!(seen[0].header("x-api-key"), Some(KEY));
    assert_eq!(seen[0].header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(seen[0].header("authorization"), None);
    assert_eq!(models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Claude A", "Claude B"]);
}

#[test]
fn a_refused_key_is_a_bad_key_error() {
    for connector in [Connector::OpenAi, Connector::Anthropic] {
        let (base, h) = serve(vec![Answer(
            401,
            "application/json",
            json!({"error": {"message": format!("Incorrect API key {KEY}")}}).to_string(),
            Duration::ZERO,
        )]);
        let e = http::list_models(&endpoint(connector, &base, Some(KEY)), Duration::from_secs(5)).unwrap_err();
        h.join().unwrap();
        assert_eq!(e, AssistantError::BadKey);
        assert!(!e.to_string().contains(KEY));
    }
}

#[test]
fn other_errors_carry_the_providers_message() {
    let (base, h) = serve(vec![Answer(404, "application/json", json!({"message": "No such model"}).to_string(), Duration::ZERO)]);
    let e = http::chat(&endpoint(Connector::OpenAi, &base, Some(KEY)), &chat_req(), Duration::from_secs(5)).unwrap_err();
    h.join().unwrap();
    assert_eq!(e, AssistantError::Http { status: 404, message: "No such model".into() });
    let (base, h) = serve(vec![Answer(200, "application/json", "not json".into(), Duration::ZERO)]);
    assert!(matches!(http::list_models(&endpoint(Connector::OpenAi, &base, None), Duration::from_secs(5)), Err(AssistantError::Protocol(_))));
    h.join().unwrap();
}

#[test]
fn a_slow_server_times_out() {
    let (base, h) = serve(vec![Answer(200, "application/json", "{}".into(), Duration::from_millis(1500))]);
    let t = std::time::Instant::now();
    let e = http::list_models(&endpoint(Connector::OpenAi, &base, None), Duration::from_millis(300)).unwrap_err();
    assert!(matches!(e, AssistantError::Timeout(_)), "{e:?}");
    assert!(t.elapsed() < Duration::from_millis(1400));
    h.join().unwrap();
}

#[test]
fn nothing_listening_locally_says_the_server_isnt_running() {
    // Bind and drop: the port is free and refuses connections.
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let ep = Endpoint::new(ProviderId::Local, Some(&format!("http://127.0.0.1:{port}/v1")), None).unwrap();
    let e = http::list_models(&ep, Duration::from_secs(3)).unwrap_err();
    assert!(matches!(e, AssistantError::NotRunning(_)), "{e:?}");
    assert!(e.to_string().contains("Ollama"));
}

#[test]
fn an_openai_compatible_chat_call() {
    let (base, h) =
        serve(vec![ok_json(json!({"model": "m1", "choices": [{"message": {"role": "assistant", "content": "Hello!"}, "finish_reason": "stop"}]}))]);
    let r = http::chat(&endpoint(Connector::OpenAi, &base, Some(KEY)), &chat_req(), Duration::from_secs(5)).unwrap();
    assert_eq!(r, ChatReply { text: "Hello!".into(), model: "m1".into(), stop_reason: Some("stop".into()) });
    let seen = h.join().unwrap();
    assert_eq!(seen[0].line.trim(), "POST /v1/chat/completions HTTP/1.1");
    let body: Value = serde_json::from_str(&seen[0].body).unwrap();
    assert_eq!(body, json!({"model": "m1", "messages": [{"role": "system", "content": "Be brief."}, {"role": "user", "content": "Hi"}]}));
}

#[test]
fn an_anthropic_chat_call() {
    let (base, h) = serve(vec![ok_json(
        json!({"model": "claude-x", "content": [{"type": "text", "text": "Hel"}, {"type": "text", "text": "lo"}], "stop_reason": "end_turn"}),
    )]);
    let r = http::chat(&endpoint(Connector::Anthropic, &base, Some(KEY)), &chat_req(), Duration::from_secs(5)).unwrap();
    assert_eq!(r.text, "Hello");
    assert_eq!(r.stop_reason.as_deref(), Some("end_turn"));
    let seen = h.join().unwrap();
    assert_eq!(seen[0].line.trim(), "POST /v1/messages HTTP/1.1");
    let body: Value = serde_json::from_str(&seen[0].body).unwrap();
    assert_eq!(body, json!({"model": "m1", "max_tokens": 4096, "system": "Be brief.", "messages": [{"role": "user", "content": "Hi"}]}));
}

#[test]
fn streamed_chats_arrive_in_pieces() {
    let sse = "data: {\"model\":\"m1\",\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
    let (base, h) = serve(vec![Answer(200, "text/event-stream", sse.into(), Duration::ZERO)]);
    let mut pieces = Vec::new();
    let r = http::chat_stream(&endpoint(Connector::OpenAi, &base, None), &chat_req(), Duration::from_secs(5), &mut |t| pieces.push(t.to_string()))
        .unwrap();
    assert_eq!(pieces, ["Hel", "lo"]);
    assert_eq!(r.text, "Hello");
    assert_eq!(r.stop_reason.as_deref(), Some("stop"));
    let seen = h.join().unwrap();
    assert_eq!(seen[0].header("accept"), Some("text/event-stream"));
    assert!(seen[0].body.contains("\"stream\":true"));

    let sse = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"claude-x\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\nevent: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
    let (base, h) = serve(vec![Answer(200, "text/event-stream", sse.into(), Duration::ZERO)]);
    let r = http::chat_stream(&endpoint(Connector::Anthropic, &base, Some(KEY)), &chat_req(), Duration::from_secs(5), &mut |_| {}).unwrap();
    assert_eq!(r, ChatReply { text: "Hi".into(), model: "claude-x".into(), stop_reason: Some("end_turn".into()) });
    h.join().unwrap();
}

#[test]
fn a_stream_error_event_is_an_error() {
    let mut p = StreamParser::new(Connector::Anthropic);
    assert!(p.line("event: error").unwrap().is_none());
    assert!(p.line("data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}").is_err());
    let mut p = StreamParser::new(Connector::OpenAi);
    assert_eq!(p.line(": keep-alive").unwrap(), None);
    assert_eq!(p.line("data: not json").unwrap(), None);
}

#[test]
fn hostile_model_lists_never_panic() {
    for body in ["", "null", "[]", "{\"data\": 5}", "{\"data\": [1, null, {\"id\": 3}]}", "[{\"id\": \"x\"}]", "{\"has_more\": true}"] {
        let _ = wire::parse_models(Connector::Anthropic, body);
        let _ = wire::parse_models(Connector::OpenAi, body);
        let _ = wire::parse_chat(Connector::Anthropic, body);
        let _ = wire::parse_chat(Connector::OpenAi, body);
    }
    assert_eq!(wire::parse_models(Connector::OpenAi, "[{\"id\": \"x\"}]").unwrap().0.len(), 1);
}

#[test]
fn a_stopped_stream_ends_with_stopped() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let sse =
        "data: {\"choices\":[{\"delta\":{\"content\":\"one\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"two\"}}]}\n\ndata: [DONE]\n\n";
    let (base, h) = serve(vec![Answer(200, "text/event-stream", sse.into(), Duration::ZERO)]);
    let stop = AtomicBool::new(false);
    let mut pieces = Vec::new();
    let r = http::chat_stream_until(&endpoint(Connector::OpenAi, &base, None), &chat_req(), Duration::from_secs(5), &stop, &mut |t| {
        pieces.push(t.to_string());
        stop.store(true, Ordering::Relaxed);
    });
    assert_eq!(r, Err(AssistantError::Stopped));
    assert_eq!(pieces, ["one"], "nothing after the stop");
    h.join().unwrap();
    // Stopped before it starts: nothing is sent.
    let r =
        http::chat_stream_until(&endpoint(Connector::OpenAi, "http://127.0.0.1:9/v1", None), &chat_req(), Duration::from_secs(1), &stop, &mut |_| {});
    assert_eq!(r, Err(AssistantError::Stopped));
}
