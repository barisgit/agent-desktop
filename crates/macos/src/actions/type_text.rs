use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy,
    StepMechanism,
};

use crate::tree::AXElement;

/// Types by replacing the current selection through `AXSelectedText`.
///
/// Chromium and Electron text inputs accept that write but ignore it, while
/// they do honour `AXValue`. When the insertion left a known value unchanged,
/// the full composed value is written once through `AXValue`. That write is
/// absolute, and the app applies both writes in order, so an insertion that
/// only landed late is overwritten rather than duplicated. Secure fields never
/// take this path.
pub(crate) fn execute_type(
    element: &AXElement,
    text: &str,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<Vec<ActionStep>, AdapterError> {
    let role = text_target_role(element, deadline)?;
    if policy.is_headed() {
        crate::actions::physical_keyboard::type_text(element, text, policy, deadline)?;
        return Ok(vec![
            ActionStep::succeeded("PIDTargetedUnicodeText")
                .with_mechanism(StepMechanism::PhysicalSynthetic)
                .with_verified(false),
        ]);
    }
    if role == "AXSecureTextField" {
        insert_selected_text(element, text, deadline)?;
        return Ok(vec![semantic_step("AXSelectedText")]);
    }

    let before = read_value(element, deadline);
    let selection = crate::tree::attributes::selected_text_range(element, deadline);
    insert_selected_text(element, text, deadline)?;
    let after = read_value(element, deadline);

    let fallback = fallback_value(before.as_deref(), selection, after.as_deref(), text);
    let Some(value) = fallback.filter(|_| value_is_settable(element, deadline)) else {
        return Ok(vec![semantic_step("AXSelectedText")]);
    };
    write_value_after_insertion(element, &value, deadline)?;
    Ok(vec![
        semantic_step("AXSelectedText"),
        semantic_step("AXValue"),
    ])
}

/// Returns the value `AXValue` should receive when an accepted insertion left
/// the field unchanged. Any change, unknown value, or unknown insertion point
/// returns `None`, because the insertion may then have landed.
fn fallback_value(
    before: Option<&str>,
    selection: Option<std::ops::Range<usize>>,
    after: Option<&str>,
    text: &str,
) -> Option<String> {
    let before = before?;
    if after? != before {
        return None;
    }
    let composed = agent_desktop_core::expected_insertion(before, text, selection)?;
    (composed != before).then_some(composed)
}

/// The preceding `AXSelectedText` write was accepted, so a failed fallback
/// cannot report the action as never delivered.
fn write_value_after_insertion(
    element: &AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    crate::actions::ax_helpers::set_ax_string_or_err(element, "AXValue", value, deadline).map_err(
        |error| {
            if error.disposition == DeliverySemantics::not_delivered() {
                error.with_disposition(DeliverySemantics::delivered_unverified())
            } else {
                error
            }
        },
    )
}

fn value_is_settable(element: &AXElement, deadline: Deadline) -> bool {
    matches!(
        crate::actions::ax_helpers::is_attr_settable(element, "AXValue", deadline),
        Ok(true)
    )
}

fn read_value(element: &AXElement, deadline: Deadline) -> Option<String> {
    crate::tree::attributes::copy_string_attr_result(element, "AXValue", deadline)
        .ok()
        .flatten()
}

fn semantic_step(label: &'static str) -> ActionStep {
    ActionStep::succeeded(label)
        .with_mechanism(StepMechanism::SemanticApi)
        .with_verified(false)
}

fn insert_selected_text(
    element: &AXElement,
    text: &str,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    prepare(element, deadline)?;
    write_selected_text(text, |attribute, value| {
        crate::actions::ax_helpers::set_ax_string_or_err(element, attribute, value, deadline)
    })?;
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_details(serde_json::json!({ "operation": "AXSelectedText" }))
            .with_disposition(DeliverySemantics::delivered_unverified()));
    }
    Ok(())
}

fn write_selected_text(
    text: &str,
    write: impl FnOnce(&str, &str) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    write("AXSelectedText", text)
}

fn text_target_role(element: &AXElement, deadline: Deadline) -> Result<String, AdapterError> {
    prepare(element, deadline)?;
    let result = crate::tree::attributes::copy_string_attr_result(element, "AXRole", deadline);
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    let role = result.map_err(|error| {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not read keyboard target role",
        )
        .with_details(serde_json::json!({ "ax_error": error }))
    })?;
    match role {
        Some(role)
            if matches!(
                role.as_str(),
                "AXTextField" | "AXTextArea" | "AXSecureTextField" | "AXComboBox"
            ) =>
        {
            Ok(role)
        }
        _ => Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Type requires a text field, secure text field, or combo box",
        )),
    }
}

fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
    crate::tree::attributes::set_messaging_timeout(element, deadline)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn execute_type(
    _element: &crate::tree::AXElement,
    _text: &str,
    _policy: agent_desktop_core::InteractionPolicy,
    _deadline: Deadline,
) -> Result<Vec<agent_desktop_core::ActionStep>, AdapterError> {
    Err(AdapterError::not_supported("type_text"))
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    #[test]
    fn type_writes_the_current_selection_instead_of_the_whole_value() {
        let observed = RefCell::new(None);
        super::write_selected_text("inserted", |attribute, value| {
            observed.replace(Some((attribute.to_owned(), value.to_owned())));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            observed.into_inner(),
            Some(("AXSelectedText".into(), "inserted".into()))
        );
    }

    #[test]
    fn unchanged_value_falls_back_to_the_composed_value_at_the_selection() {
        assert_eq!(
            super::fallback_value(Some("ab"), Some(1..1), Some("ab"), "X"),
            Some("aXb".into())
        );
        assert_eq!(
            super::fallback_value(Some("a😀b"), Some(1..3), Some("a😀b"), "X"),
            Some("aXb".into())
        );
        assert_eq!(
            super::fallback_value(Some(""), None, Some(""), "new"),
            Some("new".into())
        );
    }

    #[test]
    fn fallback_never_runs_when_the_insertion_may_have_landed_or_evidence_is_missing() {
        let cases = [
            (Some("ab"), Some(1..1), Some("aXb"), "X"),
            (Some("ab"), Some(1..1), Some("something else"), "X"),
            (None, Some(0..0), Some(""), "X"),
            (Some(""), Some(0..0), None, "X"),
            (Some("ab"), None, Some("ab"), "X"),
            (Some("ab"), Some(0..5), Some("ab"), "X"),
            (Some("😀"), Some(1..1), Some("😀"), "X"),
            (Some("ab"), Some(0..2), Some("ab"), "ab"),
            (Some("ab"), Some(1..1), Some("ab"), ""),
        ];
        for (before, range, after, text) in cases {
            assert_eq!(
                super::fallback_value(before, range.clone(), after, text),
                None,
                "{before:?} {range:?} {after:?} {text:?}"
            );
        }
    }
}
