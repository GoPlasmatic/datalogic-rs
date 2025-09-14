use serde_json::Value;

use crate::{CompiledNode, ContextStack, DataLogic, Error, Result};

/// Try operator function - catches errors and provides fallback values
#[inline]
pub fn evaluate_try(
    args: &[CompiledNode],
    context: &mut ContextStack,
    engine: &DataLogic,
) -> Result<Value> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(
            "try requires at least one argument".to_string(),
        ));
    }

    let mut last_error: Option<Error> = None;

    // Try each argument in order until one succeeds
    for (i, arg) in args.iter().enumerate() {
        // Special handling for the last argument - it can access error context
        if i == args.len() - 1 && i > 0 {
            // This is the last argument and there was at least one error before
            if let Some(ref err) = last_error {
                if let Error::Thrown(error_obj) = err {
                    // Push error context for the last argument
                    context.push(error_obj.clone());
                    match engine.evaluate_node(arg, context) {
                        Ok(result) => {
                            context.pop();
                            return Ok(result);
                        }
                        Err(new_err) => {
                            context.pop();
                            last_error = Some(new_err);
                        }
                    }
                } else {
                    // Not a thrown error, just try normally
                    match engine.evaluate_node(arg, context) {
                        Ok(result) => return Ok(result),
                        Err(err) => last_error = Some(err),
                    }
                }
            } else {
                // No previous error, just evaluate normally
                match engine.evaluate_node(arg, context) {
                    Ok(result) => return Ok(result),
                    Err(err) => last_error = Some(err),
                }
            }
        } else {
            // Not the last argument, just try normally
            match engine.evaluate_node(arg, context) {
                Ok(result) => return Ok(result),
                Err(err) => last_error = Some(err),
            }
        }
    }

    // If we get here, all arguments failed
    // Return the last error
    match last_error {
        Some(err) => Err(err),
        None => Err(Error::InvalidArguments(
            "try: no arguments provided".to_string(),
        )),
    }
}
