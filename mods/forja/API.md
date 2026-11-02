# forja: the API for other native mods

`forja` ("Forge for Horizon") is a shared native mod that the technical ports of this repository compile into the same
game binary. It gives them what Forge 47 (Minecraft 1.20.1) gave them through capabilities: energy (Forge Energy, FE
`int`), fluids (`FluidStack` / `FluidTank`, mB `int`) and item handlers, machine state saved with the world, a screen
helper, compound blocks (multipart), shapes from models, mod networking with menus for guests, and redstone.

The rules are plain Rust in `src/lib.rs` (tested in `tests/`); the game half is `native/horizon.rs`. Section numbers
below are the ones the other mods' comments cite.

## 0. A mod that uses forja needs forja

Native mods have no dependencies between them: `crate::game::forja` exists only when `forja` is in the same build. A
mod that calls it does not compile without it; its README says so. A mod's tests can use forja's rules directly:
`[dev-dependencies] horizon-forja = { path = "../forja" }` (for `cargo test` only).

## 1. Where it lives

- The module is `crate::game::machines::forja` (inside `game/machines.rs`, to use the hoppers' container views);
  `crate::game::forja` re-exports it (`alias.rs`). Call `crate::game::forja::...` from any mod under `crate::game`.
- Everything is `pub(in crate::game)`: visible from every mod's adapter, not from the `horizon_client` library.
- The rule types are in `crate::game::forja::engine`, also re-exported (`forja::Dir`, `forja::Spec`,
  `forja::FluidStack`, `forja::EnergyHandler`...).
- `Dir` follows `Direction.get3DDataValue` (down, up, north, south, west, east). `face: Option<Dir>` is the face of the
  target block seen by the caller (as `getCapability(cap, side)`); `None` is the block itself, full access.

## 2. A machine with its state in forja

For every registered block forja keeps a `Machine`: energy, tanks with a role, item slots with a role, progress, free
integer data and per-face access. The table is per world and dimension, in `<world>/data/forja/<dimension>.txt`,
written with the world; items are the game's NBT in hex.

```rust
use crate::game::forja::{self, Access, Dir, SlotRole, Sides, Spec, TankRole};

fn furnator_spec() -> Spec {
    Spec::new("powah:furnator_starter")
        .energy(10_000, 0, 60)          // capacity, max_receive, max_extract
        .slots(&[SlotRole::Input])
}

// In the mod's world-load hook (idempotent):
forja::register_machine(furnator_spec(), Some(furnator_tick));

fn furnator_tick(game: &mut Game<'_>, pos: [i32; 3]) {
    let burning = forja::with_machine(game, pos, |m| { /* read and change the state */ true });
    // Move things only after the closure: forja calls inside it answer "not here".
    if burning == Some(true) {
        forja::push_energy_around(game, pos, 60);
    }
}
```

forja handles by itself: the state on placement (or on first use after `/setblock`); a tick for every machine in a
block-ticking chunk; on any break, the slots' items drop at the next tick (energy and fluids are lost, as for a Forge
machine broken without a wrench); a changed spec (capacities follow, extra items drop); and every pipe, cable or machine
calling `forja::energy_*`, `fluid_*` or `item_*` sees the machine through its roles and faces.
`m.set_energy_capacity(cap)` overrides the spec's capacity (Mekanism's energy upgrades); `0` goes back to the spec.

| `SlotRole` / `TankRole` | from outside |
|---|---|
| `Input` | insert / fill only |
| `Output` | extract / drain only |
| `Both` | both |
| `Internal` (slots) | nothing (upgrades, batteries) |

`Access` per face: `None` (no capability on that face), `In`, `Out`, `Both`. `Dir::relative(facing)` gives
`Rel::{Front, Back, Left, Right, Top, Bottom}` for face configurations.

## 3. Operations

They work on any block: a forja machine, another mod's provider, a vanilla container (items).

```rust
forja::energy_info(game, p, face);  forja::energy_receive(game, p, face, 1000, true);
forja::transfer_energy(game, (from, Some(Dir::East)), (to, Some(Dir::West)), 1000);
forja::fluid_fill(game, p, face, &FluidStack::new("minecraft:water", 1000), false);
forja::transfer_fluid(game, (a, fa), (b, fb), None, 1000);
forja::item_insert(game, p, face, stack, false);  forja::transfer_items(game, (a, fa), (b, fb), 64);
```

Every move simulates both ends first; if the target still takes less than it promised, the rest goes back (energy,
fluid) or drops in the world (items), never lost. Fluids never mix. Buckets: water, lava, milk, and a mod's own with
`forja::register_bucket`. Vanilla item containers follow the hoppers' rules.

## 4. Blocks that keep their own state (providers)

When a block's state is not in forja's table (the Ender Tank's tank belongs to its frequency), the mod registers a
provider: a cheap check on `&Game` and the function that lends the handler for one operation
(`register_energy`, `register_fluid`, `register_items`). `claims` must be cheap and side-effect free; `f` is called once
per operation; a block with a forja spec never reaches the providers; re-entry is safe ("not here", no deadlock).

## 5. A machine's screen (the ironchest pattern)

A mod cannot add a menu type, so a machine with items uses a vanilla container template (`minecraft:hopper` 5 slots,
`dispenser` 9, `barrel`/`chest` 27...): `BlockEntity::Chest` holds the items (hoppers and forja see a vanilla
container, breaking spills it), the menu is vanilla, and forja draws the mod's background and bars over it with
`forja::register_screen` (`Gauge::Progress`, `Energy`, `Tank(i)`, `Data(value, max)`). The texture is carried under
`assets/<ns>/textures/block/...` so the GUI atlas packs it. Guests with forja get it as a forja menu (section 10).

## 6. Items with energy

An item's energy is `custom_data.forja:energy` (Int); its capacity is in the item's spec in your mod. Then
`EnergyStorage { energy, capacity, max_receive, max_extract }` with the same rules.

## 7. Not done yet

Mekanism gases, the ME network (AE2 does it in its own mod, on top of section 8), multiblocks, comparators on machines
without a container; facades and spinning parts; fluid NBT and fluid tags. Light, sounds and particles stay in your mod.

## 8. Compound blocks (multipart)

Horizon has one state per block. forja adds a **host** per position (a block of your mod, invisible: template
`minecraft:barrier`) with a **centre** (a cable) and up to six **face parts**, the model of AE2's `CableBusContainer`.
forja keeps the host and parts in its world table, gives the outline and collision as the union of the parts' boxes,
aims the crosshair at the part under it (through the gaps of a cable), draws every part in the section mesh with the
state of a model block of your mod, places parts like a torch, breaks a part or the whole host (`break_whole`), takes a
part out with a wrench, and links centres to neighbours (`JoinFn`). Use: `register_host`, `register_part`
(`PartSpec::center`, `PartSpec::face`), `host_at`, `part_at`, `targeted_part`, `put_part`, `take_part`,
`hosts_generation`. AE2's `ae2_register_parts` is the full example.

## 9. Shapes from models

A content mod's block takes its outline, collision and crosshair shape from its vanilla template: a flat grate on a
`glass` template was a full cube. forja computes once, for every state of the mods' blocks, the boxes of the model the
blockstate picks (variant or every multipart piece, rotated as `BlockModelTransform` does), and uses them when they
differ from the template's shape. Only non-`minecraft:` blocks; templates whose shape is behaviour keep it (doors,
trapdoors, gates, fences, walls, panes, slabs, stairs, beds, scaffolding); collision follows the model only when the
template's collision is its outline. Projectiles, explosions and the camera keep the template's shape.

## 10. Mod networking and forja menus

Single player runs an integrated server; the window joins it as a guest, like LAN friends. The protocol only has
vanilla menus, so forja adds a typed channel over Horizon's connection (`custom_payload` on `forja:net`, split into
frames of at most 30,000 bytes and reassembled in order; a broken frame drops the whole message) and, on top of it, the
**forja menu**.

```rust
const NOTE: forja::Channel<MyMsg> = forja::Channel::new("mymod:note");   // MyMsg: forja::Wire
forja::register_server_channel(NOTE.id, on_server);
forja::register_client_channel(NOTE.id, on_client);   // or read with forja::client_take(NOTE.id)
forja::send_to_guest(host, conn, NOTE.id, &NOTE.encode(&msg));
forja::send_to_server(game, NOTE.id, &bytes);
forja::guest_has_forja(conn); forja::server_has_forja();
```

A forja menu runs the mod's own host code on the server **as the guest** (`as_guest`) and the client only mirrors the
slots and sends its input back, so a mod does not write its menu logic twice: `forja::register_menu(MenuKind {...})`,
then `forja::guest_menu`, `guest_sections`, `guest_widget`, `hud_batch` on the client. Any forja machine with
`register_screen` opened by a guest becomes the generic `forja:machine` menu. A client without forja (vanilla Java) gets
what it got before. forja also sends an item's `minecraft:custom_data` to clients, as vanilla does.

## 11. Redstone

Horizon asks a block for a signal per position only for states of class TrappedChest (or Jukebox) that are signal
sources, so a block that emits is declared with template `minecraft:trapped_chest`. Weak signal 0..15 on every face;
strong signal only towards the block below.

```rust
forja::set_redstone(game, pos, forja::Signal::side(Dir::North, 15, 15));
forja::set_part_redstone(game, host_pos, Some(Dir::North), signal);
forja::redstone_output(game, pos); forja::redstone_input(game, pos); forja::has_redstone_input(game, pos);
```

A change updates the neighbours; the table is per level and not saved (the mod sets it again from its ticks). A
multipart host emits through its `HostSpec.signal_block`, a twin block on the `trapped_chest` template.
