fn position(tab: &crate::creative::Tab) -> core::ops::ControlFlow<(f32, f32)> {
    if tab.id == "create:main" {
        // An extra column on the left uses the existing column-zero tab sprite.
        // All drawing, hover and click handling call this same function.
        core::ops::ControlFlow::Break((-28.0, -28.0))
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
