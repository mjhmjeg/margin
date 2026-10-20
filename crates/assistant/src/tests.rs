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
    let provider = if connector == Connector::Anthropic { ProviderId::Anthropic } else { ProviderId::Other };
    Endpoint { provider, connector, base_url: base.to_string(), api_key: key.map(str::to_string), local: false }
}

fn chat_req() -> ChatRequest {
    ChatRequest {
        model: "m1".into(),
        system: Some("Be brief.".into()),
        messages: vec![Message { role: Role::User, content: "Hi".into() }],
        max_tokens: None,
        json: false,
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

/// Margin → Optimo (2026-10-14): a key stored under the old service is copied to the new one
/// where the new one has none; the old entry stays as a backup; a key already in the new store
/// is never overwritten; running it twice changes nothing. Memory stores only: the real
/// keychain is never touched by tests.
#[test]
fn keys_move_from_margins_service_to_optimos_once_and_never_overwrite() {
    use crate::keys::migrate;
    assert_eq!(crate::keys::SERVICE, "ai.spert.optimo.assistant");
    assert_eq!(crate::keys::LEGACY_SERVICE, "ai.spert.margin.assistant");
    assert_eq!(crate::keys::SystemKeys::default().service(), crate::keys::SERVICE);
    assert_eq!(crate::keys::SystemKeys::legacy().service(), crate::keys::LEGACY_SERVICE);
    let (old, new) = (MemoryKeys::default(), MemoryKeys::default());
    old.set("mistral", KEY).unwrap();
    old.set("anthropic", "old-anthropic").unwrap();
    new.set("anthropic", "new-anthropic").unwrap();
    let accounts: Vec<&str> = ProviderId::ALL.iter().map(|p| p.id()).collect();
    assert_eq!(migrate(&old, &new, &accounts), (vec!["mistral".to_string()], true));
    assert_eq!(new.get("mistral").unwrap().as_deref(), Some(KEY));
    assert_eq!(new.get("anthropic").unwrap().as_deref(), Some("new-anthropic"), "never overwritten");
    assert_eq!(old.get("mistral").unwrap().as_deref(), Some(KEY), "the old entry stays as a backup");
    assert_eq!(new.get("openai").unwrap(), None);
    assert_eq!(migrate(&old, &new, &accounts), (vec![], true), "a second run copies nothing");
}

/// The real keychain: run by hand (`cargo test -p wordcraft-assistant -- --ignored`). Ignored in
/// CI because it writes to the login keychain of whoever runs it and may show a macOS prompt.
#[test]
#[ignore = "touches the real system keychain"]
fn the_system_keychain_round_trips() {
    let s = crate::keys::SystemKeys::default();
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
    assert_eq!(r, ChatReply { text: "Hello!".into(), model: "m1".into(), stop_reason: Some("stop".into()), json_refused: false });
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
    assert_eq!(r, ChatReply { text: "Hi".into(), model: "claude-x".into(), stop_reason: Some("end_turn".into()), json_refused: false });
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

// Incident 2026-10-13: JSON mode, output budget, Mistral's chunked content.

#[test]
fn json_mode_and_the_output_budget_are_asked_for_per_provider() {
    let req = ChatRequest { max_tokens: Some(8192), json: true, ..chat_req() };
    let body = |ep: &Endpoint| -> Value { serde_json::from_str(wire::chat_request(ep, &req, true).body.as_deref().unwrap()).unwrap() };
    let mistral = Endpoint::new(ProviderId::Mistral, None, Some("k".into())).unwrap();
    let b = body(&mistral);
    assert_eq!(b["response_format"], json!({"type": "json_object"}));
    assert_eq!(b["max_tokens"], 8192);
    let openai = Endpoint::new(ProviderId::OpenAi, None, Some("k".into())).unwrap();
    let b = body(&openai);
    assert_eq!(b["max_completion_tokens"], 8192, "OpenAI's newer models refuse max_tokens");
    assert!(b.get("max_tokens").is_none());
    let claude = Endpoint::new(ProviderId::Anthropic, None, Some("k".into())).unwrap();
    let b = body(&claude);
    assert_eq!(b["max_tokens"], 8192);
    assert!(b.get("response_format").is_none(), "no JSON switch on the Messages API");
    let plain = ChatRequest { json: false, ..req.clone() };
    let b: Value = serde_json::from_str(wire::chat_request(&mistral, &plain, false).body.as_deref().unwrap()).unwrap();
    assert!(b.get("response_format").is_none());
}

#[test]
fn refusals_of_json_mode_and_of_the_budget_are_recognised() {
    assert!(wire::refuses_json_mode(400, r#"{"error": {"message": "'response_format.type' must be 'json_schema' or 'text'"}}"#));
    assert!(wire::refuses_json_mode(422, "json_object is not supported for this model"));
    assert!(!wire::refuses_json_mode(500, "response_format"));
    assert!(!wire::refuses_json_mode(400, "model not found"));
    assert!(wire::refuses_max_tokens(400, "max_tokens: 16384 > 4096, which is the maximum allowed"));
    assert!(wire::refuses_max_tokens(400, "Unsupported parameter: 'max_tokens'. Use 'max_completion_tokens' instead."));
    assert!(!wire::refuses_max_tokens(429, "max_tokens"));
}

#[test]
fn mistrals_chunked_reasoning_content_gives_only_the_answer_text() {
    // https://docs.mistral.ai/capabilities/reasoning/: thinking chunks, then a transition event
    // with the last thinking and the first text chunk, then plain strings.
    let mut p = StreamParser::new(Connector::OpenAi);
    let ev = |c: Value| format!("data: {}", json!({"model": "mistral-medium", "choices": [{"index": 0, "delta": {"content": c}}]}));
    assert_eq!(p.line(&ev(json!([{"type": "thinking", "thinking": [{"type": "text", "text": "hmm {"}]}]))).unwrap(), None);
    assert_eq!(
        p.line(&ev(json!([{"type": "thinking", "thinking": [{"type": "text", "text": "ok"}]}, {"type": "text", "text": "{\"report\""}])))
            .unwrap()
            .as_deref(),
        Some("{\"report\"")
    );
    assert_eq!(p.line(&ev(json!(": \"x\"}"))).unwrap().as_deref(), Some(": \"x\"}"));
    assert_eq!(p.finish().text, "{\"report\": \"x\"}");
    let whole = r#"{"model": "m", "choices": [{"message": {"content": [{"type": "thinking", "thinking": [{"type": "text", "text": "t"}]}, {"type": "text", "text": "answer"}]}, "finish_reason": "stop"}]}"#;
    assert_eq!(wire::parse_chat(Connector::OpenAi, whole).unwrap().text, "answer");
}

#[test]
fn a_server_refusing_json_mode_is_asked_once_more_without_it() {
    let refusal =
        Answer(400, "application/json", json!({"error": {"message": "response_format json_object is not supported"}}).to_string(), Duration::ZERO);
    let ok = Answer(
        200,
        "text/event-stream",
        format!("data: {}\n\ndata: [DONE]\n\n", json!({"choices": [{"delta": {"content": "{}"}, "finish_reason": "stop"}]})),
        Duration::ZERO,
    );
    let (base, h) = serve(vec![refusal, ok]);
    let ep = endpoint(Connector::OpenAi, &base, None);
    let req = ChatRequest { json: true, max_tokens: Some(8192), ..chat_req() };
    let r = http::chat_stream(&ep, &req, Duration::from_secs(5), &mut |_| {}).unwrap();
    assert!(r.json_refused);
    assert_eq!(r.text, "{}");
    let seen = h.join().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(seen[0].body.contains("response_format") && !seen[1].body.contains("response_format"));
    assert!(seen[1].body.contains("\"max_tokens\":8192"), "the budget stays");
}

/// Mistral's web search: the Conversations call, and its references and searched pages read back
/// (the answer's shape as Mistral sent it on 2026-10-20).
#[test]
fn web_search_sends_the_tool_and_reads_references_and_pages() {
    let result = json!({
        "a1": {"url": "https://en.wikipedia.org/wiki/Albula_railway_line", "title": "Albula railway line", "date": null, "snippets": ["…"]},
        "b2": {"url": "https://whc.unesco.org/en/list/1276/", "title": "UNESCO", "date": "2008-07-07"}
    })
    .to_string();
    let answer = json!({"object": "conversation.response", "outputs": [
        {"type": "tool.execution", "name": "web_search", "info": {"result": result}},
        {"type": "message.output", "model": "mistral-medium-latest", "content": [
            {"type": "text", "text": "- It opened on 1 July 1903"},
            {"type": "tool_reference", "tool": "web_search", "title": "Albula railway line", "url": "https://en.wikipedia.org/wiki/Albula_railway_line"},
            {"type": "text", "text": "."}
        ]}
    ]});
    let (base, h) = serve(vec![ok_json(answer)]);
    let mut ep = endpoint(Connector::OpenAi, &base, Some(KEY));
    ep.provider = ProviderId::Mistral;
    let mut req = chat_req();
    req.max_tokens = Some(800);
    let reply = http::web_search(&ep, &req, Duration::from_secs(5), &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let seen = h.join().unwrap();
    assert!(seen[0].line.starts_with("POST /v1/conversations"), "{}", seen[0].line);
    let body: Value = serde_json::from_str(&seen[0].body).unwrap();
    assert_eq!(body["tools"], json!([{"type": "web_search"}]));
    assert_eq!(body["store"], json!(false), "nothing is kept on Mistral's side");
    assert_eq!(body["instructions"], json!("Be brief."));
    assert_eq!(body["completion_args"]["max_tokens"], json!(800));
    assert_eq!(seen[0].header("authorization"), Some(format!("Bearer {KEY}").as_str()));
    assert_eq!(reply.text(), "- It opened on 1 July 1903.");
    assert_eq!(reply.pieces.len(), 3);
    assert!(matches!(&reply.pieces[1], WebPiece::Ref(p) if p.url.ends_with("Albula_railway_line")));
    assert_eq!(reply.searched.len(), 2);
    assert!(reply.searched.iter().any(|p| p.date == "2008-07-07"));
    assert_eq!(reply.model, "mistral-medium-latest");
    // Only Mistral searches; other providers are refused before anything is sent.
    let other = endpoint(Connector::OpenAi, &base, Some(KEY));
    assert!(matches!(
        http::web_search(&other, &req, Duration::from_secs(1), &std::sync::atomic::AtomicBool::new(false)),
        Err(AssistantError::Unsupported(_))
    ));
    assert!(wire::parse_web_search("{}").is_err());
}
