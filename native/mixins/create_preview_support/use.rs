fn use_block(game: &mut crate::game::Game<'_>, hit: Option<&crate::game::Hit>) -> core::ops::ControlFlow<Result<bool>> {
    crate::game::create_runtime::remember_hit(hit);
    if let Some(hit)=hit {
        match crate::game::create_runtime::use_depot(game,hit.pos) {
            Ok(true)=>return core::ops::ControlFlow::Break(Ok(true)),
            Err(e)=>return core::ops::ControlFlow::Break(Err(e)),
            _=>{}
        }
        match crate::game::create_runtime::use_transport(game,hit.pos) {
            Ok(true)=>return core::ops::ControlFlow::Break(Ok(true)),
            Err(e)=>return core::ops::ControlFlow::Break(Err(e)),
            _=>{}
        }
        match crate::game::create_runtime::use_machine(game,hit.pos) {
            Ok(true)=>return core::ops::ControlFlow::Break(Ok(true)),
            Err(e)=>return core::ops::ControlFlow::Break(Err(e)),
            _=>{}
        }
    }
    if !game.may_build() || !game.held().is_some_and(|s| &*s.item=="create:wrench") {return core::ops::ControlFlow::Continue(())}
    let Some(hit)=hit else{return core::ops::ControlFlow::Continue(())};
    match crate::game::create_runtime::wrench_block(game,hit.pos) {
        Ok(true)=>core::ops::ControlFlow::Break(Ok(true)),
        Ok(false)=>core::ops::ControlFlow::Continue(()),
        Err(e)=>core::ops::ControlFlow::Break(Err(e)),
    }
}
