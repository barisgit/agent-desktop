use super::*;
use std::os::unix::net::UnixListener;

#[test]
fn travel_acknowledgement_obeys_remaining_budget() {
    check_acknowledgement(agent_desktop_core::CursorPhase::Travel, false);
}

#[test]
fn drag_requires_acknowledgement_before_tracking_is_considered_ready() {
    check_acknowledgement(agent_desktop_core::CursorPhase::Drag, false);
}

#[test]
fn effect_requires_acknowledgement_before_it_is_considered_delivered() {
    check_acknowledgement(agent_desktop_core::CursorPhase::Effect, false);
}

#[test]
fn rejected_cursor_control_is_not_an_arrival() {
    for (acknowledgement, reason) in [
        (Some(0), "decode"),
        (Some(2), "route"),
        (Some(3), "renderer"),
        (Some(9), "invalid acknowledgement"),
        (None, "did not confirm"),
    ] {
        check_rejection(acknowledgement, reason);
    }
}

fn check_rejection(acknowledgement: Option<u8>, reason: &str) {
    let path = std::path::PathBuf::from(format!("/tmp/cr-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&path).unwrap();
    let receiver = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut payload = Vec::new();
        stream.read_to_end(&mut payload).unwrap();
        if let Some(acknowledgement) = acknowledgement {
            stream.write_all(&[acknowledgement]).unwrap();
        }
    });
    let instruction = agent_desktop_core::CursorOverlayInstruction::new(
        agent_desktop_core::Point {
            x: -1200.0,
            y: -400.0,
        },
        &agent_desktop_core::CursorOverlayConfig::enabled(None, 8).unwrap(),
        false,
    )
    .unwrap();
    let control = CursorOverlayControl::present("run-rejected".into(), instruction);
    let result = send_until(&path, &control, Instant::now() + Duration::from_secs(1));
    receiver.join().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(result.is_err(), "rejection must not confirm arrival");
    assert!(result.unwrap_err().message.contains(reason));
}

fn check_acknowledgement(phase: agent_desktop_core::CursorPhase, accepted: bool) {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::path::PathBuf::from(format!("/tmp/ca-{}-{unique:x}.sock", std::process::id()));
    let result = std::panic::catch_unwind(|| {
        let listener = UnixListener::bind(&path).unwrap();
        let receiver = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut payload = Vec::new();
            stream.read_to_end(&mut payload).unwrap();
            thread::sleep(Duration::from_millis(700));
        });
        let instruction = agent_desktop_core::CursorOverlayInstruction::new(
            agent_desktop_core::Point { x: 10.0, y: 20.0 },
            &agent_desktop_core::CursorOverlayConfig::enabled(None, 8).unwrap(),
            false,
        )
        .unwrap()
        .with_phase(phase);
        let control = CursorOverlayControl::present("run-budget".into(), instruction);
        let started = Instant::now();
        assert_eq!(
            send_until(&path, &control, started + Duration::from_millis(30)).is_ok(),
            accepted
        );
        let elapsed = started.elapsed();
        receiver.join().unwrap();
        assert!(elapsed < Duration::from_millis(500), "{elapsed:?}");
    });
    let _ = std::fs::remove_file(path);
    result.unwrap();
}

#[test]
fn previous_generation_renderer_is_retired_with_a_disable_it_can_decode() {
    let path = std::path::PathBuf::from(format!("/tmp/cg-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    let receiver = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut payload = Vec::new();
        stream.read_to_end(&mut payload).unwrap();
        stream.write_all(&[1]).unwrap();
        payload
    });
    let missing = std::path::PathBuf::from(format!("/tmp/cg-missing-{}.sock", std::process::id()));

    retire(
        [missing, path.clone()],
        "run-upgrade",
        Instant::now() + Duration::from_secs(1),
    );

    let payload = receiver.join().unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&payload).unwrap(),
        serde_json::json!({ "action": "disable", "session_id": "run-upgrade" })
    );
}
