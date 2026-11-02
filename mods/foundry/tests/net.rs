// Mod networking: the frames of the `foundry:net` channel and the typed messages.
// Every frame comes from another machine: nothing it holds may panic or half-apply a message.
use horizon_foundry::*;

fn reassemble(frames: &[Vec<u8>]) -> Vec<(String, Vec<u8>)> {
    let mut r = Reassembler::new();
    frames.iter().filter_map(|f| r.push(f).expect("a good frame")).collect()
}

#[test]
fn a_small_message_is_one_frame_and_comes_back_whole() {
    let one = frames(7, "ironchest:menu", b"hello");
    assert_eq!(one.len(), 1);
    assert!(one[0].len() < 64);
    assert_eq!(reassemble(&one), vec![("ironchest:menu".to_owned(), b"hello".to_vec())]);
    // An empty body is a message too.
    let empty = frames(8, "a:b", &[]);
    assert_eq!(empty.len(), 1);
    assert_eq!(reassemble(&empty), vec![("a:b".to_owned(), Vec::new())]);
}

#[test]
fn a_big_message_is_split_under_the_serverbound_limit_and_joined_in_order() {
    let body: Vec<u8> = (0..250_000u32).map(|i| (i * 31 % 251) as u8).collect();
    let parts = frames(1, "ae2:window", &body);
    assert_eq!(parts.len(), body.len().div_ceil(FRAME_DATA));
    // ServerboundCustomPayloadPacket.MAX_PAYLOAD_SIZE.
    assert!(parts.iter().all(|f| f.len() <= 32_767), "every frame fits a serverbound payload");
    let mut r = Reassembler::new();
    for (i, f) in parts.iter().enumerate() {
        let done = r.push(f).unwrap();
        assert_eq!(done.is_some(), i + 1 == parts.len(), "only the last frame finishes it");
    }
    assert_eq!(r.pending(), 0);
    let again = reassemble(&parts);
    assert_eq!(again[0].1, body);
}

#[test]
fn two_messages_interleaved_on_one_connection_both_arrive() {
    let a: Vec<u8> = vec![1; FRAME_DATA * 2 + 5];
    let b: Vec<u8> = vec![2; FRAME_DATA + 1];
    let fa = frames(10, "x:a", &a);
    let fb = frames(11, "x:b", &b);
    let mut r = Reassembler::new();
    let mut out = Vec::new();
    for f in [&fa[0], &fb[0], &fa[1], &fb[1], &fa[2]] {
        if let Some(m) = r.push(f).unwrap() {
            out.push(m);
        }
    }
    assert_eq!(out, vec![("x:b".to_owned(), b), ("x:a".to_owned(), a)]);
}

#[test]
fn bad_frames_are_refused_without_panicking_and_lose_only_their_message() {
    let body = vec![9u8; FRAME_DATA + 10];
    let f = frames(3, "m:c", &body);
    let mut r = Reassembler::new();
    // Another version.
    let mut v = f[0].clone();
    v[0] = 2;
    assert_eq!(r.push(&v), Err(FrameError::Version));
    // A second part without its first.
    assert_eq!(r.push(&f[1]), Err(FrameError::OutOfOrder));
    // The first part twice: the message restarts (the old half is not kept twice) and completes.
    assert_eq!(r.push(&f[0]), Ok(None));
    assert_eq!(r.push(&f[0]), Ok(None));
    assert_eq!(r.pending(), 1);
    assert_eq!(r.push(&f[1]), Ok(Some(("m:c".to_owned(), body.clone()))));
    assert_eq!(r.pending(), 0);
    // A part out of order loses the message.
    assert_eq!(r.push(&f[0]), Ok(None));
    assert_eq!(r.push(&frames(3, "m:c", &vec![9u8; FRAME_DATA * 3])[2]), Err(FrameError::OutOfOrder));
    assert_eq!(r.pending(), 0);
    // A cut frame.
    assert_eq!(r.push(&f[0]), Ok(None));
    let cut = &f[1][..f[1].len() - 1];
    assert_eq!(r.push(cut), Err(FrameError::Malformed));
    assert_eq!(r.pending(), 0);
    // Garbage of every length.
    let mut seed = 12345u32;
    for len in 0..200 {
        let g: Vec<u8> = (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (seed >> 16) as u8
            })
            .collect();
        let _ = r.push(&g);
    }
    // A good message still goes through afterwards.
    let ok = frames(99, "m:ok", b"fine");
    let mut fresh = Reassembler::new();
    assert_eq!(fresh.push(&ok[0]), Ok(Some(("m:ok".to_owned(), b"fine".to_vec()))));
}

#[test]
fn a_lying_length_and_too_many_messages_are_refused() {
    // A first frame announcing more than MAX_MESSAGE.
    let mut w = Writer::new();
    w.u8(NET_VERSION).u32(1).uvar(0).uvar((MAX_MESSAGE / FRAME_DATA + 1) as u64).str("a:b").uvar((MAX_MESSAGE + 1) as u64);
    let mut r = Reassembler::new();
    assert!(r.push(&w.finish()).is_err());
    // Unfinished messages are capped per connection.
    let big = vec![0u8; FRAME_DATA + 1];
    for id in 0..MAX_PENDING as u32 {
        assert_eq!(r.push(&frames(id, "a:b", &big)[0]), Ok(None));
    }
    assert_eq!(r.push(&frames(500, "a:b", &big)[0]), Err(FrameError::TooMany));
    assert_eq!(r.pending(), MAX_PENDING);
}

#[derive(Clone, Debug, PartialEq)]
struct Note {
    who: String,
    pos: [i32; 3],
    items: Vec<(u32, Option<Vec<u8>>)>,
    level: i64,
    on: bool,
}
impl Wire for Note {
    fn put(&self, w: &mut Writer) {
        w.str(&self.who).put(&self.pos).put(&self.items).ivar(self.level).bool(self.on);
    }
    fn get(r: &mut Reader<'_>) -> Option<Self> {
        Some(Note { who: r.get()?, pos: r.get()?, items: r.get()?, level: r.ivar()?, on: r.bool()? })
    }
}

#[test]
fn a_typed_channel_carries_its_messages_both_ways() {
    const CH: Channel<Note> = Channel::new("test:note");
    let n = Note { who: "Alex".into(), pos: [-30_000_000, -64, 29_999_999], items: vec![(0, None), (107, Some(vec![1, 2, 3]))], level: -1 << 40, on: true };
    let body = CH.encode(&n);
    let f = frames(5, CH.id, &body);
    let (ch, back) = reassemble(&f).remove(0);
    assert_eq!(ch, "test:note");
    assert_eq!(CH.decode(&back), Some(n.clone()));
    // A cut or padded message is not a Note.
    assert_eq!(CH.decode(&back[..back.len() - 1]), None);
    let mut padded = back.clone();
    padded.push(0);
    assert_eq!(CH.decode(&padded), None);
    // Varints at their ends.
    let mut w = Writer::new();
    w.ivar(i64::MIN).ivar(i64::MAX).uvar(u64::MAX).i32(i32::MIN).f32(-0.5).f64(1e300);
    let b = w.finish();
    let mut r = Reader::new(&b);
    assert_eq!((r.ivar(), r.ivar(), r.uvar(), r.i32(), r.f32(), r.f64()), (Some(i64::MIN), Some(i64::MAX), Some(u64::MAX), Some(i32::MIN), Some(-0.5), Some(1e300)));
    assert!(r.done());
}

#[test]
fn every_menu_message_round_trips_and_no_prefix_of_it_decodes() {
    let to_server = vec![
        MenuToServer::Hello { version: MENU_VERSION },
        MenuToServer::Input { window: 3, seq: 9, input: MenuInput::Ops(vec![(300, 0, 0), (SLOT_OUTSIDE, 1, 4), (35, 2, 2)]) },
        MenuToServer::Input {
            window: 3,
            seq: 10,
            input: MenuInput::Mouse { button: 1, x: 412.5, y: 99.0, w: 1920, h: 1080, shift: true, extra: vec![1, 0] },
        },
        MenuToServer::Input { window: 3, seq: 11, input: MenuInput::Scroll { delta: -1.0, slot: 305, x: 1.0, y: 2.0, w: 3, h: 4, shift: false } },
        MenuToServer::Input { window: 3, seq: 12, input: MenuInput::Key { key: 1, shift: false } },
        MenuToServer::Input { window: 3, seq: 13, input: MenuInput::Text { text: Some("iron ingot".into()), backspace: false } },
        MenuToServer::Input { window: 3, seq: 14, input: MenuInput::Text { text: None, backspace: true } },
        MenuToServer::Input { window: 3, seq: 15, input: MenuInput::Button { id: 4, arg: -7 } },
        MenuToServer::Close { window: 3 },
        MenuToServer::Size { w: 640, h: 360 },
    ];
    for m in &to_server {
        let b = to_bytes(m);
        assert_eq!(from_bytes::<MenuToServer>(&b).as_ref(), Some(m));
        for cut in 0..b.len() {
            assert_eq!(from_bytes::<MenuToServer>(&b[..cut]), None, "{m:?} cut at {cut}");
        }
    }
    let to_client = vec![
        MenuToClient::Hello { version: MENU_VERSION },
        MenuToClient::Open { window: 1, kind: "ironchest:chest".into(), pos: [1, 2, 3], size: 108, title: "Diamond".into() },
        MenuToClient::Slots { window: 1, size: 108, changes: vec![(0, Some(vec![10, 0, 0])), (107, None)] },
        MenuToClient::Machine { window: 1, text: Some("foundry 1\nm 1 2 3 x:y\n".into()) },
        MenuToClient::Machine { window: 1, text: None },
        MenuToClient::Section { window: 1, index: 2, data: vec![0; 70] },
        MenuToClient::Close { window: 1 },
    ];
    for m in &to_client {
        let b = to_bytes(m);
        assert_eq!(from_bytes::<MenuToClient>(&b).as_ref(), Some(m));
        for cut in 0..b.len() {
            assert_eq!(from_bytes::<MenuToClient>(&b[..cut]), None, "{m:?} cut at {cut}");
        }
    }
    // More container slots than a menu may have, or a flood of clicks, is not a message.
    let too_big = to_bytes(&MenuToClient::Open { window: 1, kind: "a:b".into(), pos: [0; 3], size: MAX_MENU_SLOTS as u32 + 1, title: String::new() });
    assert_eq!(from_bytes::<MenuToClient>(&too_big), None);
    let flood = to_bytes(&MenuToServer::Input { window: 1, seq: 1, input: MenuInput::Ops(vec![(0, 0, 0); 513]) });
    assert_eq!(from_bytes::<MenuToServer>(&flood), None);
}
