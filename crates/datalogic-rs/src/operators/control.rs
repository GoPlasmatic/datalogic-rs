use crate::{CompiledNode, Engine, Result};

// =============================================================================
// Arena-mode control operators
// =============================================================================

use crate::arena::{ContextStack, DataValue, truthy_arena};
use bumpalo::Bump;

#[inline]
pub(crate) fn evaluate_if<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_null());
    }

    if args.len() == 3 {
        let cond = engine.dispatch_node(&args[0], ctx, arena)?;
        let idx = if truthy_arena(cond, engine) { 1 } else { 2 };
        return engine.dispatch_node(&args[idx], ctx, arena);
    }

    let mut i = 0;
    while i < args.len() {
        if i == args.len() - 1 {
            return engine.dispatch_node(&args[i], ctx, arena);
        }
        let cond = engine.dispatch_node(&args[i], ctx, arena)?;
        if truthy_arena(cond, engine) {
            // The `i == len - 1` guard above already returned, so here
            // `i < len - 1` and a then-branch at `i + 1` always exists.
            return engine.dispatch_node(&args[i + 1], ctx, arena);
        }
        i += 2;
    }
    Ok(crate::arena::singletons::singleton_null())
}

#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_coalesce<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Ok(crate::arena::singletons::singleton_null());
    }
    for arg in args {
        let v = engine.dispatch_node(arg, ctx, arena)?;
        if !matches!(v, DataValue::Null) {
            return Ok(v);
        }
    }
    Ok(crate::arena::singletons::singleton_null())
}

#[cfg(feature = "ext-control")]
#[inline]
pub(crate) fn evaluate_switch<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    use crate::operators::comparison::compare_equals;
    if args.len() < 2 {
        return Ok(crate::arena::singletons::singleton_null());
    }
    let disc_av = engine.dispatch_node(&args[0], ctx, arena)?;

    match &args[1] {
        CompiledNode::Array { nodes, .. } => {
            for case_node in nodes.iter() {
                match case_node {
                    CompiledNode::Array { nodes: pair, .. } if pair.len() >= 2 => {
                        let cv_av = engine.dispatch_node(&pair[0], ctx, arena)?;
                        // Propagate comparison errors rather than treating them
                        // as "no match": a failed compare is a real error, not
                        // a silent fall-through to the default arm.
                        if compare_equals(disc_av, cv_av, true, engine)? {
                            return engine.dispatch_node(&pair[1], ctx, arena);
                        }
                    }
                    CompiledNode::Value { lit: Some(av), .. } => {
                        if let DataValue::Array(pair_av) = av.as_ref() {
                            if pair_av.len() >= 2
                                && compare_equals(disc_av, &pair_av[0], true, engine)?
                            {
                                return Ok(&pair_av[1]);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        CompiledNode::Value { lit: Some(av), .. } => {
            if let DataValue::Array(cases_av) = av.as_ref() {
                for case_av in cases_av.iter() {
                    if let DataValue::Array(pair_av) = case_av {
                        if pair_av.len() >= 2
                            && compare_equals(disc_av, &pair_av[0], true, engine)?
                        {
                            return Ok(&pair_av[1]);
                        }
                    }
                }
            }
        }
        _ => {}
    }

    if args.len() > 2 {
        return engine.dispatch_node(&args[2], ctx, arena);
    }
    Ok(crate::arena::singletons::singleton_null())
}
