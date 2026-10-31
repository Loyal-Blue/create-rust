// Placement consumes only after a supported plan and a current inventory
// snapshot. Host block acceptance is explicit; rejection refunds the input.
fn tick_deployer_placement(game:&mut Game<'_>,p:MachinePos,rpm:f32)->bool {
    if game.with_level(|b,l,t|b.has_neighbor_signal(l,t,horizon_core::position::BlockPos::new(p.0,p.1,p.2))) {
        let mut state=read_machine(game,p);state.status="Redstone paused".into();store_machine(game,p,state);return true;
    }
    let Some(entity)=game.world.block_entities.get(&p) else{return false};
    let before=entity.slots().to_vec();
    let portable_before=portable(&before);
    let state=read_machine(game,p);
    let mut stock=recipes::Stock{fluids:state.fluid_in.clone(),slots:usize::MAX,tank_capacity:machines::TANK_CAPACITY,..Default::default()};
    for s in portable_before.iter().take(9).flatten(){*stock.items.entry(s.id.clone()).or_default()+=s.count;}
    let catalog=surface_catalog(game,"deploying");
    if recipes::matching_recipe(&stock,"deploying",&catalog).is_some(){return false;}
    let step=facing_step(game,p);
    let Some(target)=machines::placing::target([p.0,p.1,p.2],step) else{return false};
    let Some(mut plan)=machines::placing::plan(&state,&portable_before,target,rpm) else{return false};
    if plan.result==machines::TickResult::NoPower {
        if matches!(state.status.as_str(),"Overstressed"|"Conflict"){plan.state.status=state.status;}
        store_machine(game,p,plan.state);return true;
    }
    let middle=offset_machine(p,step,1);
    let clear=game.world.loaded(middle.0,middle.2) && game.world.loaded(target[0],target[2])
        && game.res.facts().is_air(game.world.block(middle.0,middle.1,middle.2))
        && game.res.facts().is_air(game.world.block(target[0],target[1],target[2]));
    let block=game.pack.assets.registry.default_state(&plan.item)
        .filter(|id|game.res.facts().get(*id).full_collision);
    let unobstructed=clear && block.is_some_and(|id|game.with_level(|_,level,_|
        horizon_world::level::LevelAccess::is_unobstructed(level,horizon_core::position::BlockPos::new(target[0],target[1],target[2]),id)));
    if !unobstructed {
        plan.state.progress=0.0;plan.state.status="Placement obstructed".into();
        store_machine(game,p,plan.state);return true;
    }
    if plan.result!=machines::TickResult::Completed {store_machine(game,p,plan.state);return true;}
    let Some(after)=restore(&plan.inventory.after,&before) else{return true};
    let Some(entity)=game.world.block_entities.get_mut(&p) else{return true};
    if entity.slots()!=before {return true;}
    entity.slots_mut().clone_from_slice(&after);
    let placed=game.with_level(|b,level,ticks|b.set_block(level,ticks,
        horizon_core::position::BlockPos::new(target[0],target[1],target[2]),block.unwrap(),horizon_world::level::flags::UPDATE_ALL));
    if !placed {
        if let Some(entity)=game.world.block_entities.get_mut(&p).filter(|e|e.slots()==after) {
            entity.slots_mut().clone_from_slice(&before);
        }else{
            game.spawn_item(ItemStack::new(&plan.item,1),[p.0 as f64+0.5,p.1 as f64+0.5,p.2 as f64+0.5],[0.0,0.0,0.0],10);
        }
        plan.state.status="Placement refused; input returned".into();
    }
    if game.process_level_events().is_err() {plan.state.status="Placement event update failed".into();}
    dirty_machine(game,p);store_machine(game,p,plan.state);true
}
