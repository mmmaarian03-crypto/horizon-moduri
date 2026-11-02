// PlayerMixin.updatePlayerPose: a gliding player who could swim here takes the SPIN_ATTACK pose
// (0.6 x 0.6, eyes at 0.4), and the rest of the method is skipped.
fn hangglider_pose(w: &mut MotionWorld<'_, '_, '_>) -> ::core::ops::ControlFlow<Result<()>> {
    if !crate::motion::hangglider_glide::get() {
        return ::core::ops::ControlFlow::Continue(());
    }
    match w.can_fit_pose(Pose::Swimming) {
        Ok(true) => ::core::ops::ControlFlow::Break(w.player.set_pose(Pose::SpinAttack)),
        Ok(false) => ::core::ops::ControlFlow::Continue(()),
        Err(e) => ::core::ops::ControlFlow::Break(Err(e)),
    }
}
