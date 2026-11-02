// Forja menus for guests, simulated end to end: a guest's client sends its clicks over the channel,
// the server runs them on the authoritative slots (here a small vanilla-like click engine stands in
// for the game's `menu_ops` and the mods' own sessions), and mirrors back what changed. Whatever the
// clicks, the frames lost, repeated or for another window: the client's copy ends equal to the
// server's and no item is made or lost (container + inventory + carried + thrown).
mod common;
use common::*;
use horizon_forja::*;

const MAX: u32 = 64;

fn enc(s: &St) -> Vec<u8> {
    format!("{}*{}", s.item, s.count).into_bytes()
}
fn dec(b: &[u8]) -> St {
    let t = String::from_utf8(b.to_vec()).unwrap();
    let (i, n) = t.split_once('*').unwrap();
    st(i, n.parse().unwrap())
}

/// The server's side: a block's container (game slots 300..), the guest's 36 inventory slots and
/// carried stack (window 0 is the game's own sync, mirrored like the menu here).
struct Server {
    container: Vec<Option<St>>,
    inv: Vec<Option<St>>,
    carried: Option<St>,
    thrown: Vec<St>,
    drag: Option<(i32, Vec<i32>)>,
    window: i32,
    gate: InputGate,
    frames: Reassembler,
    sent_slots: Mirror<Vec<u8>>,
    sent_inv: Mirror<Vec<u8>>,
    sent_carried: Option<Vec<u8>>,
    next_id: u32,
}
/// The guest's client: only copies.
struct Client {
    container: Vec<Option<Vec<u8>>>,
    inv: Vec<Option<Vec<u8>>>,
    carried: Option<Vec<u8>>,
    window: i32,
    seq: u32,
    frames: Reassembler,
    next_id: u32,
}

impl Server {
    fn slot(&mut self, s: i32) -> Option<&mut Option<St>> {
        match s {
            0..36 => self.inv.get_mut(s as usize),
            300.. => self.container.get_mut((s - 300) as usize),
            _ => None,
        }
    }
    fn total(&self) -> u32 {
        self.container.iter().chain(&self.inv).flatten().map(|s| s.count).sum::<u32>()
            + self.carried.as_ref().map_or(0, |c| c.count)
            + self.thrown.iter().map(|s| s.count).sum::<u32>()
    }
    /// Moves `stack` into the slots `to` (merging first, as moveItemStackTo); the rest is returned.
    fn move_into(&mut self, mut stack: St, to: std::ops::Range<i32>) -> Option<St> {
        for pass in 0..2 {
            for s in to.clone() {
                let Some(slot) = self.slot(s) else { continue };
                match slot {
                    Some(t) if pass == 0 && t.same_kind(&stack) && t.count < MAX => {
                        let n = (MAX - t.count).min(stack.count);
                        t.count += n;
                        stack.count -= n;
                    }
                    None if pass == 1 => {
                        *slot = Some(stack);
                        return None;
                    }
                    _ => {}
                }
                if stack.count == 0 {
                    return None;
                }
            }
        }
        Some(stack)
    }
    /// One click (`AbstractContainerMenu.doClick`, the subset the test needs).
    fn click(&mut self, slot: i32, button: i32, input: u8) {
        let n = self.container.len() as i32;
        match input {
            // PICKUP
            0 => {
                if slot == SLOT_OUTSIDE {
                    if let Some(mut c) = self.carried.take() {
                        if button == 1 && c.count > 1 {
                            c.count -= 1;
                            self.thrown.push(st(&c.item, 1));
                            self.carried = Some(c);
                        } else {
                            self.thrown.push(c);
                        }
                    }
                    return;
                }
                let mut carried = self.carried.take();
                let Some(s) = self.slot(slot) else {
                    self.carried = carried;
                    return;
                };
                match (s.take(), carried.take()) {
                    (None, None) => {}
                    (Some(mut t), None) => {
                        if button == 1 {
                            let half = t.count.div_ceil(2);
                            t.count -= half;
                            carried = Some(st(&t.item, half));
                            *s = (t.count > 0).then_some(t);
                        } else {
                            carried = Some(t);
                        }
                    }
                    (None, Some(mut c)) => {
                        if button == 1 && c.count > 1 {
                            c.count -= 1;
                            *s = Some(st(&c.item, 1));
                            carried = Some(c);
                        } else {
                            *s = Some(c);
                        }
                    }
                    (Some(mut t), Some(mut c)) if t.same_kind(&c) => {
                        let want = if button == 1 { 1 } else { c.count };
                        let m = want.min(MAX - t.count);
                        t.count += m;
                        c.count -= m;
                        *s = Some(t);
                        carried = (c.count > 0).then_some(c);
                    }
                    (Some(t), Some(c)) => {
                        *s = Some(c);
                        carried = Some(t);
                    }
                }
                self.carried = carried;
            }
            // QUICK_MOVE: container -> inventory (hotbar last), inventory -> container.
            1 => {
                let Some(s) = self.slot(slot) else { return };
                let Some(stack) = s.take() else { return };
                let rest = if slot >= 300 { self.move_into(stack, 9..36).and_then(|r| self.move_into(r, 0..9)) } else { self.move_into(stack, 300..300 + n) };
                if let Some(r) = rest {
                    *self.slot(slot).unwrap() = Some(r);
                }
            }
            // SWAP with a hotbar slot (button 0..8).
            2 => {
                if !(0..9).contains(&button) || self.slot(slot).is_none() {
                    return;
                }
                let a = self.slot(slot).unwrap().take();
                let b = self.inv[button as usize].take();
                self.inv[button as usize] = a;
                *self.slot(slot).unwrap() = b;
            }
            // THROW from a slot (button 1 = the whole stack).
            4 => {
                let Some(s) = self.slot(slot) else { return };
                let Some(mut t) = s.take() else { return };
                let thrown = if button == 1 || t.count == 1 {
                    t
                } else {
                    t.count -= 1;
                    let one = st(&t.item, 1);
                    *s = Some(t);
                    one
                };
                self.thrown.push(thrown);
            }
            // QUICK_CRAFT: stage = button & 3 (start, add, end), type = button >> 2 (even, one).
            5 => match button & 3 {
                0 => self.drag = Some((button >> 2, Vec::new())),
                1 => {
                    if let Some((_, slots)) = self.drag.as_mut()
                        && !slots.contains(&slot)
                        && (slot < 36 || (300..300 + n).contains(&slot))
                    {
                        slots.push(slot);
                    }
                }
                _ => {
                    let Some((ty, slots)) = self.drag.take() else { return };
                    let Some(mut c) = self.carried.take() else { return };
                    let targets: Vec<i32> = slots
                        .into_iter()
                        .filter(|&s| self.slot(s).is_some_and(|x| x.as_ref().is_none_or(|t| t.same_kind(&c) && t.count < MAX)))
                        .collect();
                    if !targets.is_empty() {
                        let each = if ty == 0 { (c.count / targets.len() as u32).max(1) } else { 1 };
                        for s in targets {
                            if c.count == 0 {
                                break;
                            }
                            let slot = self.slot(s).unwrap();
                            let have = slot.as_ref().map_or(0, |t| t.count);
                            let m = each.min(MAX - have).min(c.count);
                            *slot = Some(st(&c.item, have + m));
                            c.count -= m;
                        }
                    }
                    self.carried = (c.count > 0).then_some(c);
                }
            },
            _ => {}
        }
    }
    /// The frames the client sent: each input once, for the open window only.
    fn receive(&mut self, frame: &[u8]) {
        let Ok(Some((channel, body))) = self.frames.push(frame) else { return };
        if channel != MENU_CHANNEL {
            return;
        }
        if let Some(MenuToServer::Input { window, seq, input: MenuInput::Ops(ops) }) = from_bytes(&body)
            && self.gate.accept(window, seq)
        {
            for (slot, button, input) in ops {
                self.click(slot, button, input);
            }
        }
    }
    /// broadcastChanges: the menu's slots, window 0's, the carried stack.
    fn sync(&mut self) -> Vec<Vec<u8>> {
        let container: Vec<Option<Vec<u8>>> = self.container.iter().map(|s| s.as_ref().map(enc)).collect();
        let inv: Vec<Option<Vec<u8>>> = self.inv.iter().map(|s| s.as_ref().map(enc)).collect();
        let carried = self.carried.as_ref().map(enc);
        let mut out = Vec::new();
        let mut send = |s: &mut Self, m: MenuToClient| {
            s.next_id += 1;
            out.extend(frames(s.next_id, MENU_CHANNEL, &to_bytes(&m)));
        };
        let c = self.sent_slots.diff(&container);
        if !c.is_empty() {
            let w = self.window;
            send(self, MenuToClient::Slots { window: w, size: self.container.len() as u32, changes: c });
        }
        let i = self.sent_inv.diff(&inv);
        if !i.is_empty() {
            send(self, MenuToClient::Slots { window: 0, size: 36, changes: i });
        }
        if self.sent_carried != carried {
            self.sent_carried = carried.clone();
            send(self, MenuToClient::Slots { window: -1, size: 1, changes: vec![(0, carried)] });
        }
        out
    }
}

impl Client {
    fn click(&mut self, slot: i32, button: i32, input: u8) -> Vec<Vec<u8>> {
        self.seq += 1;
        self.next_id += 1;
        let m = MenuToServer::Input { window: self.window, seq: self.seq, input: MenuInput::Ops(vec![(slot, button, input)]) };
        frames(self.next_id, MENU_CHANNEL, &to_bytes(&m))
    }
    fn receive(&mut self, frame: &[u8]) {
        let Ok(Some((_, body))) = self.frames.push(frame) else { return };
        match from_bytes::<MenuToClient>(&body) {
            Some(MenuToClient::Slots { window: 0, changes, .. }) => apply_changes(&mut self.inv, &changes, 36),
            Some(MenuToClient::Slots { window: -1, changes, .. }) => self.carried = changes.into_iter().next().and_then(|c| c.1),
            Some(MenuToClient::Slots { window, size, changes }) if window == self.window => {
                self.container.resize(size as usize, None);
                apply_changes(&mut self.container, &changes, size as usize)
            }
            _ => {}
        }
    }
}

fn pair(size: usize) -> (Server, Client) {
    let mut container = vec![None; size];
    let items = ["minecraft:diamond", "minecraft:iron_ingot", "minecraft:dirt", "ironchest:gold_chest"];
    for (i, slot) in container.iter_mut().enumerate().step_by(3) {
        *slot = Some(st(items[i % 4], (i as u32 * 7) % 64 + 1));
    }
    let mut inv = vec![None; 36];
    inv[0] = Some(st("minecraft:diamond", 10));
    inv[20] = Some(st("minecraft:cobblestone", 64));
    let window = next_window(0, None);
    let mut server = Server {
        container,
        inv,
        carried: None,
        thrown: Vec::new(),
        drag: None,
        window,
        gate: InputGate::new(window),
        frames: Reassembler::new(),
        sent_slots: Mirror::new(size),
        sent_inv: Mirror::new(36),
        sent_carried: None,
        next_id: 0,
    };
    let mut client =
        Client { container: vec![None; size], inv: vec![None; 36], carried: None, window, seq: 0, frames: Reassembler::new(), next_id: 0 };
    for f in server.sync() {
        client.receive(&f);
    }
    (server, client)
}
fn same(server: &Server, client: &Client) -> bool {
    let c: Vec<Option<Vec<u8>>> = server.container.iter().map(|s| s.as_ref().map(enc)).collect();
    let i: Vec<Option<Vec<u8>>> = server.inv.iter().map(|s| s.as_ref().map(enc)).collect();
    c == client.container && i == client.inv && server.carried.as_ref().map(enc) == client.carried
}

#[test]
fn the_guest_sees_a_diamond_chest_of_108_slots_exactly() {
    let (s, c) = pair(108);
    assert!(same(&s, &c), "the first sync sends every slot");
    assert_eq!(c.container.iter().flatten().count(), s.container.iter().flatten().count());
    // Past six chest rows: the slots a vanilla generic_9x6 menu could not carry.
    assert!(c.container[54..].iter().any(Option::is_some));
    assert_eq!(dec(c.container[105].as_ref().unwrap()), *s.container[105].as_ref().unwrap());
}

#[test]
fn clicks_shift_clicks_swaps_and_throws_travel_and_the_copies_agree() {
    let (mut s, mut c) = pair(81);
    let start = s.total();
    // Pick up the first gold-tier slot, put half back, shift-click the rest, swap, throw.
    for (slot, button, input) in [(300, 0, 0u8), (380, 1, 0), (380, 0, 0), (303, 0, 1), (0, 0, 1), (306, 4, 2), (309, 0, 4), (20, 1, 4)] {
        for f in c.click(slot, button, input) {
            s.receive(&f);
        }
        for f in s.sync() {
            c.receive(&f);
        }
        assert!(same(&s, &c), "after {slot} {button} {input}");
        assert_eq!(s.total(), start, "nothing made or lost after {slot} {button} {input}");
    }
    assert!(!s.thrown.is_empty());
}

#[test]
fn a_drag_is_split_on_the_server_and_mirrored() {
    let (mut s, mut c) = pair(27);
    let start = s.total();
    // Carry the 10 diamonds, drag evenly over three empty container slots.
    let mut ops = vec![(0, 0, 0u8), (SLOT_OUTSIDE, 0, 5)];
    ops.extend([301, 302, 304].map(|x| (x, 1, 5)));
    ops.push((SLOT_OUTSIDE, 2, 5));
    c.seq += 1;
    let m = MenuToServer::Input { window: c.window, seq: c.seq, input: MenuInput::Ops(ops) };
    for f in frames(77, MENU_CHANNEL, &to_bytes(&m)) {
        s.receive(&f);
    }
    for f in s.sync() {
        c.receive(&f);
    }
    assert!(same(&s, &c));
    assert_eq!(s.total(), start);
    let counts: Vec<u32> = [1, 2, 4].iter().map(|&i| s.container[i].as_ref().map_or(0, |x| x.count)).collect();
    assert_eq!(counts, vec![3, 3, 3]);
    assert_eq!(s.carried.as_ref().map(|x| x.count), Some(1));
}

#[test]
fn a_repeated_frame_another_window_or_an_old_input_changes_nothing() {
    let (mut s, mut c) = pair(54);
    let pick = c.click(300, 0, 0);
    for f in &pick {
        s.receive(f);
    }
    let after_one = (s.container.clone(), s.carried.clone());
    // The same frames again (a duplicated packet): the gate refuses the old seq.
    for f in &pick {
        s.receive(f);
    }
    assert_eq!((s.container.clone(), s.carried.clone()), after_one);
    // A click for a window that is not open.
    c.window += 1;
    for f in c.click(303, 0, 0) {
        s.receive(&f);
    }
    c.window -= 1;
    assert_eq!((s.container.clone(), s.carried.clone()), after_one);
    // An input older than the last that ran.
    let mut old = MenuToServer::Input { window: c.window, seq: 1, input: MenuInput::Ops(vec![(303, 0, 0)]) };
    for f in frames(500, MENU_CHANNEL, &to_bytes(&old)) {
        s.receive(&f);
    }
    assert_eq!((s.container.clone(), s.carried.clone()), after_one);
    // A slot that is not the menu's.
    if let MenuToServer::Input { seq, input, .. } = &mut old {
        *seq = 999;
        *input = MenuInput::Ops(vec![(299, 0, 0), (300 + 54, 0, 0), (-5, 0, 1)]);
    }
    for f in frames(501, MENU_CHANNEL, &to_bytes(&old)) {
        s.receive(&f);
    }
    assert_eq!((s.container.clone(), s.carried.clone()), after_one);
    for f in s.sync() {
        c.receive(&f);
    }
    assert!(same(&s, &c));
}

#[test]
fn thousands_of_random_clicks_never_make_or_lose_an_item_and_the_client_follows() {
    for size in [1usize, 27, 54, 81, 108] {
        let (mut s, mut c) = pair(size);
        let start = s.total();
        let mut seed = 0x5eed_u64 + size as u64;
        let mut rnd = |n: u64| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % n
        };
        for step in 0..3000 {
            let slot = match rnd(10) {
                0 => SLOT_OUTSIDE,
                1..=3 => rnd(36) as i32,
                _ => 300 + rnd(size as u64 + 2) as i32, // sometimes one past the end
            };
            let input = [0u8, 0, 0, 1, 2, 4, 5][rnd(7) as usize];
            let button = match input {
                2 => rnd(9) as i32,
                5 => [0, 1, 2, 4, 5, 6][rnd(6) as usize],
                _ => rnd(2) as i32,
            };
            let f = c.click(slot, button, input);
            // A frame lost now and then: the click just did not happen, for both.
            if rnd(50) != 0 {
                for x in &f {
                    s.receive(x);
                }
            }
            if rnd(5) == 0 {
                for x in &f {
                    s.receive(x); // and repeated sometimes
                }
            }
            for x in s.sync() {
                c.receive(&x);
            }
            assert_eq!(s.total(), start, "size {size} step {step}: {slot} {button} {input}");
            assert!(same(&s, &c), "size {size} step {step}: the client's copy");
            assert!(s.container.iter().chain(&s.inv).flatten().all(|x| x.count > 0 && x.count <= MAX));
        }
    }
}

#[test]
fn the_mirror_sends_only_what_changed_and_empties_what_is_gone() {
    let mut m: Mirror<Vec<u8>> = Mirror::new(4);
    let now = vec![None, Some(vec![1]), None, Some(vec![2])];
    assert_eq!(m.diff(&now), vec![(1, Some(vec![1])), (3, Some(vec![2]))]);
    assert_eq!(m.diff(&now), vec![]);
    // The list shrinks (an AE2 window with fewer sections): what is gone is sent empty.
    assert_eq!(m.diff(&[None, Some(vec![9])]), vec![(1, Some(vec![9])), (3, None)]);
    let mut copy: Vec<Option<Vec<u8>>> = vec![None; 4];
    apply_changes(&mut copy, &[(1, Some(vec![9])), (3, None), (900, Some(vec![1]))], MAX_MENU_SLOTS);
    assert_eq!(copy, vec![None, Some(vec![9]), None, None]);
}

#[test]
fn window_ids_cycle_like_vanillas_and_avoid_the_open_vanilla_menu() {
    assert_eq!(next_window(0, None), 1);
    assert_eq!(next_window(100, None), 1);
    assert_eq!(next_window(5, Some(6)), 7);
    assert_eq!(next_window(99, Some(100)), 1);
    let mut gate = InputGate::new(7);
    assert!(gate.accept(7, 1));
    assert!(!gate.accept(7, 1));
    assert!(!gate.accept(8, 2));
    assert!(gate.accept(7, 5));
    assert!(!gate.accept(7, 4));
}
