// A section with compound blocks: meshed as usual, then their parts are added.
fn foundry_mesh(
    this: &mut Mesher<'_>,
    region: &[[Option<&Chunk>; 3]; 3],
    sy: i32,
) -> ::core::ops::ControlFlow<SectionMesh> {
    match foundry_shapes::mesh(this, region, sy) {
        Some(mesh) => ::core::ops::ControlFlow::Break(mesh),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
