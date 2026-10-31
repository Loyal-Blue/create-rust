#!/usr/bin/env python3
"""Run real Horizon recipe matching/remainders against shared crafter transactions."""
import argparse,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--horizon',type=Path,required=True);a=p.parse_args();r=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='create-crafting-tests-') as tmp:
 t=Path(tmp);(t/'src').mkdir();assets=t/'assets'
 for sub in ['data/minecraft/recipe','data/create/recipe','data/minecraft/loot_table','data/minecraft/tags/item','data/minecraft/tags/block','data/minecraft/tags/entity_type']:(assets/sub).mkdir(parents=True,exist_ok=True)
 ids=['minecraft:oak_planks','minecraft:stone','minecraft:stick','minecraft:crafting_table','create:andesite_alloy','create:crushing_wheel','create:mechanical_crafter','create:electron_tube','create:brass_casing','minecraft:milk_bucket','minecraft:bucket','minecraft:cake','minecraft:sugar','minecraft:egg','minecraft:wheat']
 items={id:{'minecraft:max_stack_size':64} for id in ids};items['minecraft:milk_bucket']={'minecraft:max_stack_size':1,'minecraft:use_remainder':{'id':'minecraft:bucket'}}
 (assets/'horizon-mod-content.json').write_text(json.dumps(['create']));(assets/'items.json').write_text(json.dumps(items));(assets/'data/minecraft/tags/item/planks.json').write_text(json.dumps({'values':['minecraft:oak_planks']}))
 (assets/'data/minecraft/recipe/stick.json').write_text(json.dumps({'type':'minecraft:crafting_shaped','pattern':['P','P'],'key':{'P':'#minecraft:planks'},'result':{'id':'minecraft:stick','count':4}}))
 (assets/'data/minecraft/recipe/cake.json').write_text(json.dumps({'type':'minecraft:crafting_shaped','pattern':['MMM','SES','WWW'],'key':{'M':'minecraft:milk_bucket','S':'minecraft:sugar','E':'minecraft:egg','W':'minecraft:wheat'},'result':{'id':'minecraft:cake'}}))
 for name in ['horizon_mechanical_crafter','horizon_mechanical_crushing_wheel']:(assets/f'data/create/recipe/{name}.json').write_bytes((r/f'content/data/create/recipe/{name}.json').read_bytes())
 manifest='[package]\nname="create-crafting-host-tests"\nversion="0.1.0"\nedition="2024"\n[dependencies]\n'
 manifest+='horizon-client={path='+json.dumps(str((a.horizon/'crates/horizon-client').resolve()))+',default-features=false}\n'
 for name,path in [('horizon-world',a.horizon/'crates/horizon-world'),('create-runtime',r/'crates/create-machines'),('create-simulation',r/'crates/create-recipes')]:manifest+=name+'={path='+json.dumps(str(path.resolve()))+'}\n'
 manifest+='serde_json="1"\n'
 (t/'Cargo.toml').write_text(manifest)
 n=r/'native/mixins/create_preview_support';body=(n/'machines_body.rs').read_text();bridge=body[body.index('fn portable('):body.index('fn ensure_machine(')]
 src='use create_simulation as recipes;use create_runtime as machines;use horizon_client::items::{ItemStack,Items};use horizon_client::crafting::Recipes;\n'+bridge+(n/'surface_items.rs').read_text()
 src+='\nfn main(){let root=std::path::Path::new('+json.dumps(str(assets))+');let items=Items::catalog(root,&horizon_world::loot::PackOverlay::NONE).unwrap();let recipes=Recipes::load(root,&items,&horizon_world::loot::PackOverlay::NONE).unwrap();\n'
 src+=r'''
 let pattern=[" AAA ","AAPAA","APSPA","AAPAA"," AAA "];
 let mut grid=Vec::new();for row in pattern {for c in row.chars(){grid.push(match c{'A'=>Some(ItemStack::new("create:andesite_alloy",2)),'P'=>Some(ItemStack::new("minecraft:oak_planks",2)),'S'=>Some(ItemStack::new("minecraft:stone",2)),_=>None});}}
 let (id,out)=recipes.find(&grid,5).expect("5x5 mechanical recipe");assert_eq!(&*out.item,"create:crushing_wheel");assert_eq!(out.count,2);assert!(recipes.find(&grid[..9],3).is_none());
 let book=horizon_client::recipe_book::Book::build(&recipes,&items);let entry=book.entries.iter().find(|e|&*e.result.item=="create:crushing_wheel").unwrap();assert!(!entry.fits(3,3));assert!(entry.fits(5,5));
 run(&recipes,&items,&grid,5,id,&out);
 let grid=vec![Some(ItemStack::new("minecraft:oak_planks",2)),None,Some(ItemStack::new("minecraft:oak_planks",2)),None];let (id,out)=recipes.find(&grid,2).unwrap();assert_eq!(&*out.item,"minecraft:stick");run(&recipes,&items,&grid,2,id,&out);
 let grid=["minecraft:milk_bucket","minecraft:milk_bucket","minecraft:milk_bucket","minecraft:sugar","minecraft:egg","minecraft:sugar","minecraft:wheat","minecraft:wheat","minecraft:wheat"].map(|id|Some(ItemStack::new(id,1))).to_vec();let (id,out)=recipes.find(&grid,3).unwrap();let after=run(&recipes,&items,&grid,3,id,&out);for slots in after.iter().take(3){assert_eq!(&*slots[0].as_ref().unwrap().item,"minecraft:bucket");}
 println!("PASS: real Horizon 5x5 crushing-wheel matching, vanilla-table size filtering, vanilla crafting and three bucket remainders commit through crafter inventories");
}
fn run(recipes:&Recipes,items:&Items,grid:&[Option<ItemStack>],size:usize,id:&str,out:&ItemStack)->Vec<Vec<Option<ItemStack>>>{
 let before:Vec<_>=grid.iter().map(|s|{let mut v=vec![None;27];v[0]=s.clone();v}).collect();let input:Vec<_>=before.iter().map(|s|portable(s)).collect();let rest=recipes.remainders(id,grid,size);let rests=portable(&rest);let output=portable(&[Some(out.clone())])[0].clone().unwrap();let key=if id.contains(':'){id.to_owned()}else{format!("minecraft:{id}")};let mut state=machines::MachineState::default();
 for _ in 0..100 {let u=machines::crafting::work(&state,&input,1,&key,&output,&rests,32.0,|id|items.get(id).map_or(1,|i|i.max_stack)).unwrap();state=machines::MachineState::decode(&u.state.encode()).unwrap();if u.result==machines::TickResult::Completed{let mut originals:Vec<_>=before.iter().flatten().cloned().collect();originals.push(Some(out.clone()));originals.extend(rest);let after:Vec<_>=u.after.iter().map(|s|restore(s,&originals).unwrap()).collect();assert_eq!(after[0][9].as_ref().unwrap(),out);for (slots,original) in after.iter().zip(grid){if let Some(s)=original && s.count>1 {assert_eq!(slots[0].as_ref().unwrap().count,s.count-1);}}return after;}}
 panic!("crafting transaction never finished");
}
'''
 (t/'src/main.rs').write_text(src);subprocess.run(['cargo','run','--manifest-path',str(t/'Cargo.toml'),'--quiet'],check=True)
