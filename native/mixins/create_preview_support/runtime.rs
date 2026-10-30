#[allow(dead_code)]
mod kinetic {
// Engine-independent prototype. No Horizon runtime adapter exists yet.
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    East,
    West,
    Up,
    Down,
    South,
    North,
}
impl Face {
    pub fn axis(self) -> Axis {
        match self {
            Self::East | Self::West => Axis::X,
            Self::Up | Self::Down => Axis::Y,
            _ => Axis::Z,
        }
    }
    pub fn opposite(self) -> Self {
        match self {
            Self::East => Self::West,
            Self::West => Self::East,
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::South => Self::North,
            Self::North => Self::South,
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
            Self::East => Self::Up,
            Self::Up => Self::Down,
            Self::Down => Self::South,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Placement {
    Axial,
    HorizontalAxial,
    Directional,
    Upright,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Axis(Axis),
    Facing(Face),
}
/// Written placement policy, pending in-game parity validation against Create.
/// Normal axial placement aligns to an existing drive when available; sneaking
/// bypasses alignment. Directional machines face the player in all six directions.
pub fn placement(
    kind: Placement,
    clicked: Face,
    look: Face,
    horizontal: Face,
    sneaking: bool,
    neighbour: Option<Axis>,
) -> Orientation {
    match kind {
        Placement::Axial => Orientation::Axis(if sneaking {
            clicked.axis()
        } else {
            neighbour.unwrap_or(clicked.axis())
        }),
        Placement::HorizontalAxial => Orientation::Axis(
            match if sneaking {
                clicked.axis()
            } else {
                neighbour.unwrap_or(clicked.axis())
            } {
                Axis::Y => horizontal.axis(),
                a => a,
            },
        ),
        Placement::Directional => {
            Orientation::Facing(if sneaking { look } else { look.opposite() })
        }
        Placement::Upright => Orientation::Facing(horizontal.opposite()),
    }
}
pub fn wrench(orientation: Orientation, reverse: bool) -> Orientation {
    match orientation {
        Orientation::Axis(a) => Orientation::Axis(match (a, reverse) {
            (Axis::X, false) | (Axis::Z, true) => Axis::Y,
            (Axis::Y, false) | (Axis::X, true) => Axis::Z,
            _ => Axis::X,
        }),
        Orientation::Facing(mut f) => {
            for _ in 0..if reverse { 5 } else { 1 } {
                f = f.next()
            }
            Orientation::Facing(f)
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos(pub i32, pub i32, pub i32);
impl Pos {
    fn along(self, axis: Axis, sign: i32) -> Self {
        match axis {
            Axis::X => Self(self.0 + sign, self.1, self.2),
            Axis::Y => Self(self.0, self.1 + sign, self.2),
            Axis::Z => Self(self.0, self.1, self.2 + sign),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Shaft,
    WaterWheel,
    CreativeMotor(f64),
}
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub axis: Axis,
    pub kind: Kind,
    pub rpm: f64,
    pub angle: f64,
    pub jammed: bool,
}
impl Node {
    pub fn new(axis: Axis, kind: Kind) -> Self {
        Self {
            axis,
            kind,
            rpm: 0.0,
            angle: 0.0,
            jammed: false,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct FlowSample {
    pub offset: [f64; 3],
    pub velocity: [f64; 3],
}
/// Tangential flow torque around the wheel axis. Still water has no torque.
/// This is a prototype physical rule, not an assertion of original Create parity.
pub fn wheel_rpm(axis: Axis, flow: &[FlowSample]) -> f64 {
    let a = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    };
    let torque: f64 = flow
        .iter()
        .map(|s| {
            let r = s.offset;
            let v = s.velocity;
            [
                r[1] * v[2] - r[2] * v[1],
                r[2] * v[0] - r[0] * v[2],
                r[0] * v[1] - r[1] * v[0],
            ][a]
        })
        .sum();
    if !torque.is_finite() || torque.abs() < 1e-6 {
        0.0
    } else {
        torque.signum() * 8.0
    }
}
#[derive(Default)]
pub struct Network {
    pub nodes: BTreeMap<Pos, Node>,
}
impl Network {
    /// Recompute from topology every tick: removing a source or connection stops
    /// disconnected shafts immediately. Only coaxial, directly touching shafts
    /// and sources connect in this first stage. Conflicting drives jam safely.
    pub fn tick(&mut self, seconds: f64, flows: &BTreeMap<Pos, Vec<FlowSample>>) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        let mut remaining: std::collections::BTreeSet<_> = self.nodes.keys().copied().collect();
        for node in self.nodes.values_mut() {
            node.rpm = 0.0;
            node.jammed = false;
        }
        while let Some(start) = remaining.pop_first() {
            let mut queue = VecDeque::from([start]);
            let mut group = vec![];
            let mut drive: Option<f64> = None;
            let mut jam = false;
            while let Some(pos) = queue.pop_front() {
                let node = self.nodes[&pos];
                group.push(pos);
                let rpm = match node.kind {
                    Kind::Shaft => 0.0,
                    Kind::CreativeMotor(r) => {
                        if r.is_finite() {
                            r.clamp(-256.0, 256.0)
                        } else {
                            0.0
                        }
                    }
                    Kind::WaterWheel => {
                        wheel_rpm(node.axis, flows.get(&pos).map(Vec::as_slice).unwrap_or(&[]))
                    }
                };
                if rpm != 0.0 {
                    if let Some(old) = drive {
                        if (old - rpm).abs() > 1e-6 {
                            jam = true
                        }
                    } else {
                        drive = Some(rpm)
                    }
                }
                for sign in [-1, 1] {
                    let next = pos.along(node.axis, sign);
                    if self.nodes.get(&next).is_some_and(|n| n.axis == node.axis)
                        && remaining.remove(&next)
                    {
                        queue.push_back(next)
                    }
                }
            }
            for pos in group {
                let n = self.nodes.get_mut(&pos).unwrap();
                n.jammed = jam;
                n.rpm = if jam { 0.0 } else { drive.unwrap_or(0.0) };
                n.angle = (n.angle + n.rpm * 6.0 * seconds).rem_euclid(360.0);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directional_placement_has_all_six_faces() {
        for f in [
            Face::Up,
            Face::Down,
            Face::East,
            Face::West,
            Face::North,
            Face::South,
        ] {
            assert_eq!(
                placement(
                    Placement::Directional,
                    Face::Up,
                    f,
                    Face::South,
                    false,
                    None
                ),
                Orientation::Facing(f.opposite())
            );
        }
    }
    #[test]
    fn shaft_alignment_can_be_bypassed() {
        assert_eq!(
            placement(
                Placement::Axial,
                Face::Up,
                Face::North,
                Face::North,
                false,
                Some(Axis::X)
            ),
            Orientation::Axis(Axis::X)
        );
        assert_eq!(
            placement(
                Placement::Axial,
                Face::Up,
                Face::North,
                Face::North,
                true,
                Some(Axis::X)
            ),
            Orientation::Axis(Axis::Y)
        );
    }
    #[test]
    fn wheels_stay_horizontal_and_machines_upright() {
        assert_eq!(
            placement(
                Placement::HorizontalAxial,
                Face::Up,
                Face::Down,
                Face::East,
                false,
                None
            ),
            Orientation::Axis(Axis::X)
        );
        assert_eq!(
            placement(
                Placement::Upright,
                Face::Down,
                Face::Up,
                Face::West,
                false,
                None
            ),
            Orientation::Facing(Face::East)
        );
    }
    #[test]
    fn wrench_is_reversible() {
        for o in [Orientation::Axis(Axis::X), Orientation::Facing(Face::Down)] {
            assert_eq!(wrench(wrench(o, false), true), o)
        }
    }
    fn flow(v: f64) -> Vec<FlowSample> {
        vec![FlowSample {
            offset: [1.0, 0.0, 0.0],
            velocity: [0.0, v, 0.0],
        }]
    }
    #[test]
    fn still_water_stops_and_reversed_flow_reverses() {
        assert_eq!(wheel_rpm(Axis::Z, &flow(0.0)), 0.0);
        assert_eq!(wheel_rpm(Axis::Z, &flow(-1.0)), -8.0);
        assert_eq!(wheel_rpm(Axis::Z, &flow(1.0)), 8.0);
    }
    #[test]
    fn radial_flow_does_not_drive_a_wheel() {
        assert_eq!(wheel_rpm(Axis::X, &flow(1.0)), 0.0)
    }
    #[test]
    fn connected_shaft_rotates_then_stops_when_source_removed() {
        let mut n = Network::default();
        n.nodes
            .insert(Pos(0, 0, 0), Node::new(Axis::Z, Kind::WaterWheel));
        n.nodes
            .insert(Pos(0, 0, 1), Node::new(Axis::Z, Kind::Shaft));
        let flows = BTreeMap::from([(Pos(0, 0, 0), flow(1.0))]);
        n.tick(1.0, &flows);
        assert_eq!(n.nodes[&Pos(0, 0, 1)].rpm, 8.0);
        assert_eq!(n.nodes[&Pos(0, 0, 1)].angle, 48.0);
        n.nodes.remove(&Pos(0, 0, 0));
        n.tick(1.0, &flows);
        assert_eq!(n.nodes[&Pos(0, 0, 1)].rpm, 0.0);
    }
    #[test]
    fn perpendicular_shafts_do_not_connect() {
        let mut n = Network::default();
        n.nodes
            .insert(Pos(0, 0, 0), Node::new(Axis::Z, Kind::CreativeMotor(16.0)));
        n.nodes
            .insert(Pos(0, 0, 1), Node::new(Axis::X, Kind::Shaft));
        n.tick(1.0, &BTreeMap::new());
        assert_eq!(n.nodes[&Pos(0, 0, 1)].rpm, 0.0);
    }
    #[test]
    fn incompatible_sources_jam_and_recover() {
        let mut n = Network::default();
        for (z, rpm) in [(0, 16.0), (1, -16.0)] {
            n.nodes
                .insert(Pos(0, 0, z), Node::new(Axis::Z, Kind::CreativeMotor(rpm)));
        }
        n.tick(1.0, &BTreeMap::new());
        assert!(n.nodes.values().all(|n| n.jammed && n.rpm == 0.0));
        n.nodes.remove(&Pos(0, 0, 1));
        n.tick(1.0, &BTreeMap::new());
        assert_eq!(n.nodes[&Pos(0, 0, 0)].rpm, 16.0);
    }
    #[test]
    fn rotation_wraps_and_frame_duration_is_consistent() {
        let mut a = Network::default();
        a.nodes
            .insert(Pos(0, 0, 0), Node::new(Axis::X, Kind::CreativeMotor(64.0)));
        let mut b = Network {
            nodes: a.nodes.clone(),
        };
        a.tick(1.0, &BTreeMap::new());
        for _ in 0..20 {
            b.tick(0.05, &BTreeMap::new());
        }
        assert!((a.nodes[&Pos(0, 0, 0)].angle - b.nodes[&Pos(0, 0, 0)].angle).abs() < 1e-9);
    }
}

}
use super::{Game,Hit,Screen};
use std::{cell::Cell,collections::BTreeMap,sync::{Mutex,OnceLock,atomic::Ordering}};
use horizon_world::{registry::BlockStateId,fluid::FluidState,fluid_flow::FluidFlowWorld};
use horizon_core::position::BlockPos;
use horizon_client::{Result,items::ItemStack,mesher::Vertex,hud::{HudBatch,rgba}};
use kinetic::{Axis,Face,Orientation,Placement,Pos,Kind,Node,Network,FlowSample};
thread_local! {static HIT_FACE:Cell<Option<([i32;3],Face)>>=const{Cell::new(None)};}
fn face(v:[i32;3])->Face {match v{[1,0,0]=>Face::East,[-1,0,0]=>Face::West,[0,1,0]=>Face::Up,[0,-1,0]=>Face::Down,[0,0,-1]=>Face::North,_=>Face::South}}
fn face_name(f:Face)->&'static str {match f{Face::East=>"east",Face::West=>"west",Face::Up=>"up",Face::Down=>"down",Face::North=>"north",Face::South=>"south"}}
fn axis_name(a:Axis)->&'static str {match a{Axis::X=>"x",Axis::Y=>"y",Axis::Z=>"z"}}
fn horizontal(yaw:f32)->Face {match ((yaw/90.0).round() as i32).rem_euclid(4) {0=>Face::South,1=>Face::West,2=>Face::North,_=>Face::East}}
fn read_axis(s:&horizon_world::chunk::BlockState)->Axis {match s.properties.get("axis").map(String::as_str).unwrap_or("y"){"x"=>Axis::X,"z"=>Axis::Z,_=>Axis::Y}}
pub(super) fn remember_hit(hit:Option<&Hit>){HIT_FACE.with(|f|f.set(hit.map(|h|(h.pos,face(h.face)))));}
pub(super) fn placed(game:&mut Game<'_>,pos:[i32;3],state:BlockStateId)->Result<BlockStateId>{
    let Some(s)=game.pack.assets.registry.state(state) else{return Ok(state)};
    let name=s.name.clone();if !name.starts_with("create:"){return Ok(state)}
    let Some((clicked,clicked_face))=HIT_FACE.with(Cell::get) else{return Ok(state)};
    let normal=match clicked_face {Face::East=>[1,0,0],Face::West=>[-1,0,0],Face::Up=>[0,1,0],Face::Down=>[0,-1,0],Face::North=>[0,0,-1],Face::South=>[0,0,1]};
    if pos!=clicked&&pos!=[clicked[0]+normal[0],clicked[1]+normal[1],clicked[2]+normal[2]] {return Ok(state)};

    let sneak=game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
    let horizontal=horizontal(game.yaw);
    let look=if game.pitch>45.0{Face::Down}else if game.pitch< -45.0{Face::Up}else{horizontal};
    let kind=if s.properties.contains_key("facing") {
        if matches!(name.as_str(),"create:mechanical_press"|"create:mechanical_mixer"|"create:mechanical_saw"|"create:millstone"|"create:hand_crank"){Placement::Upright}else{Placement::Directional}
    }else if s.properties.contains_key("axis"){
        if name.ends_with("water_wheel"){Placement::HorizontalAxial}else{Placement::Axial}
    }else{return Ok(state)};
    let mut neighbour=None;
    for d in [[0,1,0],[0,-1,0],[1,0,0],[-1,0,0],[0,0,1],[0,0,-1]] {
        let p=[pos[0]+d[0],pos[1]+d[1],pos[2]+d[2]];
        if let Some(n)=game.pack.assets.registry.state(game.world.block(p[0],p[1],p[2])) {
            if n.name.starts_with("create:")&&n.properties.contains_key("axis")&&read_axis(n)==face(d).axis(){neighbour=Some(read_axis(n));break}
        }
    }
    let orientation=kinetic::placement(kind,clicked_face,look,horizontal,sneak,neighbour);
    let (key,value)=match orientation{Orientation::Axis(a)=>("axis",axis_name(a)),Orientation::Facing(f)=>("facing",face_name(f))};
    let new=game.with_props(state,&[(key,value)]).unwrap_or(state);
    if new!=state{game.level_set(pos,new)?;}Ok(new)
}
pub(super) fn wrench_block(game:&mut Game<'_>,pos:[i32;3])->Result<bool>{
    let state=game.world.block(pos[0],pos[1],pos[2]);let Some(s)=game.pack.assets.registry.state(state)else{return Ok(false)};
    if !s.name.starts_with("create:"){return Ok(false)}
    let (key,values):(&str,&[&str])=if s.properties.contains_key("facing") {if matches!(s.name.as_str(),"create:mechanical_press"|"create:mechanical_mixer"|"create:mechanical_saw"|"create:millstone"|"create:hand_crank") {("facing",&["south","west","north","east"])} else {("facing",&["south","west","north","east","up","down"])}}else if s.properties.contains_key("axis"){("axis",&["x","y","z"])}else{return Ok(false)};
    let index=values.iter().position(|v|s.properties.get(key).is_some_and(|s|s==v)).unwrap_or(0);
    let step=if game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft){values.len()-1}else{1};
    let next=values[(index+step)%values.len()];if let Some(new)=game.with_props(state,&[(key,next)]) {game.level_set(pos,new)?;return Ok(true)}Ok(false)
}
struct Water<'a,'b>{game:&'a Game<'b>}
impl FluidFlowWorld for Water<'_,'_>{
    type Error=std::io::Error;type Block=BlockStateId;
    fn fluid(&mut self,p:BlockPos)->std::io::Result<FluidState>{Ok(self.game.res.behaviours().fluid(self.game.world.block(p.x,p.y,p.z)))}
    fn block(&mut self,p:BlockPos)->std::io::Result<BlockStateId>{Ok(self.game.world.block(p.x,p.y,p.z))}
    fn blocks_fluid_flow(&self,s:BlockStateId)->bool{self.game.pack.assets.registry.state(s).is_some_and(|s|self.game.pack.assets.blocks_fluid_flow.contains(&s.name))}
    fn is_ice_class(&self,s:BlockStateId)->bool{self.game.res.facts().get(s).classes.iter().any(|c|c.as_ref()=="IceBlock")}
    fn is_face_sturdy(&mut self,s:BlockStateId,_:BlockPos,d:horizon_core::geometry::Direction)->std::io::Result<bool>{Ok(self.game.res.facts().get(s).sturdy_full&(1<<d as u8)!=0)}
}
#[derive(Default)]
struct Cache{world:usize,sections:BTreeMap<(i32,usize,i32),(std::sync::Arc<horizon_client::chunk::Section>,Vec<(Pos,u32)>)>,network:Network,last_time:f64}
fn dynamic(name:&str)->bool {matches!(name,"create:shaft"|"create:water_wheel"|"create:large_water_wheel")}
fn refresh(game:&Game<'_>,cache:&mut Cache){
    let world=&game.world as *const _ as usize;
    if cache.world!=world{*cache=Cache{world,..Cache::default()};}
    let mut live=std::collections::BTreeSet::new();
    let mut nodes=BTreeMap::new();
    for (&(cx,cz),chunk) in &game.world.chunks{
        for (sy,section) in chunk.sections.iter().enumerate(){
            let key=(cx,sy,cz);live.insert(key);let token=std::sync::Arc::clone(section);
            let changed=cache.sections.get(&key).is_none_or(|(old,_)|!std::sync::Arc::ptr_eq(old,&token));
            if changed{
                let mut found=vec![];
                // Skip uniform air/stone sections before walking the palette.
                if !section.blocks.uniform().is_some_and(|id|game.pack.assets.registry.state(BlockStateId(id)).is_none_or(|s|!dynamic(&s.name)&&s.name!="create:creative_motor")){
                    for i in 0..4096{
                        let id=section.get(i);let Some(s)=game.pack.assets.registry.state(BlockStateId(id))else{continue};
                        if dynamic(&s.name)||s.name=="create:creative_motor"{let pos=Pos(cx*16+(i&15) as i32,chunk.min_y()+sy as i32*16+(i>>8) as i32,cz*16+((i>>4)&15) as i32);found.push((pos,id));}
                    }
                }
                cache.sections.insert(key,(token,found));
            }
            for &(pos,id) in &cache.sections[&key].1{
                let s=game.pack.assets.registry.state(BlockStateId(id)).unwrap();
                let axis=if s.name=="create:creative_motor"{match s.properties.get("facing").map(String::as_str){Some("east"|"west")=>Axis::X,Some("north"|"south")=>Axis::Z,_=>Axis::Y}}else{read_axis(s)};
                let kind=if s.name=="create:shaft"{Kind::Shaft}else if s.name=="create:creative_motor"{Kind::CreativeMotor(16.0)}else{Kind::WaterWheel};
                let mut node=Node::new(axis,kind);if let Some(old)=cache.network.nodes.get(&pos){node.angle=old.angle;}nodes.insert(pos,node);
            }
        }
    }
    cache.sections.retain(|k,_|live.contains(k));cache.network.nodes=nodes;
}
fn flows(game:&Game<'_>,network:&Network)->BTreeMap<Pos,Vec<FlowSample>>{
    let mut out=BTreeMap::new();let mut water=Water{game};
    for (&pos,node) in &network.nodes{
        if !matches!(node.kind,Kind::WaterWheel){continue}
        let s=game.pack.assets.registry.state(game.world.block(pos.0,pos.1,pos.2)).unwrap();let radius:i32=if s.name=="create:large_water_wheel"{2}else{1};
        let mut samples=vec![];
        for u in -radius..=radius{for v in -radius..=radius{
            if u.abs()!=radius&&v.abs()!=radius{continue}
            let offset=match node.axis{Axis::X=>[0,u,v],Axis::Y=>[u,0,v],Axis::Z=>[u,v,0]};let p=BlockPos::new(pos.0+offset[0],pos.1+offset[1],pos.2+offset[2]);
            let Ok(fluid)=water.fluid(p)else{continue};if fluid.kind()!=horizon_world::fluid::FluidKind::Water{continue}
            if let Ok(flow)=horizon_world::fluid_flow::flow_at(&mut water,fluid,p){samples.push(FlowSample{offset:offset.map(f64::from),velocity:[flow.x,flow.y,flow.z]});}
        }}out.insert(pos,samples);
    }out
}
pub(super) fn draw_rotation(game:&mut Game<'_>,camera:glam::DVec3,alpha:f64,out:&mut Vec<Vertex>){
    static CACHE:OnceLock<Mutex<Cache>>=OnceLock::new();let Ok(mut cache)=CACHE.get_or_init(||Mutex::new(Cache::default())).lock()else{return};
    refresh(game,&mut cache);let time=game.world.game_time as f64+alpha;let dt=if time>=cache.last_time{(time-cache.last_time).min(20.0)/20.0}else{0.0};cache.last_time=time;
    let flow=flows(game,&cache.network);cache.network.tick(dt,&flow);
    for (&pos,node) in &cache.network.nodes{
        let at=glam::DVec3::new(pos.0 as f64+0.5,pos.1 as f64+0.5,pos.2 as f64+0.5)-camera;if at.length_squared()>96.0*96.0{continue}
        let state_id=game.world.block(pos.0,pos.1,pos.2);let Some(state)=game.pack.assets.registry.state(state_id)else{continue};if !dynamic(&state.name){continue}
        let item=ItemStack::new(&state.name,1);let model=game.resolve_item(&item,horizon_assets::item_model::DisplayContext::None,None,0);
        let rotation=glam::Mat4::from_axis_angle(match node.axis{Axis::X=>glam::Vec3::X,Axis::Y=>glam::Vec3::Y,Axis::Z=>glam::Vec3::Z},(node.angle as f32).to_radians());
        let orientation=match node.axis{Axis::Y=>glam::Mat4::IDENTITY,Axis::X=>glam::Mat4::from_rotation_z(-std::f32::consts::FRAC_PI_2),Axis::Z=>glam::Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2)};
        let pose=glam::Mat4::from_translation(at.as_vec3())*rotation*orientation;
        game.submit_item_world(&model,pose,horizon_client::block_entity_render::vertex_light(game.world.light(pos.0,pos.1,pos.2)),out);
    }
}
pub(super) fn draw_pages(game:&mut Game<'_>,batch:&mut HudBatch){
    if game.screen!=Screen::Creative||!game.creative_screen.tabs.iter().any(|t|t.id=="create:main"){return}
    let (x,y)=Game::creative_origin(batch);let page=horizon_client::mod_content::create_preview_support::PAGE.load(Ordering::Relaxed);
    batch.fill(&game.gui.atlas,x+65.0,y-48.0,65.0,17.0,rgba(35,35,35,255));
    batch.text(&game.gui.atlas,&format!("<   {} / 2   >",page+1),x+68.0,y-44.0,u32::MAX,false);
}
pub(super) fn click_page(game:&mut Game<'_>)->bool{
    let Some(r)=&game.renderer else{return false};let batch=HudBatch::new([r.config.width,r.config.height]);let (x,y)=Game::creative_origin(&batch);let (mx,my)=game.cursor;
    if mx<x+65.0||mx>=x+130.0||my<y-48.0||my>=y-31.0{return false}
    let page=&horizon_client::mod_content::create_preview_support::PAGE;let target=1-page.load(Ordering::Relaxed);
    let index=game.creative_screen.tabs.iter().position(|t|if target==1{t.id=="create:main"}else{t.id!="create:main"&&!t.search&&!t.inventory});let Some(index)=index else{return false};
    page.store(target,Ordering::Relaxed);super::select_creative_tab(&mut game.creative_screen,&mut game.inv_clicks,index);game.inv_drag=None;game.last_inv_click=None;true
}
