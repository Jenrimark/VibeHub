//! 本地 HTTP 服务：接收 hook 推送的事件，更新状态并广播给前端。
use crate::state::{AppState, IncomingEvent};
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

/// 在后台线程启动 HTTP 服务。`/event` 接收 POST，其它返回 404。
/// 启动失败（如端口占用）时返回 Err。
pub fn start(app: AppHandle, state: Arc<Mutex<AppState>>) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{PORT}");
    let server = Server::http(&addr).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::AddrInUse, e.to_string())
    })?;

    std::thread::spawn(move || {
        for mut req in server.incoming_requests() {
            let is_event = req.method() == &Method::Post && req.url() == "/event";
            if !is_event {
                let _ = req.respond(Response::from_string("not found").with_status_code(404));
                continue;
            }

            let mut body = String::new();
            if std::io::Read::read_to_string(req.as_reader(), &mut body).is_err() {
                let _ = req.respond(Response::from_string("bad body").with_status_code(400));
                continue;
            }

            match serde_json::from_str::<IncomingEvent>(&body) {
                Ok(ev) => {
                    let updated = {
                        let mut guard = state.lock().unwrap();
                        guard.handle(&ev, now_secs())
                    };
                    // 广播给前端；忽略 emit 错误（前端可能尚未就绪）。
                    let _ = app.emit("agent-update", &updated);
                    let resp = Response::from_string("ok").with_header(
                        Header::from_bytes(&b"Content-Type"[..], &b"text/plain"[..]).unwrap(),
                    );
                    let _ = req.respond(resp);
                }
                Err(e) => {
                    let _ = req.respond(
                        Response::from_string(format!("invalid json: {e}"))
                            .with_status_code(400),
                    );
                }
            }
        }
    });

    Ok(())
}
