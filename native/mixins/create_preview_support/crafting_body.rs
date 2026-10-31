fn store_crafting_status(game:&mut Game<'_>,cells:&[[i32;3]],controller:machines::MachineState) {
    for (i,p) in cells.iter().enumerate(){let mut s=read_machine(game,(p[0],p[1],p[2]));s.progress=controller.progress;s.recipe=controller.recipe.clone();s.status=controller.status.clone();s.craft_binding=if i==0{controller.craft_binding.clone()}else{None};store_machine(game,(p[0],p[1],p[2]),s);}
}
fn reset_crafting(game:&mut Game<'_>,cells:&[[i32;3]],text:&str){let p=cells[0];let mut s=read_machine(game,(p[0],p[1],p[2]));s.progress=0.0;s.recipe=None;s.craft_binding=None;s.status=text.into();store_crafting_status(game,cells,s);}
fn tick_crafting(game:&mut Game<'_>,positions:&[(MachinePos,String)],speeds:&std::collections::HashMap<[i32;3],f32>)->BTreeSet<MachinePos> {
    let all:BTreeSet<_>=positions.iter().filter(|(_,n)|n=="create:mechanical_crafter").map(|(p,_)|*p).collect();
    let nodes=all.iter().map(|p|{let f=facing_step(game,*p);let face=match f{[0,0,-1]=>0,[0,0,1]=>1,[1,0,0]=>2,[-1,0,0]=>3,_=>4};([p.0,p.1,p.2],face)}).collect();
    let mut invalid=all.clone();
    for grid in machines::crafting::grids(&nodes) {
        for p in &grid.cells{invalid.remove(&(p[0],p[1],p[2]));}
        let before:Option<Vec<_>>=grid.cells.iter().map(|p|game.world.block_entities.get(&(p[0],p[1],p[2])).map(|e|e.slots().to_vec())).collect();
        let Some(before)=before else{reset_crafting(game,&grid.cells,"Waiting for member inventory");continue};
        let size=grid.width.max(grid.height);let mut square=vec![None;size*size];
        for (i,slots) in before.iter().enumerate(){square[(i/grid.width)*size+i%grid.width]=slots.iter().take(9).flatten().find(|s|s.count>0).cloned();}
        let found=game.recipes().find(&square,size).map(|(id,result)|(id.to_owned(),result));
        let Some((id,output))=found else{reset_crafting(game,&grid.cells,"Awaiting crafting pattern");continue};
        if game.items().get(&output.item).is_none(){reset_crafting(game,&grid.cells,"Crafting output is not installed");continue;}
        let raw_remainders=game.recipes().remainders(&id,&square,size);
        let remainders:Vec<_>=(0..grid.cells.len()).map(|i|raw_remainders[(i/grid.width)*size+i%grid.width].clone()).collect();
        let portable_before:Vec<_>=before.iter().map(|s|portable(s)).collect();let portable_rest=portable(&remainders);let portable_output=portable(&[Some(output.clone())])[0].clone().unwrap();
        let controller=grid.cells[0];let state=read_machine(game,(controller[0],controller[1],controller[2]));
        let mut rpm=grid.cells.iter().map(|p|speeds.get(p).copied().unwrap_or(0.0).abs()).fold(f32::INFINITY,f32::min);
        let paused=grid.cells.iter().any(|p|game.with_level(|b,l,t|b.has_neighbor_signal(l,t,horizon_core::position::BlockPos::new(p[0],p[1],p[2]))));if paused{rpm=0.0;}
        let key=if id.contains(':'){id.clone()}else{format!("minecraft:{id}")};
        let Some(mut update)=machines::crafting::work(&state,&portable_before,grid.identity(),&key,&portable_output,&portable_rest,rpm,|id|game.items().get(id).map_or(1,|i|i.max_stack)) else{continue};
        if paused{update.state.status="Redstone paused".into();}
        else if update.result==machines::TickResult::NoPower && matches!(state.status.as_str(),"Overstressed"|"Conflict"){update.state.status=state.status.clone();}
        let mut originals:Vec<_>=before.iter().flatten().cloned().collect();originals.push(Some(output));originals.extend(remainders);
        let after:Option<Vec<_>>=update.after.iter().map(|s|restore(s,&originals)).collect();let Some(after)=after else{continue};
        if grid.cells.iter().zip(&before).any(|(p,s)|game.world.block_entities.get(&(p[0],p[1],p[2])).is_none_or(|e|e.slots()!=*s)){continue;}
        for ((p,slots),old) in grid.cells.iter().zip(&after).zip(&before){if slots!=old{if let Some(e)=game.world.block_entities.get_mut(&(p[0],p[1],p[2])){e.slots_mut().clone_from_slice(slots);}dirty_machine(game,(p[0],p[1],p[2]));}}
        store_crafting_status(game,&grid.cells,update.state);
    }
    for p in invalid {reset_crafting(game,&[[p.0,p.1,p.2]],"Use a complete horizontal-facing rectangle (max 9 x 9)");}
    all
}
