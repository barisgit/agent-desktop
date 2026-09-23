use super::*;
use agent_desktop_core::{CursorOverlayConfig, ProcessId};

fn target(x: f64) -> CursorOverlayInstruction {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CursorOverlayInstruction::new(Point { x, y: 300.0 }, &config, false)
        .expect("valid instruction")
        .with_window((ProcessId::new(10), "w-42".into()))
}

fn fades_at(state: &mut OverlayState, at: Instant) -> usize {
    let mut fades = 0;
    expire_pose(state, at, || fades += 1);
    fades
}

#[test]
fn enable_and_non_target_controls_do_not_select_an_instruction_to_render() {
    let enable = CursorOverlayControl::enable("run-enable".into(), CursorOverlayStyle::default());
    let non_target = CursorOverlayControl::present("run-enable".into(), {
        let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
        CursorOverlayInstruction::new(Point { x: 400.0, y: 300.0 }, &config, false)
            .expect("valid instruction")
    });
    let target = CursorOverlayControl::present("run-enable".into(), target(400.0));

    assert!(target_instruction(&enable).is_none());
    assert!(target_instruction(&non_target).is_none());
    assert!(target_instruction(&target).is_some());
}

#[test]
fn target_pose_fades_at_three_seconds_and_a_new_target_resets_deadline() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    let mut fades = 0;
    state.record_target_pose(now);

    expire_pose(
        &mut state,
        now + Duration::from_millis(TARGET_POSE_IDLE_MS - 1),
        || fades += 1,
    );
    assert_eq!(fades, 0);

    state.record_target_pose(now + Duration::from_millis(2_000));
    expire_pose(
        &mut state,
        now + Duration::from_millis(TARGET_POSE_IDLE_MS),
        || fades += 1,
    );
    assert_eq!(fades, 0);
    expire_pose(
        &mut state,
        now + Duration::from_millis(2_000 + TARGET_POSE_IDLE_MS),
        || fades += 1,
    );
    assert_eq!(fades, 1);
    expire_pose(
        &mut state,
        now + Duration::from_millis(2_001 + TARGET_POSE_IDLE_MS),
        || fades += 1,
    );
    assert_eq!(fades, 1);
}

#[test]
fn hide_forgets_the_landing_but_keeps_the_idle_deadline() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    let shown = target(400.0);
    let present = CursorOverlayControl::present("run-hide".into(), shown.clone());
    state.record_target_pose(now);
    apply_landing_memory(&present, &mut state, Some(&shown));

    apply_landing_memory(
        &CursorOverlayControl::hide("run-hide".into()),
        &mut state,
        None,
    );

    assert_eq!(state.at, None);
    assert_eq!(
        state.pose_deadline,
        Some(now + Duration::from_millis(TARGET_POSE_IDLE_MS))
    );
    assert_eq!(
        fades_at(
            &mut state,
            now + Duration::from_millis(TARGET_POSE_IDLE_MS - 1)
        ),
        0
    );
    assert_eq!(
        fades_at(&mut state, now + Duration::from_millis(TARGET_POSE_IDLE_MS)),
        1
    );
}

#[test]
fn show_before_or_after_the_deadline_never_extends_it() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    state.record_target_pose(now);
    let show = CursorOverlayControl::show("run-show".into());

    apply_landing_memory(
        &CursorOverlayControl::hide("run-show".into()),
        &mut state,
        None,
    );
    apply_landing_memory(&show, &mut state, None);
    assert_eq!(
        fades_at(&mut state, now + Duration::from_millis(TARGET_POSE_IDLE_MS)),
        1
    );

    apply_landing_memory(&show, &mut state, None);
    assert_eq!(state.pose_deadline, None);
    assert_eq!(
        fades_at(
            &mut state,
            now + Duration::from_millis(2 * TARGET_POSE_IDLE_MS)
        ),
        0
    );
}
