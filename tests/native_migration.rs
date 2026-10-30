use super::*;
fn registry()->horizon_world::registry::BlockRegistry {
    use horizon_world::nbt::Tag;
    let mut states=vec![];let mut defaults=vec![];
    for name in ["create:creative_motor","create:mechanical_press"] {
        defaults.push((name,Tag::Int(states.len() as i32)));
        for face in ["south","north","east","west","up","down"] {
            states.push(horizon_world::chunk::BlockState{name:name.into(),properties:std::collections::BTreeMap::from([("facing".into(),face.into())])}.to_nbt());
        }
    }
    horizon_world::registry::BlockRegistry::from_nbt(&Tag::compound([("DataVersion",Tag::Int(horizon_world::DATA_VERSION)),("states",Tag::List(states)),("defaults",Tag::compound(defaults))])).unwrap()
}
#[test]fn legacy_motor_axis_and_waterlogging_migrate(){
    let registry=registry();let old=horizon_world::chunk::BlockState{name:"create:creative_motor".into(),properties:std::collections::BTreeMap::from([("axis".into(),"x".into()),("waterlogged".into(),"false".into())])};
    let core::ops::ControlFlow::Break(Ok(id))=migrate(&registry,&old)else{panic!("not migrated")};
    assert_eq!(registry.state(id).unwrap().properties,std::collections::BTreeMap::from([("facing".into(),"east".into())]));
}
#[test]fn legacy_upright_machine_stays_upright(){
    let registry=registry();let old=horizon_world::chunk::BlockState{name:"create:mechanical_press".into(),properties:std::collections::BTreeMap::new()};
    let core::ops::ControlFlow::Break(Ok(id))=migrate(&registry,&old)else{panic!("not migrated")};assert_eq!(registry.state(id).unwrap().properties["facing"],"south");
}
#[test]fn modern_facing_is_preserved(){let registry=registry();let s=registry.state(horizon_world::registry::BlockStateId(1)).unwrap();assert!(matches!(migrate(&registry,s),core::ops::ControlFlow::Continue(())));}
