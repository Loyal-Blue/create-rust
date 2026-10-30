fn receive(game:&mut crate::game::Game<'_>,packet:&horizon_protocol::packets::play::Clientbound)->core::ops::ControlFlow<Result<()>>{
    if crate::game::create_runtime::receive_machines(game,packet){core::ops::ControlFlow::Break(Ok(()))}else{core::ops::ControlFlow::Continue(())}
}
