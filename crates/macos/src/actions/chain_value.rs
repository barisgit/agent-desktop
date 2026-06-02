use agent_desktop_core::AdapterError;
use std::time::Instant;

use crate::{
    actions::{ax_helpers, chain_verify},
    tree::AXElement,
};

pub(crate) fn set_dynamic_verified(
    element: &AXElement,
    attr: &str,
    value: &str,
) -> Result<bool, AdapterError> {
    if attr == "AXValue" {
        ax_helpers::set_ax_value_coerced(element, value)?;
    } else {
        ax_helpers::set_ax_string_or_err(element, attr, value)?;
    }
    Ok(chain_verify::dynamic_write_had_effect(
        attr,
        ax_helpers::element_role(element).as_deref(),
        value,
        crate::tree::copy_value_typed(element).as_deref(),
    ))
}

pub(crate) fn increment_to_value(
    element: &AXElement,
    target: &str,
    deadline: Option<Instant>,
) -> Result<bool, AdapterError> {
    const MAX_INCREMENT_STEPS: usize = 1024;

    let target = match finite_target(target) {
        Some(target) => target,
        None => return Ok(false),
    };
    let read =
        || crate::tree::copy_value_typed(element).and_then(|value| value.parse::<f64>().ok());
    let mut current = match read() {
        Some(current) => current,
        None => return Ok(false),
    };
    let actions = ax_helpers::list_ax_actions(element);
    if !actions.iter().any(|action| action == "AXIncrement")
        && !actions.iter().any(|action| action == "AXDecrement")
    {
        return Ok(false);
    }
    let start = current;
    for _ in 0..MAX_INCREMENT_STEPS {
        if (current - target).abs() < 0.5 {
            return Ok(true);
        }
        if deadline.is_some_and(|deadline| Instant::now() > deadline) {
            return Err(chain_verify::increment_deadline_error(
                start, current, target,
            ));
        }
        let action = if current < target {
            "AXIncrement"
        } else {
            "AXDecrement"
        };
        if !ax_helpers::try_ax_action(element, action) {
            break;
        }
        match read() {
            Some(next) if (next - current).abs() >= f64::EPSILON => current = next,
            _ => break,
        }
    }
    if (current - target).abs() < 0.5 {
        return Ok(true);
    }
    if (current - start).abs() >= f64::EPSILON {
        return Err(chain_verify::increment_step_limit_error(
            start, current, target,
        ));
    }
    Ok(false)
}

fn finite_target(target: &str) -> Option<f64> {
    target.parse::<f64>().ok().filter(|value| value.is_finite())
}

pub(crate) fn set_bool_verified(
    element: &AXElement,
    attr: &str,
    value: bool,
) -> Result<bool, AdapterError> {
    Ok(ax_helpers::set_ax_bool_or_err(element, attr, value)?
        && chain_verify::bool_write_had_effect(
            attr,
            value,
            crate::tree::copy_bool_attr(element, attr),
        ))
}

#[cfg(test)]
mod tests {
    use super::finite_target;

    #[test]
    fn finite_target_rejects_non_finite_numbers() {
        assert_eq!(finite_target("42.5"), Some(42.5));
        assert_eq!(finite_target("NaN"), None);
        assert_eq!(finite_target("inf"), None);
        assert_eq!(finite_target("-inf"), None);
        assert_eq!(finite_target("not-a-number"), None);
    }
}
