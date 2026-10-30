fn place(game: &mut crate::game::Game<'_>, pos: [i32;3], state: horizon_world::registry::BlockStateId, stack: &horizon_client::items::ItemStack) -> core::ops::ControlFlow<Result<BlockStateId>> {
    if !stack.item.starts_with("create:") || stack.data().and_then(|d|d.other("minecraft:block_state")).is_some() {return core::ops::ControlFlow::Continue(())}
    core::ops::ControlFlow::Break(crate::game::create_runtime::placed(game,pos,state))
}
