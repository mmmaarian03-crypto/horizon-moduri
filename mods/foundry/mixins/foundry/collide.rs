// Compound blocks collide with their parts' boxes; content-mod blocks with their model's boxes.
fn foundry_collide(block: CollisionBlock, p: BlockPos) -> ::core::ops::ControlFlow<io::Result<VoxelShape>> {
    match crate::mesher::foundry_shapes::collision(block.state.0, [p.x, p.y, p.z]) {
        Some(shape) => ::core::ops::ControlFlow::Break(Ok(shape)),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
