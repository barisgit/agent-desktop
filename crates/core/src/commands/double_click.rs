use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{RefClickArgs, execute_ref_action_with_context},
    context::CommandContext,
    error::AppError,
};
use serde_json::Value;

pub fn execute(
    args: RefClickArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let request = context.request(Action::DoubleClick, args.policy);
    execute_ref_action_with_context(args.into(), adapter, request, context)
}
