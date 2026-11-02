// The foundry frames of the guests in this level leave their inboxes before the game reads them.
fn foundry_inbox(hub: &mut Hub) {
    crate::foundry_net::scan_inboxes(hub);
}
