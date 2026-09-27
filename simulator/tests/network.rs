use lingclaw_sdk::{
    capabilities::Capabilities,
    runtime::{Runtime, Store},
};
use std::{
    cell::RefCell,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

struct Server {
    url: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(handler: impl Fn(TcpStream) + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let handler = Arc::new(handler);
        let thread = thread::spawn(move || {
            let mut workers = vec![];
            while !stopped.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let handler = handler.clone();
                        workers.push(thread::spawn(move || handler(stream)));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(e) => panic!("{e}"),
                }
            }
            for worker in workers {
                worker.join().unwrap();
            }
        });
        Self {
            url,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}
fn request(stream: &mut TcpStream) -> Vec<u8> {
    let mut bytes = vec![];
    let mut byte = [0];
    while !bytes.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        bytes.push(byte[0]);
        assert!(bytes.len() < 16384);
    }
    let headers = String::from_utf8_lossy(&bytes).to_lowercase();
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length: "))
        .unwrap_or("0")
        .parse::<usize>()
        .unwrap();
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    bytes.extend(body);
    bytes
}
fn app(code: &str) -> Runtime {
    Runtime::new(&format!("{code}\nfunction on_tick() screen.begin(0); screen.present() end\nfunction on_http_response(id, r) response=r; calls=(calls or 0)+1 end"), "network-test", Capabilities::mini(), Rc::new(RefCell::new(Store::default()))).unwrap()
}
fn wait(runtime: &mut Runtime) {
    let start = Instant::now();
    while runtime.eval::<bool>("return response == nil").unwrap() {
        assert!(
            start.elapsed() < Duration::from_secs(4),
            "HTTP callback did not arrive"
        );
        let tick = Instant::now();
        runtime.tick(20);
        assert!(
            tick.elapsed() < Duration::from_millis(200),
            "tick blocked on I/O"
        );
        assert!(runtime.fault.is_none(), "{:?}", runtime.fault);
        thread::sleep(Duration::from_millis(5));
    }
}
#[test]
fn real_get_post_headers_status_and_binary_body() {
    let (tx, rx) = mpsc::channel();
    let server = Server::new(move |mut stream| {
        tx.send(request(&mut stream)).unwrap();
        stream.write_all(b"HTTP/1.1 418 Teapot\r\nContent-Type: application/octet-stream\r\nContent-Length: 4\r\nConnection: close\r\n\r\n\x00\xffAB").unwrap();
    });
    for method in ["GET", "POST"] {
        let code = if method == "POST" {
            format!(
                "id=http.request({{url='{}',method='POST',headers={{['X-Test']='hello&world'}},body='payload'}})",
                server.url
            )
        } else {
            format!("id=http.get('{}')", server.url)
        };
        let mut runtime = app(&code);
        assert!(runtime.eval::<bool>("return response == nil").unwrap());
        wait(&mut runtime);
        assert_eq!(runtime.eval::<u16>("return response.status").unwrap(), 418);
        assert_eq!(
            runtime
                .eval::<String>("return response.content_type")
                .unwrap(),
            "application/octet-stream"
        );
        assert_eq!(
            &*runtime
                .eval::<mlua::String>("return response.body")
                .unwrap()
                .as_bytes(),
            b"\x00\xffAB"
        );
        let received = String::from_utf8(rx.recv_timeout(Duration::from_secs(1)).unwrap()).unwrap();
        assert!(received.starts_with(method));
        if method == "POST" {
            assert!(received.to_lowercase().contains("x-test: hello&world"));
            assert!(received.ends_with("payload"));
        }
        for _ in 0..3 {
            runtime.tick(20);
        }
        assert_eq!(runtime.eval::<u32>("return calls").unwrap(), 1);
    }
}
#[test]
fn redirects_are_returned_without_following() {
    let (tx, rx) = mpsc::channel();
    let server = Server::new(move |mut stream| {
        request(&mut stream);
        tx.send(()).unwrap();
        stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    });
    let mut runtime = app(&format!("id=http.get('{}')", server.url));
    wait(&mut runtime);
    assert_eq!(runtime.eval::<u16>("return response.status").unwrap(), 302);
    rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(rx.try_recv().is_err());
}
#[test]
fn limits_cover_declared_chunked_and_exact_length_responses() {
    for (reply, expected) in [
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nabcde",
            "response_too_large",
        ),
        (
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n",
            "response_too_large",
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nabcd",
            "abcd",
        ),
    ] {
        let server = Server::new(move |mut stream| {
            request(&mut stream);
            let _ = stream.write_all(reply.as_bytes());
        });
        let mut runtime = app(&format!(
            "id=http.get('{}',{{max_response_bytes=4}})",
            server.url
        ));
        wait(&mut runtime);
        assert_eq!(
            runtime
                .eval::<String>("return response.error and response.error.code or response.body")
                .unwrap(),
            expected
        );
    }
}
#[test]
fn timeouts_cover_headers_and_slow_body_without_blocking_ticks() {
    for headers in [false, true] {
        let server = Server::new(move |mut stream| {
            request(&mut stream);
            if headers {
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\na");
            }
            thread::sleep(Duration::from_millis(700));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        });
        let mut runtime = app(&format!("id=http.get('{}',{{timeout_ms=500}})", server.url));
        wait(&mut runtime);
        assert_eq!(
            runtime
                .eval::<String>("return response.error.code")
                .unwrap(),
            "timeout"
        );
        assert!(runtime.state.borrow().elapsed >= 40);
    }
}
#[test]
fn cancellation_stop_and_drop_close_inflight_connections() {
    for action in ["cancel", "stop", "drop"] {
        let (seen_tx, seen_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        let server = Server::new(move |mut stream| {
            request(&mut stream);
            seen_tx.send(()).unwrap();
            let mut byte = [0];
            let read = stream.read(&mut byte);
            let closed = match read {
                Ok(0) => true,
                Err(e) => matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                        | std::io::ErrorKind::BrokenPipe
                ),
                _ => false,
            };
            closed_tx.send(closed).unwrap();
        });
        let mut runtime = app(&format!("id=http.get('{}')", server.url));
        runtime.tick(20);
        seen_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        match action {
            "cancel" => {
                assert!(
                    runtime
                        .eval::<bool>("return http.cancel(id) and not http.cancel(id)")
                        .unwrap()
                );
            }
            "stop" => runtime.stop(),
            _ => {
                let state = runtime.state.clone();
                drop(runtime);
                assert!(closed_rx.recv_timeout(Duration::from_secs(3)).unwrap());
                drop(state);
                continue;
            }
        }
        assert!(closed_rx.recv_timeout(Duration::from_secs(3)).unwrap());
        for _ in 0..3 {
            runtime.tick(20);
        }
        assert!(runtime.eval::<bool>("return response == nil").unwrap());
    }
}
#[test]
fn refused_connection_is_reported_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let mut runtime = app(&format!("id=http.get('http://{addr}')"));
    wait(&mut runtime);
    assert_eq!(
        runtime
            .eval::<String>("return response.error.code")
            .unwrap(),
        "network_error"
    );
}
#[test]
fn failed_startup_and_cancel_before_tick_do_not_send_requests() {
    let (tx, rx) = mpsc::channel();
    let server = Server::new(move |_| {
        tx.send(()).unwrap();
    });
    let source = format!("id=http.get('{}'); error('startup failure')", server.url);
    assert!(
        Runtime::new(
            &source,
            "failure",
            Capabilities::mini(),
            Rc::new(RefCell::new(Store::default()))
        )
        .is_err()
    );
    let mut runtime = app(&format!(
        "id=http.get('{}'); assert(http.cancel(id))",
        server.url
    ));
    runtime.tick(20);
    assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
}
#[test]
#[ignore = "requires public Internet and trusted TLS certificates"]
fn public_https_uses_certificate_validation() {
    let mut runtime =
        app("id=http.get('https://example.com', {timeout_ms=10000,max_response_bytes=32768})");
    let start = Instant::now();
    while runtime.eval::<bool>("return response == nil").unwrap()
        && start.elapsed() < Duration::from_secs(12)
    {
        runtime.tick(20);
        thread::sleep(Duration::from_millis(10));
    }
    runtime.eval::<()>("assert(response and response.status == 200, response and response.error and response.error.code)").unwrap();
}

#[test]
fn post_preserves_binary_body_and_does_not_mutate_options() {
    let (tx, rx) = mpsc::channel();
    let server = Server::new(move |mut stream| {
        tx.send(request(&mut stream)).unwrap();
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let mut runtime = app(&format!(
        "options={{url='{}',method='POST',body=string.char(0,255,65)}}; id=http.request(options); assert(options.body==string.char(0,255,65))",
        server.url
    ));
    wait(&mut runtime);
    let received = rx.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(received.ends_with(&[0, 255, 65]));
    assert_eq!(runtime.eval::<u16>("return response.status").unwrap(), 204);
}

#[test]
fn custom_concurrency_limit_and_paused_callback_delivery() {
    let server = Server::new(move |mut stream| {
        request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .unwrap();
    });
    let mut caps = Capabilities::mini();
    caps.0["runtime"]["lua_sdk"]["http"]["max_pending"] = 1.into();
    let mut runtime=Runtime::new(&format!("id=http.get('{}'); function on_tick() screen.begin(0); screen.present() end; function on_http_response(_,r) response=r end",server.url),"limit",caps,Rc::new(RefCell::new(Store::default()))).unwrap();
    assert!(runtime.eval::<()>("http.get('http://127.0.0.1')").is_err());
    runtime.tick(20);
    thread::sleep(Duration::from_millis(80));
    // A completed worker cannot call Lua while the runtime is paused.
    assert!(runtime.eval::<bool>("return response==nil").unwrap());
    wait(&mut runtime);
    assert_eq!(
        runtime.eval::<String>("return response.body").unwrap(),
        "ok"
    );
}
