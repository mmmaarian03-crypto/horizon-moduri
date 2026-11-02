fn oreberries_inert(this: &mut J, r: &Rename) -> core::ops::ControlFlow<()> {
    if let J::Str(s) = this
        && r.to.starts_with("oreberriesreplanted:")
        && oreberriesreplanted::inert(s)
    {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
