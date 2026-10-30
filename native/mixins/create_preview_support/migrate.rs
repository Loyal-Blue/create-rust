fn migrate(registry: &crate::registry::BlockRegistry, descriptor: &crate::chunk::BlockState) -> core::ops::ControlFlow<io::Result<BlockStateId>> {
    let known = matches!(descriptor.name.as_str(),"create:creative_motor"|"create:encased_fan"|"create:mechanical_drill"|"create:deployer"|"create:mechanical_pump"|"create:mechanical_press"|"create:mechanical_mixer"|"create:mechanical_saw"|"create:millstone"|"create:hand_crank");
    if !known || descriptor.properties.contains_key("facing") {return core::ops::ControlFlow::Continue(())}
    let Some(default) = registry.default_state(&descriptor.name).and_then(|s|registry.state(s)) else{return core::ops::ControlFlow::Continue(())};
    if !default.properties.contains_key("facing") {return core::ops::ControlFlow::Continue(())}
    let mut migrated=default.clone();
    let upright=matches!(descriptor.name.as_str(),"create:mechanical_press"|"create:mechanical_mixer"|"create:mechanical_saw"|"create:millstone"|"create:hand_crank"|"create:deployer");
    let facing=if upright {"south"}else{match descriptor.properties.get("axis").map(String::as_str){Some("x")=>"east",Some("y")=>"up",_=>"south"}};
    migrated.properties.insert("facing".into(),facing.into());
    core::ops::ControlFlow::Break(registry.resolve(&migrated))
}
