// The hidden blocks are air in the section's mesh.
fn a_good_place_blank(
    this: &mut Mesher<'_>,
    region: &[[Option<&Chunk>; 3]; 3],
    sy: i32,
) -> ::core::ops::ControlFlow<SectionMesh> {
    let Some(c) = region[1][1] else {
        return ::core::ops::ControlFlow::Continue(());
    };
    let base = [c.x * 16, MIN_Y + sy * 16, c.z * 16];
    // The padded cube reaches two cells (`B` in the original mesher) outside the section.
    if !a_good_place_hidden::near(base, 2) || !a_good_place_hidden::enter() {
        return ::core::ops::ControlFlow::Continue(());
    }
    let mut copies: Vec<Option<Chunk>> = Vec::with_capacity(9);
    for row in region {
        for chunk in row {
            copies.push(chunk.map(|c| {
                let mut copy = c.clone();
                for p in a_good_place_hidden::inside(c.x, c.z) {
                    copy.set(p[0] & 15, p[1], p[2] & 15, 0);
                }
                copy
            }));
        }
    }
    let blanked: [[Option<&Chunk>; 3]; 3] =
        std::array::from_fn(|z| std::array::from_fn(|x| copies[z * 3 + x].as_ref()));
    let mesh = this.mesh(&blanked, sy);
    a_good_place_hidden::leave();
    ::core::ops::ControlFlow::Break(mesh)
}
