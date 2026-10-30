fn tank_member_states(game: &Game<'_>,group:&machines::tanks::Group)->BTreeMap<[i32;3],machines::MachineState> {
    group.members.iter().map(|p|(*p,read_machine(game,(p[0],p[1],p[2])))).collect()
}
fn shared_tanks(game:&Game<'_>,positions:&[(MachinePos,String)],nodes:&mut BTreeMap<[i32;3],machines::fluids::Node>)->Vec<machines::tanks::Group> {
    let tanks=positions.iter().filter(|(_,name)|name=="create:fluid_tank").map(|(p,_)|[p.0,p.1,p.2]).collect();
    let groups:Vec<_>=machines::tanks::groups(&tanks).into_iter().filter(|g|g.gather(&tank_member_states(game,g)).is_some()).collect();
    for g in &groups {
        let port=g.controller();
        for p in &g.members {nodes.insert(*p,machines::fluids::Node::Alias{port});}
        nodes.insert(port,machines::fluids::Node::GroupPort{capacity:g.capacity()});
    }
    groups
}
fn commit_tank_flow(game:&mut Game<'_>,changes:BTreeMap<[i32;3],machines::MachineState>,groups:&[machines::tanks::Group])->bool {
    let mut updates=BTreeMap::new();
    for (p,view) in changes {
        if let Some(group)=groups.iter().find(|g|g.controller()==p) {
            let originals=tank_member_states(game,group);
            let Some(scattered)=group.scatter(&view,&originals) else {return false};
            for (q,state) in scattered {updates.insert(q,state);}
        }else{updates.insert(p,view);}
    }
    // Every group expansion succeeds before any persistent record is written.
    for (p,state) in updates {store_machine(game,(p[0],p[1],p[2]),state);}
    true
}
pub(super) fn use_shared_tank(game:&mut Game<'_>,p:[i32;3])->Result<bool> {
    let pos=(p[0],p[1],p[2]);
    if game.remote.is_some() || !game.may_build() || machine_name(game,pos)!="create:fluid_tank" {return Ok(false);}
    let held=game.held().cloned();
    let sneaking=game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    if sneaking && held.is_some() {return Ok(false);}
    let positions=all_create(game);let mut nodes=BTreeMap::new();
    let Some(group)=shared_tanks(game,&positions,&mut nodes).into_iter().find(|g|g.members.contains(&p)) else {return Ok(false)};
    let original=tank_member_states(game,&group);
    let Some(mut view)=group.gather(&original) else {return Ok(false)};
    let used:u32=view.fluid_in.values().chain(view.fluid_out.values()).sum();
    if held.is_none() {
        if !sneaking {return Ok(false);}
        transport_notice(game,pos,&format!("Shared tank: {used}/{} mB, {} blocks, {}",group.capacity(),group.members.len(),view.fluid_in.keys().next().map(String::as_str).unwrap_or("empty")));
        return Ok(true);
    }
    let held=held.unwrap();
    let fluid=match &*held.item {
        "minecraft:water_bucket"=>Some("minecraft:water"),"minecraft:lava_bucket"=>Some("minecraft:lava"),
        "minecraft:milk_bucket"=>Some("minecraft:milk"),"create:honey_bucket"=>Some("create:honey"),"create:chocolate_bucket"=>Some("create:chocolate"),_=>None,
    };
    let remainder=if let Some(fluid)=fluid {
        if !machines::tanks::fill(&mut view,group.capacity(),fluid,1000) {
            transport_notice(game,pos,"Shared tank is full or contains a different fluid");return Ok(true);
        }
        "minecraft:bucket"
    }else if &*held.item=="minecraft:bucket" {
        let Some(bucket)=[("minecraft:water","minecraft:water_bucket"),("minecraft:lava","minecraft:lava_bucket"),("minecraft:milk","minecraft:milk_bucket"),("create:honey","create:honey_bucket"),("create:chocolate","create:chocolate_bucket")].into_iter().find_map(|(fluid,bucket)|view.extract_fluid(fluid,1000).then_some(bucket)) else {
            transport_notice(game,pos,"Shared tank has less than a bucket of supported fluid");return Ok(true);
        };
        bucket
    }else{return Ok(false)};
    let Some(scattered)=group.scatter(&view,&original) else {return Ok(true)};
    for (q,state) in scattered {store_machine(game,(q[0],q[1],q[2]),state);}
    if !game.creative {game.consume_held();}
    if !game.creative || &*held.item=="minecraft:bucket" {
        if let Some(left)=game.give(ItemStack::new(remainder,1)){game.player_drop(left);}
    }
    transport_notice(game,pos,&format!("Shared tank: {} / {} mB",view.fluid_in.values().chain(view.fluid_out.values()).sum::<u32>(),group.capacity()));
    Ok(true)
}
