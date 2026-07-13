use clap::{Parser, ValueEnum};
use serde::Deserialize;

fn default_scroll_amount() -> u32 {
    3
}

fn headless_policy_default() -> InteractionPolicyArg {
    InteractionPolicyArg::Headless
}

fn default_mouse_button() -> String {
    "left".to_string()
}

fn default_mouse_click_count() -> u32 {
    1
}

fn default_scroll_direction() -> String {
    "down".to_string()
}

#[derive(ValueEnum, Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum InteractionPolicyArg {
    #[default]
    Physical,
    FocusFallback,
    Headless,
}

impl InteractionPolicyArg {
    pub(crate) fn to_core(self) -> agent_desktop_core::InteractionPolicy {
        use agent_desktop_core::InteractionPolicy;
        match self {
            Self::Physical => InteractionPolicy::headed(),
            Self::FocusFallback => InteractionPolicy::focus_fallback(),
            Self::Headless => InteractionPolicy::headless(),
        }
    }
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long = "window-id",
        help = "Accepted for script ergonomics; refs already identify their window"
    )]
    #[serde(default, rename = "window-id", alias = "window_id")]
    pub window_id: Option<String>,
    #[arg(value_name = "TEXT", allow_hyphen_values = true, help = "Text to type")]
    pub text: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetValueArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long = "window-id",
        help = "Accepted for script ergonomics; refs already identify their window"
    )]
    #[serde(default, rename = "window-id", alias = "window_id")]
    pub window_id: Option<String>,
    #[arg(
        value_name = "VALUE",
        allow_hyphen_values = true,
        help = "Value to set"
    )]
    pub value: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(value_name = "VALUE", help = "Option to select")]
    pub value: String,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScrollArgs {
    #[arg(value_name = "REF", help = "Element ref from snapshot (@e1, @e2 ...)")]
    pub ref_id: String,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(
        long,
        default_value = "down",
        help = "Direction: up, down, left, right"
    )]
    #[serde(default = "default_scroll_direction")]
    pub direction: String,
    #[arg(long, default_value = "3", help = "Number of scroll units")]
    #[serde(default = "default_scroll_amount")]
    pub amount: u32,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PressArgs {
    #[arg(
        value_name = "COMBO",
        help = "Key combo: return, escape, cmd+c, shift+tab ..."
    )]
    pub combo: String,
    #[arg(
        value_name = "REF",
        help = "Element ref to deliver the press to; overrides app and pid targets"
    )]
    #[serde(default, rename = "ref", alias = "ref_id")]
    pub ref_id: Option<String>,
    #[arg(long, help = "Snapshot ID returned by snapshot; omit to use latest")]
    #[serde(default)]
    pub snapshot: Option<String>,
    #[arg(
        long = "window-id",
        help = "Target a specific window id for key delivery",
        conflicts_with_all = ["app", "target_app", "target_pid"]
    )]
    #[serde(default, rename = "window-id", alias = "window_id")]
    pub window_id: Option<String>,
    #[arg(long, help = "Target application name (focuses app before pressing)")]
    pub app: Option<String>,
    #[arg(
        long = "target-app",
        help = "Target application name for headless key delivery",
        conflicts_with_all = ["app", "target_pid"]
    )]
    #[serde(default, rename = "target-app", alias = "target_app")]
    pub target_app: Option<String>,
    #[arg(
        long = "target-pid",
        help = "Target application PID for headless key delivery",
        conflicts_with_all = ["app", "target_app"]
    )]
    #[serde(default, rename = "target-pid", alias = "target_pid")]
    pub target_pid: Option<i32>,
    #[arg(
        long,
        value_enum,
        default_value_t = InteractionPolicyArg::Headless,
        help = "Interaction policy: headless (default), focus-fallback, physical"
    )]
    #[serde(default = "headless_policy_default")]
    pub policy: InteractionPolicyArg,
    #[arg(
        long,
        help = "Send the combo even if the adapter flags it as a dangerous shortcut"
    )]
    #[serde(default)]
    pub force: bool,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeyComboArgs {
    #[arg(
        value_name = "COMBO",
        help = "Key or modifier to hold/release: shift, cmd, ctrl ..."
    )]
    pub combo: String,
    #[arg(
        long,
        help = "Send the combo even if the adapter flags it as a dangerous shortcut"
    )]
    #[serde(default)]
    pub force: bool,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HoverArgs {
    #[arg(
        value_name = "REF",
        help = "Element ref to hover over; requires --headed"
    )]
    pub ref_id: Option<String>,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: Option<String>,
    #[arg(long, help = "Hold hover position for N milliseconds")]
    pub duration: Option<u64>,
    #[arg(
        long,
        value_enum,
        default_value_t = InteractionPolicyArg::Physical,
        help = "Interaction policy: physical (default), focus-fallback, headless"
    )]
    #[serde(default)]
    pub policy: InteractionPolicyArg,
    #[arg(
        long = "target-app",
        help = "Target application name for headless event routing",
        conflicts_with = "target_pid"
    )]
    #[serde(default, rename = "target-app", alias = "target_app")]
    pub target_app: Option<String>,
    #[arg(
        long = "target-pid",
        help = "Target application PID for headless event routing",
        conflicts_with = "target_app"
    )]
    #[serde(default, rename = "target-pid", alias = "target_pid")]
    pub target_pid: Option<i32>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DragCliArgs {
    #[arg(long, help = "Source element ref; requires --headed")]
    pub from: Option<String>,
    #[arg(
        long,
        name = "from-xy",
        help = "Source coordinates as x,y; requires --headed"
    )]
    pub from_xy: Option<String>,
    #[arg(long, help = "Destination element ref; requires --headed")]
    pub to: Option<String>,
    #[arg(
        long,
        name = "to-xy",
        help = "Destination coordinates as x,y; requires --headed"
    )]
    pub to_xy: Option<String>,
    #[arg(
        long,
        value_name = "SNAPSHOT_ID",
        help = "Snapshot ID returned by snapshot; omit to use active session latest"
    )]
    pub snapshot: Option<String>,
    #[arg(long, help = "Drag duration in milliseconds")]
    pub duration: Option<u64>,
    #[arg(
        long = "drop-delay",
        value_name = "MS",
        help = "Hold over the destination this many ms before releasing, so the drop target activates (macOS); default 500"
    )]
    pub drop_delay: Option<u64>,
    #[arg(
        long,
        value_enum,
        default_value_t = InteractionPolicyArg::Physical,
        help = "Interaction policy: physical (default), focus-fallback, headless"
    )]
    #[serde(default)]
    pub policy: InteractionPolicyArg,
    #[arg(
        long = "target-app",
        help = "Target application name for headless event routing",
        conflicts_with = "target_pid"
    )]
    #[serde(default, rename = "target-app", alias = "target_app")]
    pub target_app: Option<String>,
    #[arg(
        long = "target-pid",
        help = "Target application PID for headless event routing",
        conflicts_with = "target_app"
    )]
    #[serde(default, rename = "target-pid", alias = "target_pid")]
    pub target_pid: Option<i32>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseMoveArgs {
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: String,
    #[arg(
        long,
        value_enum,
        default_value_t = InteractionPolicyArg::Physical,
        help = "Interaction policy: physical (default), focus-fallback, headless"
    )]
    #[serde(default)]
    pub policy: InteractionPolicyArg,
    #[arg(
        long = "target-app",
        help = "Target application name for headless event routing",
        conflicts_with = "target_pid"
    )]
    #[serde(default, rename = "target-app", alias = "target_app")]
    pub target_app: Option<String>,
    #[arg(
        long = "target-pid",
        help = "Target application PID for headless event routing",
        conflicts_with = "target_app"
    )]
    #[serde(default, rename = "target-pid", alias = "target_pid")]
    pub target_pid: Option<i32>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MouseClickArgs {
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: String,
    #[arg(
        long,
        default_value = "left",
        help = "Mouse button: left, right, middle"
    )]
    #[serde(default = "default_mouse_button")]
    pub button: String,
    #[arg(long, default_value = "1", help = "Number of clicks")]
    #[serde(default = "default_mouse_click_count")]
    pub count: u32,
    #[arg(
        long,
        value_enum,
        default_value_t = InteractionPolicyArg::Headless,
        help = "Interaction policy: headless (default), focus-fallback, physical"
    )]
    #[serde(default = "headless_policy_default")]
    pub policy: InteractionPolicyArg,
    #[arg(
        long = "target-app",
        help = "Target application name for headless event routing",
        conflicts_with = "target_pid"
    )]
    #[serde(default, rename = "target-app", alias = "target_app")]
    pub target_app: Option<String>,
    #[arg(
        long = "target-pid",
        help = "Target application PID for headless event routing",
        conflicts_with = "target_app"
    )]
    #[serde(default, rename = "target-pid", alias = "target_pid")]
    pub target_pid: Option<i32>,
}

#[derive(Parser, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MousePointArgs {
    #[arg(long, help = "Absolute coordinates as x,y; requires --headed")]
    pub xy: String,
    #[arg(
        long,
        default_value = "left",
        help = "Mouse button: left, right, middle"
    )]
    #[serde(default = "default_mouse_button")]
    pub button: String,
    #[arg(
        long,
        value_enum,
        default_value_t = InteractionPolicyArg::Physical,
        help = "Interaction policy: physical (default), focus-fallback, headless"
    )]
    #[serde(default)]
    pub policy: InteractionPolicyArg,
    #[arg(
        long = "target-app",
        help = "Target application name for headless event routing",
        conflicts_with = "target_pid"
    )]
    #[serde(default, rename = "target-app", alias = "target_app")]
    pub target_app: Option<String>,
    #[arg(
        long = "target-pid",
        help = "Target application PID for headless event routing",
        conflicts_with = "target_app"
    )]
    #[serde(default, rename = "target-pid", alias = "target_pid")]
    pub target_pid: Option<i32>,
}
