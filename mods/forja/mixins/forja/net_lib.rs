// forja's network queues in the game library (`horizon_client::forja_net`): the frames of forja's
// custom payload channel taken out of the packet streams, for the game (`crate::game::forja`) to run.
// The library cannot call the game (the binary depends on it), so it only queues.
//
// - server: a HEAD on `Hub::tick_level` runs `scan_inboxes` before the hub hands the guests' inboxes
//   to the game; the game takes them with `take_server` (HEAD on `lan_living_flags`);
// - client: a RETURN on `Session::poll` runs `client_filter` on the received events; the game takes
//   them with `take_client` (HEAD on `tick_remote`).
use crate::net::Event;
use horizon_protocol::packets::play::{Clientbound, Serverbound};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The custom payload channel of forja's frames (forja's `engine::NET_CHANNEL`).
pub const CHANNEL: &str = "forja:net";
/// Frames waiting at most, each way (a guest flooding the server loses the rest).
pub const MAX_QUEUED: usize = 4096;

static SERVER_IN: Mutex<Vec<(u32, Vec<u8>)>> = Mutex::new(Vec::new());
static CLIENT_IN: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
static SERVER_ANY: AtomicBool = AtomicBool::new(false);
static CLIENT_ANY: AtomicBool = AtomicBool::new(false);

fn ours_s(p: &Serverbound) -> bool {
    match p {
        Serverbound::CustomPayload(c) => c.channel == CHANNEL,
        _ => false,
    }
}
fn ours_c(e: &Event) -> bool {
    match e {
        Event::Packet(p) => match &**p {
            Clientbound::CustomPayload(c) => c.channel == CHANNEL,
            _ => false,
        },
        _ => false,
    }
}

/// HEAD `Hub::tick_level`: the forja frames of the guests in the level of this pass leave their
/// inboxes (in order), before the game's packet handler (which drops unknown payloads) sees them.
pub fn scan_inboxes(hub: &mut crate::lan::hub::Hub) {
    let dimension = hub.config.dimension.clone();
    let mut taken: Vec<(u32, Vec<u8>)> = Vec::new();
    for g in hub.guests.iter_mut() {
        if !g.placed || g.dimension != dimension || !g.inbox.iter().any(ours_s) {
            continue;
        }
        let conn = g.conn;
        let inbox = std::mem::take(&mut g.inbox);
        for p in inbox {
            match p {
                Serverbound::CustomPayload(c) if c.channel == CHANNEL => taken.push((conn, c.data)),
                other => g.inbox.push_back(other),
            }
        }
    }
    if taken.is_empty() {
        return;
    }
    let mut q = SERVER_IN.lock().unwrap_or_else(|e| e.into_inner());
    for t in taken {
        if q.len() < MAX_QUEUED {
            q.push(t);
        }
    }
    SERVER_ANY.store(true, Ordering::Relaxed);
}

/// The frames the guests sent since the last call: `(conn, frame)`, in arrival order.
pub fn take_server() -> Vec<(u32, Vec<u8>)> {
    if !SERVER_ANY.swap(false, Ordering::Relaxed) {
        return Vec::new();
    }
    std::mem::take(&mut *SERVER_IN.lock().unwrap_or_else(|e| e.into_inner()))
}

/// RETURN `Session::poll`: the forja frames leave the events the game handles.
pub fn client_filter(events: Vec<Event>) -> Vec<Event> {
    if !events.iter().any(ours_c) {
        return events;
    }
    let mut keep = Vec::with_capacity(events.len());
    let mut got: Vec<Vec<u8>> = Vec::new();
    for e in events {
        match e {
            Event::Packet(p) => match *p {
                Clientbound::CustomPayload(c) if c.channel == CHANNEL => got.push(c.data),
                other => keep.push(Event::Packet(Box::new(other))),
            },
            other => keep.push(other),
        }
    }
    let mut q = CLIENT_IN.lock().unwrap_or_else(|e| e.into_inner());
    for f in got {
        if q.len() < MAX_QUEUED {
            q.push(f);
        }
    }
    CLIENT_ANY.store(true, Ordering::Relaxed);
    keep
}

/// The frames the server sent since the last call, in arrival order.
pub fn take_client() -> Vec<Vec<u8>> {
    if !CLIENT_ANY.swap(false, Ordering::Relaxed) {
        return Vec::new();
    }
    std::mem::take(&mut *CLIENT_IN.lock().unwrap_or_else(|e| e.into_inner()))
}
