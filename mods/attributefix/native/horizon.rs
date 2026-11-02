// AttributeFix inside Horizon's attribute definitions (nightly 142388e1c). The rules are the
// engine below; the game builds each attribute from its catalog with `Definition::ranged`, and
// this module gives that call the range the pack's file asks for.
#[allow(dead_code)]
mod engine {
    include!("attributefix_engine.rs");
}

/// The range to build an attribute with instead of `(min, max)`, or `None` to build it as is.
pub(crate) fn adjusted(default: f64, min: f64, max: f64) -> Option<(f64, f64)> {
    engine::adjust(default, min, max)
}
