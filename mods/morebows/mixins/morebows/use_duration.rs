fn morebows_use_duration(value: i32, components: &Components) -> i32 {
    let model = components.get("minecraft:item_model").and_then(Value::as_str).unwrap_or("");
    if value == 0 && crate::game::morebows::is_bow_model(model) {
        72000
    } else {
        value
    }
}
