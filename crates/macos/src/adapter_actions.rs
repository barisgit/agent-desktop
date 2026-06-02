use agent_desktop_core::{
    action::Action,
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::{NativeHandle, TreeOptions},
    error::{AdapterError, ErrorCode},
    node::AccessibilityNode,
};

pub(crate) fn execute_action_impl(
    handle: &NativeHandle,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    with_borrowed_ax_element(handle, |element| {
        crate::actions::perform_action(element, &request)
    })
}

pub(crate) fn get_subtree_impl(
    handle: &NativeHandle,
    options: &TreeOptions,
) -> Result<AccessibilityNode, AdapterError> {
    with_borrowed_ax_element(handle, |element| {
        let mut ancestors = rustc_hash::FxHashSet::default();
        let context = crate::tree::TreeBuildContext::empty(options.include_bounds);
        crate::tree::build_subtree(
            element,
            0,
            0,
            options.max_depth,
            &mut ancestors,
            options.skeleton,
            &context,
        )
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::ElementNotFound,
                "Element no longer exists in accessibility tree",
            )
            .with_suggestion("Run 'snapshot' to refresh refs, then retry.")
        })
    })
}

pub(crate) fn execute_ax_only_action_impl(
    handle: &NativeHandle,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    use crate::actions::{chain::ChainContext, chain::execute_chain, chain_defs, discovery};

    with_borrowed_ax_element(handle, |element| {
        let chain = match request.action {
            Action::Click => &chain_defs::CLICK_CHAIN,
            Action::RightClick => &chain_defs::RIGHT_CLICK_CHAIN,
            _ => {
                return Err(AdapterError::new(
                    ErrorCode::ActionNotSupported,
                    "AX-only execution supports only click and right-click",
                ));
            }
        };
        let capabilities = discovery::discover(element);
        let context = ChainContext {
            dynamic_value: None,
            deadline: None,
        };
        execute_chain(element, &capabilities, chain, &context, request.policy)?;
        Ok(ActionResult::new(match request.action {
            Action::RightClick => "right_click",
            _ => "click",
        }))
    })
}

pub(crate) fn with_borrowed_ax_element<T>(
    handle: &NativeHandle,
    function: impl FnOnce(&crate::tree::AXElement) -> T,
) -> T {
    use std::mem::ManuallyDrop;

    let element = ManuallyDrop::new(crate::tree::AXElement(
        handle.as_raw() as accessibility_sys::AXUIElementRef
    ));
    function(&element)
}
