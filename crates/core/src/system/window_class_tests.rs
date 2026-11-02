use super::*;
use crate::{ProcessId, WindowState};

fn window(
    id: &str,
    width: f64,
    height: f64,
    accessible: bool,
    visible: Option<bool>,
    minimized: Option<bool>,
) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "t".into(),
        app: "App".into(),
        pid: ProcessId::new(1),
        process_instance: Some("1:1".into()),
        bounds: Some(Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }),
        state: WindowState {
            is_focused: false,
            accessible,
            minimized,
            visible,
        },
    }
}

#[test]
fn full_accessible_onscreen_is_driveable() {
    let w = window("w-1", 800.0, 600.0, true, Some(true), None);
    assert_eq!(classify_window_info(&w), WindowClass::DriveableFullAx);
    assert!(resolve_for_headless_action(w).is_ok());
}

#[test]
fn thumb_width_inaccessible_is_rejectable() {
    let w = window("w-thumb", 160.0, 180.0, false, Some(true), None);
    assert_eq!(classify_window_info(&w), WindowClass::RejectableSmThumb);
    assert!(matches!(
        resolve_for_headless_action(w),
        Err(HeadlessResolveError::SmThumbReject { .. })
    ));
}

#[test]
fn thumb_width_even_if_accessible_is_rejectable() {
    // Codex poison: never bind thumb CG even when AX somehow links.
    let w = window("w-sm", 158.0, 151.0, true, Some(true), None);
    assert_eq!(classify_window_info(&w), WindowClass::RejectableSmThumb);
}

#[test]
fn window_manager_app_icon_thumb_rejected() {
    let w = window("w-66", 66.0, 82.0, false, Some(true), None);
    assert_eq!(classify_window_info(&w), WindowClass::RejectableSmThumb);
    assert!(refuse_rejectable_sm_thumb(&w).is_err());
}

#[test]
fn headed_screenshot_refuse_returns_policy_denied() {
    let w = window("w-sm", 120.0, 120.0, false, Some(true), None);
    let err = refuse_rejectable_sm_thumb(&w).expect_err("thumb");
    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert_eq!(
        err.details.as_ref().unwrap()["kind"],
        "rejectable_sm_thumb"
    );
}

#[test]
fn driveable_full_ax_target_has_no_public_cg_ctor() {
    let w = window("w-2", 1024.0, 768.0, true, Some(true), None);
    let target = resolve_for_headless_action(w).expect("driveable");
    assert_eq!(target.id(), "w-2");
}

#[test]
fn minimized_class() {
    let w = window("w-m", 800.0, 600.0, true, Some(false), Some(true));
    assert_eq!(classify_window_info(&w), WindowClass::Minimized);
}

#[test]
fn inaccessible_offscreen_non_thumb_is_off_space_unknown() {
    let w = window("w-off", 800.0, 600.0, false, Some(false), None);
    assert_eq!(classify_window_info(&w), WindowClass::OffSpaceUnknown);
}

#[test]
fn cg_thumb_with_ax_full_frame_mismatch_is_rejectable() {
    let mut w = window("w-mis", 160.0, 170.0, true, Some(true), None);
    let facts = ClassifyFacts::from_window_info(&w).with_ax_bounds(Some(Rect {
        x: 0.0,
        y: 0.0,
        width: 1200.0,
        height: 800.0,
    }));
    assert_eq!(
        classify_window(facts, &ClassifyThresholds::default()),
        WindowClass::RejectableSmThumb
    );
    // Ensure raw window (without ax) still rejects on cg thumb alone.
    w.state.accessible = true;
    assert_eq!(classify_window_info(&w), WindowClass::RejectableSmThumb);
}

#[test]
fn zero_size_bounds_are_thumb() {
    let w = window("w-0", 0.0, 0.0, false, Some(false), None);
    assert_eq!(classify_window_info(&w), WindowClass::RejectableSmThumb);
}

#[test]
fn accessible_missing_bounds_is_driveable_not_thumb() {
    // list-windows / fixture rows may omit CG bounds; absence ≠ SM thumb.
    let w = WindowInfo {
        id: "w-nobounds".into(),
        title: "t".into(),
        app: "App".into(),
        pid: ProcessId::new(1),
        process_instance: Some("1:1".into()),
        bounds: None,
        state: WindowState {
            is_focused: true,
            accessible: true,
            minimized: None,
            visible: None,
        },
    };
    assert_eq!(classify_window_info(&w), WindowClass::DriveableFullAx);
    assert!(resolve_for_headless_action(w).is_ok());
}

#[test]
fn inaccessible_full_size_onscreen_is_off_space_not_thumb() {
    let w = window("w-inacc", 800.0, 600.0, false, Some(true), None);
    assert_eq!(classify_window_info(&w), WindowClass::OffSpaceUnknown);
    assert!(refuse_rejectable_sm_thumb(&w).is_ok());
}
