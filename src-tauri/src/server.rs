//! 本地 HTTP 服务：接收 hook 推送的事件，更新状态并广播给前端。
use crate::state::{AppState, Decision, IncomingEvent};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tiny_http::{Header, Method, Response, Server};

pub const PORT: u16 = 51789;

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn cors_header() -> Header {
    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap()
}

fn cors_methods_header() -> Header {
    Header::from_bytes(
        &b"Access-Control-Allow-Methods"[..],
        &b"GET, POST, OPTIONS"[..],
    )
    .unwrap()
}

fn cors_headers_header() -> Header {
    Header::from_bytes(
        &b"Access-Control-Allow-Headers"[..],
        &b"Content-Type"[..],
    )
    .unwrap()
}

fn text_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"text/plain; charset=utf-8"[..]).unwrap()
}

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap()
}

/// 在后台线程启动 HTTP 服务。
/// - POST /event          接收 hook 事件
/// - GET  /decision/{id}  hook 轮询审批结果
/// 启动失败（如端口占用）时返回 Err。
pub fn start(app: AppHandle, state: Arc<Mutex<AppState>>) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{PORT}");
    let server = Server::http(&addr).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::AddrInUse, e.to_string())
    })?;

    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let url = req.url().to_string();
            let method = req.method().clone();

            // OPTIONS — CORS 预检请求
            if method == Method::Options {
                let _ = req.respond(
                    Response::from_string("")
                        .with_status_code(204)
                        .with_header(cors_header())
                        .with_header(cors_methods_header())
                        .with_header(cors_headers_header()),
                );
                continue;
            }

            // POST /event
            if method == Method::Post && url == "/event" {
                // 限制请求体大小为 1MB，防止内存耗尽攻击。
                const MAX_BODY_SIZE: usize = 1024 * 1024;
                let mut body = Vec::new();
                let mut reader = req.as_reader();
                let mut buf = [0u8; 8192];
                let mut read_result: Option<&str> = None;
                loop {
                    match std::io::Read::read(&mut reader, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if body.len() + n > MAX_BODY_SIZE {
                                read_result = Some("body too large");
                                break;
                            }
                            body.extend_from_slice(&buf[..n]);
                        }
                        Err(_) => {
                            read_result = Some("bad body");
                            break;
                        }
                    }
                }
                if let Some(err_msg) = read_result {
                    let status = if err_msg == "body too large" { 413 } else { 400 };
                    let _ =
                        req.respond(Response::from_string(err_msg).with_status_code(status));
                    continue;
                }
                let body = match String::from_utf8(body) {
                    Ok(s) => s,
                    Err(_) => {
                        let _ = req.respond(
                            Response::from_string("invalid utf-8").with_status_code(400),
                        );
                        continue;
                    }
                };
                match serde_json::from_str::<IncomingEvent>(&body) {
                    Ok(ev) => {
                        let updated = {
                            let mut guard = state.lock().unwrap_or_else(|e| e.into_inner());
                            guard.handle(&ev, now_secs())
                        };
                        let _ = app.emit("agent-update", &updated);
                        let _ = req.respond(
                            Response::from_string("ok")
                                .with_header(text_header())
                                .with_header(cors_header()),
                        );
                    }
                    Err(e) => {
                        let _ = req.respond(
                            Response::from_string(format!("invalid json: {e}"))
                                .with_status_code(400),
                        );
                    }
                }
                continue;
            }

            // GET /decision/{id}  — hook 轮询
            if method == Method::Get && url.starts_with("/decision/") {
                let raw = &url["/decision/".len()..];
                // 剥离 query string 和 fragment。
                let decision_id = raw
                    .split(|c| c == '?' || c == '#')
                    .next()
                    .unwrap_or(raw);
                let decision = {
                    let guard = state.lock().unwrap_or_else(|e| e.into_inner());
                    guard.get_decision(decision_id)
                };
                let body = match decision {
                    Decision::Pending => r#"{"decision":"pending"}"#,
                    Decision::Allowed => r#"{"decision":"allowed"}"#,
                    Decision::Denied  => r#"{"decision":"denied"}"#,
                };
                let _ = req.respond(
                    Response::from_string(body)
                        .with_header(json_header())
                        .with_header(cors_header()),
                );
                continue;
            }

            let _ = req.respond(
                Response::from_string("not found")
                    .with_status_code(404)
                    .with_header(cors_header()),
            );
        }
    });

    Ok(())
}
