fn attributefix(range: (f64, f64), default: f64) -> (f64, f64) {
    attributefix::adjusted(default, range.0, range.1).unwrap_or(range)
}
