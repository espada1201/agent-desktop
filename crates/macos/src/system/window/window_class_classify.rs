//! Stage Manager / Spaces window classification at the inventory boundary.
//!
//! Sole owner of DriveableFullAx vs RejectableSmThumb vs OffSpaceUnknown vs
//! Minimized for macOS inventory consumers. Does not change
//! `narrow_to_real_windows` — classifier sits beside it.
//!
//! SM thumbnail reject guard (Codex SCK lesson): never bind headed hit-test /
//! screenshot / SCK to thumbnail-scale `cg_bounds` (width ≲ 160 or both edges
//! below the driveable floor). Prefer AX geometry when present; fail closed
//! to RejectableSmThumb when inaccessible or thumb-scale.

use agent_desktop_core::{
    ClassifyFacts, ClassifyThresholds, Rect, WindowClass, WindowInfo, classify_window,
};

#[allow(dead_code)] // wired via core list_windows classify; kept for AX-enriched path
pub(crate) fn classify_listed_window(info: &WindowInfo) -> WindowClass {
    classify_window(
        ClassifyFacts::from_window_info(info),
        &ClassifyThresholds::default(),
    )
}

#[allow(dead_code)]
pub(crate) fn classify_listed_window_with_ax(
    info: &WindowInfo,
    ax_bounds: Option<Rect>,
) -> WindowClass {
    classify_window(
        ClassifyFacts::from_window_info(info).with_ax_bounds(ax_bounds),
        &ClassifyThresholds::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{ProcessId, WindowState};

    fn win(width: f64, height: f64, accessible: bool) -> WindowInfo {
        WindowInfo {
            id: "w-1".into(),
            title: "t".into(),
            app: "App".into(),
            pid: ProcessId::new(1),
            process_instance: Some("1:1".into()),
            bounds: Some(Rect {
                x: 10.0,
                y: 10.0,
                width,
                height,
            }),
            state: WindowState {
                is_focused: false,
                accessible,
                minimized: None,
                visible: Some(true),
            },
        }
    }

    #[test]
    fn sm_app_icon_window_rejected() {
        assert_eq!(
            classify_listed_window(&win(66.0, 82.0, false)),
            WindowClass::RejectableSmThumb
        );
    }

    #[test]
    fn full_ax_driveable() {
        assert_eq!(
            classify_listed_window(&win(1612.0, 954.0, true)),
            WindowClass::DriveableFullAx
        );
    }
}
