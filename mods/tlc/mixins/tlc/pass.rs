fn tlc_pass(
    checks: &StartChecks,
    queries: &mut dyn CheckQueries,
    chunk: ChunkPos,
) -> core::ops::ControlFlow<bool> {
    match tlc::castle_checks(checks, queries, chunk) {
        Some(may_start) => core::ops::ControlFlow::Break(may_start),
        None => core::ops::ControlFlow::Continue(()),
    }
}
