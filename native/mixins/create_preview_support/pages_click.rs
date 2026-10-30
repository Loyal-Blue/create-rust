fn click_page(game: &mut crate::game::Game<'_>, right: bool) -> core::ops::ControlFlow<()> {
    if !right && crate::game::create_runtime::click_page(game) {core::ops::ControlFlow::Break(())}else{core::ops::ControlFlow::Continue(())}
}
