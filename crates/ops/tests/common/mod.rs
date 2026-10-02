//! Общий пробный HTTP-сервер для оракулов транспорта алертов (`red_ops_transport_redaction.rs`
//! — F-9, `red_ops_transport_redirect.rs` — F-11).
//!
//! Зачем отдельный харнесс, а не `assert!` на результат функции: оба блокера третьего круга
//! ревью (`research/reviews/R-009-alerting-rev3.md`) живут НЕ в форматировании строки, а в том,
//! **что реально уходит в сеть и что реально приходит обратно**. F-9 — недоверенное ТЕЛО
//! ответа; F-11 — недоверенный `Location`, из-за которого клиент сам отправляет секрет на
//! чужой хост. Проверять такое можно только настоящим сокетом: сервер записывает СЫРОЙ текст
//! каждого полученного запроса (строка запроса + все заголовки + тело) и отдаёт СЦЕНАРНЫЙ
//! ответ, который в реальной жизни отдаёт прокси/CDN/captive-portal/неверно настроенный
//! `TELEGRAM_API_BASE`.
//!
//! Ключевое свойство для F-11: сервер, которого ОБЯЗАНЫ не тронуть, обязан уметь сказать
//! «ко мне никто не приходил» — [`ProbeServer::hits`] == 0 после [`ProbeServer::settle`].
//! Без `settle` «ноль обращений» означало бы просто «не успели», и оракул был бы декоративным.

#![allow(dead_code)] // модуль общий для нескольких test-таргетов; каждый использует своё подмножество

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Локальный HTTP-сервер-однодневка: пишет в лог сырой текст каждого запроса и отвечает тем,
/// что вернул `responder`. Живёт, пока жив объект (`Drop` гасит поток).
pub struct ProbeServer {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl ProbeServer {
    /// `responder(сырой_запрос) -> сырой_HTTP_ответ`. Обрабатывает произвольное число
    /// соединений подряд (бинарь `ops-watchdog` за один прогон шлёт НЕСКОЛЬКО алертов —
    /// множественность из чек-листа `.claude/rules/testing.md`).
    pub fn start<F>(responder: F) -> Self
    where
        F: Fn(&str) -> String + Send + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind probe server");
        let addr = listener.local_addr().expect("probe server addr");
        listener
            .set_nonblocking(true)
            .expect("probe listener nonblocking");

        let requests = Arc::new(Mutex::new(Vec::<String>::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let (log, stop) = (Arc::clone(&requests), Arc::clone(&shutdown));

        let handle = std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((sock, _)) => {
                        let raw = read_request(&sock);
                        log.lock().expect("probe log poisoned").push(raw.clone());
                        let mut sock = sock;
                        let _ = sock.write_all(responder(&raw).as_bytes());
                        let _ = sock.flush();
                        let _ = sock.shutdown(std::net::Shutdown::Write);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            addr,
            requests,
            shutdown,
            handle: Some(handle),
        }
    }

    /// Базовый URL для `TELEGRAM_API_BASE` / `with_credentials_and_endpoint`.
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Сырой текст всех полученных запросов (строка запроса + заголовки + тело).
    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("probe log poisoned").clone()
    }

    pub fn hits(&self) -> usize {
        self.requests.lock().expect("probe log poisoned").len()
    }

    /// Дать запоздавшему запросу шанс приехать. Обязательно ПЕРЕД утверждением
    /// «ко мне никто не приходил» — иначе оракул проверяет скорость, а не инвариант.
    pub fn settle(&self) {
        std::thread::sleep(Duration::from_millis(300));
    }
}

impl Drop for ProbeServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Сырой HTTP/1.1-ответ. `Connection: close` — сервер закрывает соединение после ответа,
/// поэтому клиент не пытается переиспользовать мёртвый keep-alive сокет (иначе тест флаки).
pub fn raw_response(status_line: &str, headers: &[(&str, &str)], body: &str) -> String {
    let mut out = format!("HTTP/1.1 {status_line}\r\n");
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len() // строка ASCII/UTF-8 — len() уже в байтах, как требует Content-Length
    ));
    out
}

/// Первая строка запроса (`POST /bot<token>/sendMessage HTTP/1.1`) — то самое место, где
/// живёт секрет и которое «дружелюбный» прокси возвращает эхом в теле ошибки.
pub fn request_line(raw: &str) -> &str {
    raw.lines().next().unwrap_or("")
}

/// Значение заголовка (регистронезависимо) из сырого запроса.
pub fn header_value<'a>(raw: &'a str, name: &str) -> Option<&'a str> {
    raw.lines()
        .take_while(|l| !l.trim().is_empty())
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.trim().eq_ignore_ascii_case(name) {
                Some(v.trim())
            } else {
                None
            }
        })
}

fn read_request(sock: &TcpStream) -> String {
    let mut sock = sock.try_clone().expect("probe socket clone");
    // Сокет, принятый неблокирующим листенером, может унаследовать O_NONBLOCK — возвращаем
    // блокирующий режим явно и ограничиваем ожидание, чтобы поток сервера не завис навсегда.
    let _ = sock.set_nonblocking(false);
    let _ = sock.set_read_timeout(Some(Duration::from_millis(1000)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match sock.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if request_is_complete(&buf) {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Заголовки закончились И тело получено целиком (иначе ответ сервера уедет раньше, чем
/// клиент дописал тело, — RST и флаки-тест вместо оракула).
fn request_is_complete(buf: &[u8]) -> bool {
    let text = String::from_utf8_lossy(buf);
    let Some(header_end) = text.find("\r\n\r\n") else {
        return false;
    };
    let body_start = header_end + 4;
    let content_length = text
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())?
        })
        .unwrap_or(0);
    buf.len() >= body_start + content_length
}
