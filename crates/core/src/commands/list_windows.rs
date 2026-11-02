use crate::{
    AppError, WindowClass, classify_window_info,
    adapter::{PlatformAdapter, WindowFilter},
};
use serde_json::{Value, json};

pub struct ListWindowsArgs {
    pub app: Option<String>,
    pub timeout_ms: Option<u64>,
}

pub fn execute(args: ListWindowsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: args.app,
    };
    let windows = adapter.list_windows(
        &filter,
        crate::Deadline::after(
            args.timeout_ms
                .unwrap_or(crate::DEFAULT_OPERATION_TIMEOUT_MS),
        )?,
    )?;
    let classified = windows
        .into_iter()
        .map(|window| {
            let window_class = classify_window_info(&window);
            let mut value = serde_json::to_value(&window)?;
            if let Some(object) = value.as_object_mut() {
                object.insert(
                    "window_class".to_string(),
                    json!(window_class_label(window_class)),
                );
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    Ok(Value::Array(classified))
}

fn window_class_label(class: WindowClass) -> &'static str {
    match class {
        WindowClass::DriveableFullAx => "driveable_full_ax",
        WindowClass::RejectableSmThumb => "rejectable_sm_thumb",
        WindowClass::OffSpaceUnknown => "off_space_unknown",
        WindowClass::Minimized => "minimized",
    }
}
