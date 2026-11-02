// The selection outline of the targeted part (or of a content-mod block's model boxes).
fn foundry_outline(edges: &[[f64; 6]], camera: glam::DVec3) -> ::core::ops::ControlFlow<Vec<[f32; 6]>> {
    match crate::mesher::foundry_shapes::outline(edges, camera) {
        Some(lines) => ::core::ops::ControlFlow::Break(lines),
        None => ::core::ops::ControlFlow::Continue(()),
    }
}
