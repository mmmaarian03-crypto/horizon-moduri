# foundry: the API for other native mods

`foundry` (a Forge-like layer for Horizon) is a shared native mod that the technical ports of this repository compile
into the same game binary. It gives them what Forge 47 (Minecraft 1.20.1) gave them through capabilities: energy (Forge Energy, FE
`int`), fluids (`FluidStack` / `FluidTank`, mB `int`) and item handlers, machine state saved with the world, a screen
helper, compound blocks (multipart), shapes from models, mod networking with menus for guests, and redstone.

The rules are plain Rust in `src/lib.rs` (tested in `tests/`); the game half is `native/horizon.rs`. Section numbers
below are the ones the other mods' comments cite.

## 0. A mod that uses foundry needs foundry

Native mods have no dependencies between them: `crate::game::foundry` exists only when `foundry` is in the same build. A
mod that calls it does not compile without it; its README says so. A mod's tests can use foundry's rules directly:
`[dev-dependencies] horizon-foundry = { path = "../foundry" }` (for `cargo test` only).

## 1. Where it lives

- The module is `crate::game::machines::foundry` (inside `game/machines.rs`, to use the hoppers' container views);
  `crate::game::foundry` re-exports it (`alias.rs`). Call `crate::game::foundry::...` from any mod under `crate::game`.
- Everything is `pub(in crate::game)`: visible from every mod's adapter, not from the `horizon_client` library.
- The rule types are in `crate::game::foundry::engine`, also re-exported (`foundry::Dir`, `foundry::Spec`,
  `foundry::FluidStack`, `foundry::EnergyHandler`...).
- `Dir` follows `Direction.get3DDataValue` (down, up, north, south, west, east). `face: Option<Dir>` is the face of the
  target block seen by the caller (as `getCapability(cap, side)`); `None` is the block itself, full access.

## 2. A machine with its state in foundry

For every registered block foundry keeps a `Machine`: energy, tanks with a role, item slots with a role, progress, free
integer data and per-face access. The table is per world and dimension, in `<world>/data/foundry/<dimension>.txt`,
written with the world; items are the game's NBT in hex.

```rust
use crate::game::foundry::{self, Access, Dir, SlotRole, Sides, Spec, TankRole};

fn furnator_spec() -> Spec {
    Spec::new("powah:furnator_starter")
        .energy(10_000, 0, 60)          // capacity, max_receive, max_extract
        .slots(&[SlotRole::Input])
}

// In the mod's world-load hook (idempotent):
foundry::register_machine(furnator_spec(), Some(furnator_tick));

fn furnator_tick(game: &mut Game<'_>, pos: [i32; 3]) {
    let burning = foundry::with_machine(game, pos, |m| { /* read and change the state */ true });
    // Move things only after the closure: foundry calls inside it answer "not here".
    if burning == Some(true) {
        foundry::push_energy_around(game, pos, 60);
    }
}
```

foundry handles by itself: the state on placement (or on first use after `/setblock`); a tick for every machine in a
block-ticking chunk; on any break, the slots' items drop at the next tick (energy and fluids are lost, as for a Forge
machine broken without a wrench); a changed spec (capacities follow, extra items drop); and every pipe, cable or machine
calling `foundry::energy_*`, `fluid_*` or `item_*` sees the machine through its roles and faces.
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

They work on any block: a foundry machine, another mod's provider, a vanilla container (items).

```rust
foundry::energy_info(game, p, face);  foundry::energy_receive(game, p, face, 1000, true);
foundry::transfer_energy(game, (from, Some(Dir::East)), (to, Some(Dir::West)), 1000);
foundry::fluid_fill(game, p, face, &FluidStack::new("minecraft:water", 1000), false);
foundry::transfer_fluid(game, (a, fa), (b, fb), None, 1000);
foundry::item_insert(game, p, face, stack, false);  foundry::transfer_items(game, (a, fa), (b, fb), 64);
```

Every move simulates both ends first; if the target still takes less than it promised, the rest goes back (energy,
fluid) or drops in the world (items), never lost. Fluids never mix. Buckets: water, lava, milk, and a mod's own with
`foundry::register_bucket`. Vanilla item containers follow the hoppers' rules.

## 4. Blocks that keep their own state (providers)

When a block's state is not in foundry's table (the Ender Tank's tank belongs to its frequency), the mod registers a
provider: a cheap check on `&Game` and the function that lends the handler for one operation
(`register_energy`, `register_fluid`, `register_items`). `claims` must be cheap and side-effect free; `f` is called once
per operation; a block with a foundry spec never reaches the providers; re-entry is safe ("not here", no deadlock).

## 5. A machine's screen (the ironchest pattern)

A mod cannot add a menu type, so a machine with items uses a vanilla container template (`minecraft:hopper` 5 slots,
`dispenser` 9, `barrel`/`chest` 27...): `BlockEntity::Chest` holds the items (hoppers and foundry see a vanilla
container, breaking spills it), the menu is vanilla, and foundry draws the mod's background and bars over it with
`foundry::register_screen` (`Gauge::Progress`, `Energy`, `Tank(i)`, `Data(value, max)`). The texture is carried under
`assets/<ns>/textures/block/...` so the GUI atlas packs it. Guests with foundry get it as a foundry menu (section 10).

## 6. Items with energy

An item's energy is `custom_data.foundry:energy` (Int); its capacity is in the item's spec in your mod. Then
`EnergyStorage { energy, capacity, max_receive, max_extract }` with the same rules.

## 7. Not done yet

Mekanism gases, the ME network (AE2 does it in its own mod, on top of section 8), multiblocks, comparators on machines
without a container; facades and spinning parts; fluid NBT and fluid tags. Light, sounds and particles stay in your mod.

## 8. Compound blocks (multipart)

Horizon has one state per block. foundry adds a **host** per position (a block of your mod, invisible: template
`minecraft:barrier`) with a **centre** (a cable) and up to six **face parts**, the model of AE2's `CableBusContainer`.
foundry keeps the host and parts in its world table, gives the outline and collision as the union of the parts' boxes,
aims the crosshair at the part under it (through the gaps of a cable), draws every part in the section mesh with the
state of a model block of your mod, places parts like a torch, breaks a part or the whole host (`break_whole`), takes a
part out with a wrench, and links centres to neighbours (`JoinFn`). Use: `register_host`, `register_part`
(`PartSpec::center`, `PartSpec::face`), `host_at`, `part_at`, `targeted_part`, `put_part`, `take_part`,
`hosts_generation`. AE2's `ae2_register_parts` is the full example.

## 9. Shapes from models

A content mod's block takes its outline, collision and crosshair shape from its vanilla template: a flat grate on a
`glass` template was a full cube. foundry computes once, for every state of the mods' blocks, the boxes of the model the
blockstate picks (variant or every multipart piece, rotated as `BlockModelTransform` does), and uses them when they
differ from the template's shape. Only non-`minecraft:` blocks; templates whose shape is behaviour keep it (doors,
trapdoors, gates, fences, walls, panes, slabs, stairs, beds, scaffolding); collision follows the model only when the
template's collision is its outline. Projectiles, explosions and the camera keep the template's shape.

## 10. Mod networking and foundry menus

Single player runs an integrated server; the window joins it as a guest, like LAN friends. The protocol only has
vanilla menus, so foundry adds a typed channel over Horizon's connection (`custom_payload` on `foundry:net`, split into
frames of at most 30,000 bytes and reassembled in order; a broken frame drops the whole message) and, on top of it, the
**foundry menu**.

```rust
const NOTE: foundry::Channel<MyMsg> = foundry::Channel::new("mymod:note");   // MyMsg: foundry::Wire
foundry::register_server_channel(NOTE.id, on_server);
foundry::register_client_channel(NOTE.id, on_client);   // or read with foundry::client_take(NOTE.id)
foundry::send_to_guest(host, conn, NOTE.id, &NOTE.encode(&msg));
foundry::send_to_server(game, NOTE.id, &bytes);
foundry::guest_has_foundry(conn); foundry::server_has_foundry();
```

A foundry menu runs the mod's own host code on the server **as the guest** (`as_guest`) and the client only mirrors the
slots and sends its input back, so a mod does not write its menu logic twice: `foundry::register_menu(MenuKind {...})`,
then `foundry::guest_menu`, `guest_sections`, `guest_widget`, `hud_batch` on the client. Any foundry machine with
`register_screen` opened by a guest becomes the generic `foundry:machine` menu. A client without foundry (vanilla Java) gets
what it got before. foundry also sends an item's `minecraft:custom_data` to clients, as vanilla does.

## 11. Redstone

Horizon asks a block for a signal per position only for states of class TrappedChest (or Jukebox) that are signal
sources, so a block that emits is declared with template `minecraft:trapped_chest`. Weak signal 0..15 on every face;
strong signal only towards the block below.

```rust
foundry::set_redstone(game, pos, foundry::Signal::side(Dir::North, 15, 15));
foundry::set_part_redstone(game, host_pos, Some(Dir::North), signal);
foundry::redstone_output(game, pos); foundry::redstone_input(game, pos); foundry::has_redstone_input(game, pos);
```

A change updates the neighbours; the table is per level and not saved (the mod sets it again from its ticks). A
multipart host emits through its `HostSpec.signal_block`, a twin block on the `trapped_chest` template.
