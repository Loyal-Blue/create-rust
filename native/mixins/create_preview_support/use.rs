fn use_block(game: &mut crate::game::Game<'_>, hit: Option<&crate::game::Hit>) -> core::ops::ControlFlow<Result<bool>> {
    crate::game::create_runtime::remember_hit(hit);
    if !game.may_build() || !game.held().is_some_and(|s| &*s.item=="create:wrench") {return core::ops::ControlFlow::Continue(())}
    let Some(hit)=hit else{return core::ops::ControlFlow::Continue(())};
    match crate::game::create_runtime::wrench_block(game,hit.pos) {
        Ok(true)=>core::ops::ControlFlow::Break(Ok(true)),
        Ok(false)=>core::ops::ControlFlow::Continue(()),
        Err(e)=>core::ops::ControlFlow::Break(Err(e)),
    }
}
