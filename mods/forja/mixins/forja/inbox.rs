// The forja frames of the guests in this level leave their inboxes before the game reads them.
fn forja_inbox(hub: &mut Hub) {
    crate::forja_net::scan_inboxes(hub);
}
