use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    time::Instant,
};

#[test]
fn cancellation_interrupts_waiting_for_headers_and_a_stalled_body() {
    for partial_body in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let settings = LlmSettings {
            base_url: format!("http://{}", listener.local_addr().unwrap()),
            model: "local-test".into(),
            timeout_seconds: 300,
            ..Default::default()
        };
        let (ready, entered) = mpsc::channel();
        let (release, resume) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut headers = Vec::new();
            let mut byte = [0];
            while !headers.ends_with(b"\r\n\r\n") {
                socket.read_exact(&mut byte).unwrap();
                headers.push(byte[0]);
            }
            if partial_body {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10000\r\n\r\n{")
                    .unwrap();
                socket.flush().unwrap();
            }
            ready.send(()).unwrap();
            let _ = resume.recv_timeout(Duration::from_secs(5));
        });
        let budget = Budget::new(1);
        let sending_budget = budget.clone();
        let (sent, received) = mpsc::channel();
        let task = std::thread::spawn(move || {
            let result = ChatClient::new(&settings).unwrap().send(
                &json!({"messages":[]}),
                None,
                &sending_budget,
            );
            sent.send(result.map(|_| ())).unwrap();
        });
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        let started = Instant::now();
        budget.cancel.store(true, Ordering::SeqCst);
        let result = received.recv_timeout(Duration::from_secs(2));
        let elapsed = started.elapsed();
        release.send(()).unwrap();
        server.join().unwrap();
        task.join().unwrap();
        assert!(result
            .expect("cancellation waited for the server")
            .unwrap_err()
            .to_string()
            .contains("已取消"));
        assert!(elapsed < Duration::from_secs(2));
        assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
    }
}
