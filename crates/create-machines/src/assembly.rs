//! Ordered precision mechanism assembly. Item components are encoded by the host.
//! Weights follow pinned Create 6.0.10; RNG follows this port's prototype.
use super::Heat;
use create_simulation::{Ingredient,Product,Recipe};
use serde::{Deserialize,Serialize};
pub const ID:&str="create:sequenced_assembly/precision_mechanism";
pub const BASE:&str="create:golden_sheet";
pub const INTERMEDIATE:&str="create:incomplete_precision_mechanism";
pub const RESULT:&str="create:precision_mechanism";
pub const OPERATIONS:u8=15;
pub const RESULTS:[(&str,u32);9]=[(RESULT,120),(BASE,8),("create:andesite_alloy",8),("create:cogwheel",5),("minecraft:gold_nugget",3),("create:shaft",2),("create:crushed_raw_gold",2),("minecraft:iron_ingot",1),("minecraft:clock",1)];
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress{pub version:u8,pub recipe:String,pub completed:u8}
impl Progress{
 pub fn start()->Self{Self{version:1,recipe:ID.into(),completed:0}}
 pub fn valid(&self)->bool{self.version==1&&self.recipe==ID&&self.completed<OPERATIONS}
 pub fn decode(s:&str)->Result<Self,String>{if s.len()>1024{return Err("Assembly data exceeds limit".into());}let p:Self=serde_json::from_str(s).map_err(|e|e.to_string())?;if !p.valid(){return Err("Unsupported assembly progress".into());}Ok(p)}
 pub fn encode(&self)->String{serde_json::to_string(self).expect("assembly metadata")}
 pub fn tool(&self)->Option<&'static str>{self.valid().then(||["create:cogwheel","create:large_cogwheel","minecraft:iron_nugget"][usize::from(self.completed%3)])}
 pub fn recipe(&self)->Option<Recipe>{if !self.valid(){return None;}let stage=if self.completed==0{0}else{usize::from(self.completed%3)+1};Some(Recipe{id:STEP_IDS[stage],station:"deploying",ingredients:&INGREDIENTS[stage],products:&PRODUCTS,work:100,heat:Heat::None})}
 pub fn advance(&self,rng:&mut u64)->Option<Outcome>{if !self.valid(){return None;}if self.completed+1<OPERATIONS{let mut next=self.clone();next.completed+=1;Some(Outcome::Progress(next))}else{if *rng==0{*rng=0x4d595df4d0f33173;}*rng^=*rng<<13;*rng^=*rng>>7;*rng^=*rng<<17;Some(Outcome::Finished(result_for((*rng%150)as u32)?))}}
}
#[derive(Clone,Debug,PartialEq,Eq)]pub enum Outcome{Progress(Progress),Finished(&'static str)}
pub fn result_for(mut ticket:u32)->Option<&'static str>{for(id,weight)in RESULTS{if ticket<weight{return Some(id);}ticket-=weight;}None}
const STEP_IDS:[&str;4]=["create:assembly/precision_start","create:assembly/precision_cog","create:assembly/precision_large_cog","create:assembly/precision_nugget"];
const fn i(matcher:&'static str)->Ingredient{Ingredient{matcher,count:1,fluid:false,consume:true}}
const INGREDIENTS:[[Ingredient;2];4]=[[i(BASE),i("create:cogwheel")],[i(INTERMEDIATE),i("create:cogwheel")],[i(INTERMEDIATE),i("create:large_cogwheel")],[i(INTERMEDIATE),i("#c:nuggets/iron")]];
const PRODUCTS:[Product;1]=[Product{id:INTERMEDIATE,count:1,fluid:false,chance:1.}];
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn all_fifteen_operations_follow_five_ordered_cycles(){let mut p=Progress::start();let mut rng=1;for n in 0..15{assert_eq!(p.completed,n);assert_eq!(p.tool(),Some(["create:cogwheel","create:large_cogwheel","minecraft:iron_nugget"][usize::from(n%3)]));assert_eq!(p.recipe().unwrap().ingredients[0].matcher,if n==0{BASE}else{INTERMEDIATE});match p.advance(&mut rng).unwrap(){Outcome::Progress(next)=>{assert!(n<14);p=Progress::decode(&next.encode()).unwrap();},Outcome::Finished(id)=>{assert_eq!(n,14);assert!(RESULTS.iter().any(|(s,_)|*s==id));}}}}
 #[test]fn result_intervals_match_every_original_weight(){let mut counts=std::collections::BTreeMap::new();for n in 0..150{*counts.entry(result_for(n).unwrap()).or_insert(0)+=1;}for(id,weight)in RESULTS{assert_eq!(counts[&id],weight);}assert_eq!(result_for(150),None);assert_eq!(result_for(u32::MAX),None);}
 #[test]fn invalid_or_future_metadata_is_rejected(){for s in ["{}","{\"version\":1,\"recipe\":\"other:recipe\",\"completed\":1}","{\"version\":2,\"recipe\":\"create:sequenced_assembly/precision_mechanism\",\"completed\":1}"]{assert!(Progress::decode(s).is_err());}let mut p=Progress::start();p.completed=15;assert!(p.recipe().is_none());assert!(p.advance(&mut 1).is_none());assert!(Progress::decode(&p.encode()).is_err());}
 #[test]fn unfinished_steps_do_not_draw_final_randomness(){let mut rng=42;Progress::start().advance(&mut rng).unwrap();assert_eq!(rng,42);}
 #[test]fn saved_random_state_repeats_a_blocked_final_result(){let mut p=Progress::start();p.completed=14;let(mut a,mut b)=(1234,1234);assert_eq!(p.advance(&mut a),p.advance(&mut b));assert_eq!(a,b);assert_ne!(a,1234);}
 #[test]fn wrong_tool_does_not_apply_or_consume(){let p=Progress::start();let input=super::super::Stack{id:BASE.into(),count:1,components:String::new()};let mut hand=vec![None;27];hand[0]=Some(super::super::Stack{id:"create:large_cogwheel".into(),count:1,components:String::new()});assert!(super::super::applying::select(&input,&hand,&[p.recipe().unwrap()]).is_none());}
}
