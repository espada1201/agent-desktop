//! WindowClass at the list-windows / resolve boundary.
//!
//! Inventory-first discriminant: callers must not treat raw CG thumb bounds as
//! driveable. `DriveableFullAxTarget` has no public constructor from CG bounds.

use serde::{Deserialize, Serialize};

use crate::{AdapterError, ErrorCode, Rect, WindowInfo};

/// Classification at list-windows boundary. Orthogonal to InteractionPolicy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowClass {
    /// Full AX window; legal headless snapshot/click target.
    DriveableFullAx,
    /// Stage Manager (or similar) strip thumb — reject for drive / headed / SCK.
    RejectableSmThumb,
    /// CG row visible:false / not minimized and no full AX yet — not “closed”.
    OffSpaceUnknown,
    /// AX-confirmed minimized; existing retain path.
    Minimized,
}

/// Thresholds for thumb vs driveable geometry. Owned once; not hand-synced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassifyThresholds {
    pub thumb_max_width_px: f64,
    pub min_driveable_edge_px: f64,
}

impl Default for ClassifyThresholds {
    fn default() -> Self {
        Self {
            thumb_max_width_px: 160.0,
            min_driveable_edge_px: 200.0,
        }
    }
}

/// Facts the classifier needs. Prefer ax_bounds for geometry when present.
#[derive(Clone, Copy, Debug)]
pub struct ClassifyFacts {
    pub cg_bounds: Option<Rect>,
    pub ax_bounds: Option<Rect>,
    pub accessible: bool,
    pub minimized: Option<bool>,
    pub visible_onscreen: Option<bool>,
}

impl ClassifyFacts {
    pub fn from_window_info(info: &WindowInfo) -> Self {
        Self {
            cg_bounds: info.bounds,
            ax_bounds: None,
            accessible: info.state.accessible,
            minimized: info.state.minimized,
            visible_onscreen: info.state.visible,
        }
    }

    pub fn with_ax_bounds(mut self, ax_bounds: Option<Rect>) -> Self {
        self.ax_bounds = ax_bounds;
        self
    }
}

/// Sole classification entry (pure). Call after narrow_to_real_windows.
///
/// `RejectableSmThumb` requires *positive* thumb evidence (thumbnail-scale CG,
/// or cg≪ax mismatch). Missing bounds is not a thumb — fixtures and incomplete
/// inventory rows that are AX-accessible stay DriveableFullAx so headless
/// resolve does not poison normal windows.
pub fn classify_window(facts: ClassifyFacts, thresholds: &ClassifyThresholds) -> WindowClass {
    if facts.minimized == Some(true) {
        return WindowClass::Minimized;
    }

    let cg_thumb = facts
        .cg_bounds
        .is_some_and(|bounds| thumb_scale(bounds, thresholds));
    let ax_driveable = facts
        .ax_bounds
        .is_some_and(|bounds| driveable_scale(bounds, thresholds));
    let cg_driveable = facts
        .cg_bounds
        .is_some_and(|bounds| driveable_scale(bounds, thresholds));
    let cg_ax_mismatch = match (facts.cg_bounds, facts.ax_bounds) {
        (Some(cg), Some(ax)) if thumb_scale(cg, thresholds) && driveable_scale(ax, thresholds) => {
            true
        }
        _ => false,
    };

    // Positive thumb evidence always wins (Codex SCK poison guard).
    if cg_thumb || cg_ax_mismatch {
        return WindowClass::RejectableSmThumb;
    }

    if !facts.accessible {
        // Inaccessible without thumb geometry: not driveable, but not an SM thumb.
        return WindowClass::OffSpaceUnknown;
    }

    if ax_driveable || cg_driveable {
        if facts.visible_onscreen == Some(false) && facts.minimized != Some(true) {
            // Rare until #415; still encode the class rather than WINDOW_NOT_FOUND.
            return WindowClass::OffSpaceUnknown;
        }
        return WindowClass::DriveableFullAx;
    }

    // Accessible, no thumb evidence, no driveable-scale geometry:
    // - bounds missing → trust inventory/AX accessibility (test fixtures + sparse rows)
    // - visible:false → off-space
    // - otherwise middle-band / ambiguous → fail closed as RejectableSmThumb
    if facts.cg_bounds.is_none() && facts.ax_bounds.is_none() {
        return WindowClass::DriveableFullAx;
    }

    if facts.visible_onscreen == Some(false) {
        return WindowClass::OffSpaceUnknown;
    }

    WindowClass::RejectableSmThumb
}

pub fn classify_window_info(info: &WindowInfo) -> WindowClass {
    classify_window(ClassifyFacts::from_window_info(info), &ClassifyThresholds::default())
}

fn thumb_scale(bounds: Rect, t: &ClassifyThresholds) -> bool {
    if !(bounds.width.is_finite() && bounds.height.is_finite()) {
        return true;
    }
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return true;
    }
    bounds.width <= t.thumb_max_width_px
        || (bounds.width < t.min_driveable_edge_px && bounds.height < t.min_driveable_edge_px)
}

fn driveable_scale(bounds: Rect, t: &ClassifyThresholds) -> bool {
    bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width >= t.min_driveable_edge_px
        && bounds.height >= t.min_driveable_edge_px
}

/// Newtyped headless target — cannot construct from raw CG thumb bounds.
#[derive(Clone, Debug)]
pub struct DriveableFullAxTarget {
    id: String,
    pid: crate::ProcessId,
    info: WindowInfo,
}

impl DriveableFullAxTarget {
    /// Private construction path: only after WindowClass::DriveableFullAx.
    pub(crate) fn from_driveable(info: WindowInfo) -> Self {
        Self {
            id: info.id.clone(),
            pid: info.pid,
            info,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn pid(&self) -> crate::ProcessId {
        self.pid
    }

    pub fn window_info(&self) -> &WindowInfo {
        &self.info
    }

    pub fn into_window_info(self) -> WindowInfo {
        self.info
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadlessResolveError {
    SmThumbReject { id: String },
    OffSpaceUnknown { id: String },
    MinimizedNotDriveable { id: String },
    NotAccessible { id: String },
}

impl HeadlessResolveError {
    pub fn window_id(&self) -> &str {
        match self {
            Self::SmThumbReject { id }
            | Self::OffSpaceUnknown { id }
            | Self::MinimizedNotDriveable { id }
            | Self::NotAccessible { id } => id,
        }
    }

    pub fn into_adapter_error(self) -> AdapterError {
        match self {
            Self::SmThumbReject { id } => sm_thumb_reject_error(&id),
            Self::OffSpaceUnknown { id } => AdapterError::new(
                ErrorCode::PolicyDenied,
                format!(
                    "Window '{id}' is OffSpaceUnknown — not a DriveableFullAx headless target"
                ),
            )
            .with_suggestion(
                "Stay on the current Space, or vendor offspace remote-token (#415) if needed.",
            )
            .with_details(serde_json::json!({
                "kind": "off_space_unknown",
                "window_id": id,
                "window_class": "off_space_unknown",
            })),
            Self::MinimizedNotDriveable { id } => AdapterError::new(
                ErrorCode::PolicyDenied,
                format!("Window '{id}' is minimized and not a DriveableFullAx headless target"),
            )
            .with_details(serde_json::json!({
                "kind": "minimized_not_driveable",
                "window_id": id,
                "window_class": "minimized",
            })),
            Self::NotAccessible { id } => AdapterError::new(
                ErrorCode::ActionNotSupported,
                format!("Window '{id}' is not accessible for headless AX actions"),
            )
            .with_details(serde_json::json!({
                "kind": "not_accessible",
                "window_id": id,
            })),
        }
    }
}

/// Typed Stage Manager thumbnail reject for headed / screenshot / SCK bind.
pub fn sm_thumb_reject_error(window_id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::PolicyDenied,
        format!(
            "Window '{window_id}' is a RejectableSmThumb — refuse headed/screenshot bind to CG thumb bounds"
        ),
    )
    .with_suggestion(
        "Use a DriveableFullAx window (full AX geometry). Do not bind Stage Manager strip thumbnails.",
    )
    .with_details(serde_json::json!({
        "kind": "rejectable_sm_thumb",
        "window_id": window_id,
        "window_class": "rejectable_sm_thumb",
    }))
}

/// Resolve for headless act: ONLY Ok for DriveableFullAx.
pub fn resolve_for_headless_action(
    info: WindowInfo,
) -> Result<DriveableFullAxTarget, HeadlessResolveError> {
    resolve_for_headless_action_with(info, None, &ClassifyThresholds::default())
}

pub fn resolve_for_headless_action_with(
    info: WindowInfo,
    ax_bounds: Option<Rect>,
    thresholds: &ClassifyThresholds,
) -> Result<DriveableFullAxTarget, HeadlessResolveError> {
    let id = info.id.clone();
    let facts = ClassifyFacts::from_window_info(&info).with_ax_bounds(ax_bounds);
    match classify_window(facts, thresholds) {
        WindowClass::DriveableFullAx => Ok(DriveableFullAxTarget::from_driveable(info)),
        WindowClass::RejectableSmThumb => Err(HeadlessResolveError::SmThumbReject { id }),
        WindowClass::OffSpaceUnknown => Err(HeadlessResolveError::OffSpaceUnknown { id }),
        WindowClass::Minimized => Err(HeadlessResolveError::MinimizedNotDriveable { id }),
    }
}

/// Headed / screenshot / SCK must refuse RejectableSmThumb before binding cg_bounds.
pub fn refuse_rejectable_sm_thumb(info: &WindowInfo) -> Result<(), AdapterError> {
    refuse_rejectable_sm_thumb_with(info, None, &ClassifyThresholds::default())
}

pub fn refuse_rejectable_sm_thumb_with(
    info: &WindowInfo,
    ax_bounds: Option<Rect>,
    thresholds: &ClassifyThresholds,
) -> Result<(), AdapterError> {
    let facts = ClassifyFacts::from_window_info(info).with_ax_bounds(ax_bounds);
    match classify_window(facts, thresholds) {
        WindowClass::RejectableSmThumb => Err(sm_thumb_reject_error(&info.id)),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "window_class_tests.rs"]
mod tests;
