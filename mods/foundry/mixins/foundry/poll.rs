// The foundry frames the server sent leave the events the game handles.
fn foundry_poll(value: Vec<Event>) -> Vec<Event> {
    crate::foundry_net::client_filter(value)
}
