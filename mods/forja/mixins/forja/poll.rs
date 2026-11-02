// The forja frames the server sent leave the events the game handles.
fn forja_poll(value: Vec<Event>) -> Vec<Event> {
    crate::forja_net::client_filter(value)
}
