// An explosion: the undestroyable items go through it as nether stars (the only item its code spares), then get
// their own stacks back. RETURN is refused here (closures in the body), so the HEAD runs the explosion itself once
// with a re-entry flag ("RETURN from a HEAD", TRADUCATOR 2.12) and skips the original.
fn itemphysic_explode(
    game: &mut crate::game::Game<'_>,
    center: [f64; 3],
    radius: f32,
    damage_type: &str,
) -> ::core::ops::ControlFlow<()> {
    thread_local! {
        static INSIDE: ::core::cell::Cell<bool> = const { ::core::cell::Cell::new(false) };
    }
    if INSIDE.with(|f| f.get()) {
        return ::core::ops::ControlFlow::Continue(());
    }
    struct Out;
    impl Drop for Out {
        fn drop(&mut self) {
            INSIDE.with(|f| f.set(false));
        }
    }
    game.itemphysic_explosion_begin();
    INSIDE.with(|f| f.set(true));
    {
        let _out = Out;
        game.explosion_hurt_entities(center, radius, damage_type);
    }
    game.itemphysic_explosion_end();
    ::core::ops::ControlFlow::Break(())
}
