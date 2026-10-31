"""Author the isolated Automation Lab layout; no player folders are accessed."""
import json,argparse
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--catalog',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
r=Path(__file__).resolve().parents[2];c=r/'content';config=json.loads((c/'data/create/create_preview/support.json').read_text());catalog=json.loads(a.catalog.read_text());known={x['id'] for x in config['items']}
blocks={};cases=[];containers={};states={}
def pos(x,y,z):return [int(x),int(y),int(z)]
def key(p):return ','.join(map(str,p))
def put(p,name,**props):
 v={'pos':list(p),'name':name if ':' in name else 'create:'+name,'properties':{k:str(v).lower() for k,v in props.items()}}
 if key(p) in blocks and blocks[key(p)]!=v:raise ValueError(f'Layout collision at {p}: {blocks[key(p)]} versus {v}')
 blocks[key(p)]=v;return list(p)
def machine(p,name,**props):
 put(p,name,**props);states.setdefault(key(p),{'pos':list(p),'name':name if ':' in name else 'create:'+name,'data':{}});return list(p)
def state(p,**values):states[key(p)]['data'].update(values)
def inventory(p,items,kind='chest'):
 slots=[]
 for i,s in items.items():
  if s:slots.append({'slot':int(i),'id':s[0],'count':s[1],**({'name':s[2]} if len(s)>2 else {})})
 containers[key(p)]={'pos':list(p),'kind':kind,'items':slots};return list(p)
def chest(p,items):put(p,'minecraft:chest',facing='north',type='single',waterlogged=False);return inventory(p,items)
def sign(p,lines):put(p,'minecraft:oak_sign',rotation=0,waterlogged=False);containers[key(p)]={'pos':list(p),'kind':'sign','lines':lines[:4]}
faces={(1,0,0):'east',(-1,0,0):'west',(0,1,0):'up',(0,-1,0):'down',(0,0,1):'south',(0,0,-1):'north'}
def plus(p,d,n=1):return [p[i]+d[i]*n for i in range(3)]
def lever(case,p,facing,powered=True):
 d={'west':(1,0,0),'north':(0,0,1)}[facing];put(plus(p,d),'minecraft:stone');put(p,'minecraft:lever',face='wall',facing=facing,powered=powered);case['controls'].append({'pos':list(p),'name':'Pause drive' if powered else 'Reverse gearshift','initial_on':powered});return p
def drive(case,p,back=(0,0,1),rpm=32,direction=1):
 axis='xyz'[next(i for i,v in enumerate(back) if v)]
 machine(plus(p,back),'shaft',axis=axis);clutch=machine(plus(p,back,2),'clutch',axis=axis)
 m=machine(plus(p,back,3),'creative_motor',facing=faces[tuple(-v for v in back)]);state(m,motor_rpm=rpm,direction=direction)
 side=(0,0,1) if axis=='x' else (1,0,0);lever(case,plus(clutch,side),'north' if axis=='x' else 'west')
 return m

def bay(title,kind='machine'):
 i=len(cases);x=16+(i%5)*32;z=24+(i//5)*32
 case={'id':f'{i+1:02}','title':title,'kind':kind,'origin':[x,65,z],'controls':[],'targets':[],'expected':[]};cases.append(case)
 sign([x+3,65,z+2],[f'{i+1:02} {title[:17]}','ON = paused','OFF = run','Read LAB_GUIDE.md'])
 for dx in range(24):put([x+dx,64,z],'minecraft:cyan_concrete')
 return case,x,z

def seed(p,station,output=None,amount=8):
 recipes=[q for q in catalog['recipes'] if q['station']==station and all(t['fluid'] or t['id'].startswith('minecraft:') or t['id'] in known for t in q['products'])]
 preferred=[q for q in recipes if output and any(t['id']==output for t in q['products'])]
 if preferred:recipes=preferred
 for recipe in recipes:
  entries={};fluids={};ok=True
  for ing in recipe['ingredients']:
   matcher=ing['matcher'];ids=catalog['tags'].get(matcher[1:],[]) if matcher.startswith('#') else [matcher]
   ids=[id for id in ids if id.startswith('minecraft:') or id in known]
   if not ids:ok=False;break
   id=ids[0];n=ing['count']*(1 if not ing['consume'] else amount)
   if ing['fluid']:fluids[id]=min(n,8000)
   else:entries[len(entries)]=(id,min(n,64))
  if ok:
   inventory(p,entries)
   if fluids:state(p,fluid_in=fluids)
   return recipe
 raise ValueError(f'No installed recipe for {station}/{output}')

def dropper(case,p,item,count=16):
 put(p,'minecraft:dropper',facing='down',triggered=False);inventory(p,{0:(item,count)},'dropper');button=plus(p,(0,0,-1));put(button,'minecraft:stone_button',face='wall',facing='north',powered=False);case['controls'].append({'pos':button,'name':'Drop one input after drives run','initial_on':False})

case,x,z=bay('Geared press')
p=machine([x+10,67,z+8],'mechanical_press',facing='north');small=machine([x+10,67,z+9],'cogwheel',axis='z');large=machine([x+9,66,z+9],'large_cogwheel',axis='z');drive(case,large,rpm=16)
depot=machine([x+10,65,z+8],'depot');inventory(depot,{0:('minecraft:iron_ingot',16)});case['targets']=[{'pos':p,'rpm':-32}];case['expected']=['16 iron sheets in depot middle row'];case['recipe']='pressing'
case,x,z=bay('Heated mixer')
basin=machine([x+8,66,z+8],'basin');burner=machine([x+8,65,z+8],'blaze_burner');state(burner,burn_until=240000)
p=machine([x+8,68,z+8],'mechanical_mixer',facing='north');drive(case,p,back=(0,1,0));recipe=seed(basin,'mixing','create:brass_ingot',4);case['targets']=[{'pos':p,'rpm':32}];case['recipe_id']=recipe['id'];case['heat']='Heated';case['expected']=['Mixing result in basin middle row'];chest([x+3,65,z+8],{0:('minecraft:coal',32),1:('create:blaze_cake',8)})
case,x,z=bay('Millstone')
p=machine([x+8,69,z+8],'millstone',facing='north');drive(case,p,back=(0,-1,0));recipe=seed(p,'milling','create:wheat_flour',8);case['targets']=[{'pos':p,'rpm':32}];case['recipe_id']=recipe['id'];case['expected']=['Milled outputs in millstone middle row']
case,x,z=bay('Crushing pair')
for dz,direction in [(7,1),(9,-1)]:
 p=machine([x+9,68,z+dz],'crushing_wheel',axis='x');drive(case,p,back=((-1,0,0) if dz==7 else (1,0,0)),direction=direction);case['targets'].append({'pos':p,'rpm':32*direction})
dropper(case,[x+9,71,z+8],'minecraft:raw_iron');chute=machine([x+9,65,z+8],'chute');inventory(chute,{});chest([x+9,64,z+8],{});case['expected']=['Crushed raw iron falls into chute and collection chest']
case,x,z=bay('Upward saw')
p=machine([x+8,65,z+8],'mechanical_saw',facing='up');drive(case,p);dropper(case,[x+8,68,z+8],'create:andesite_alloy');case['targets']=[{'pos':p,'rpm':32}];case['expected']=['Six shafts rest on the saw after one dropped alloy']
case,x,z=bay('World drill')
p=machine([x+8,65,z+8],'mechanical_drill',facing='north');drive(case,p);target=put([x+8,65,z+7],'minecraft:dirt');case['targets']=[{'pos':p,'rpm':32}];case['expected']=['Dirt target breaks and drops'];case['break_target']=target
for title,medium,station,output in [('Fan washing','minecraft:water','splashing','minecraft:clay_ball'),('Fan smoking','minecraft:fire','smoking','minecraft:bread'),('Fan haunting','minecraft:soul_fire','haunting','minecraft:soul_sand'),('Fan blasting','minecraft:lava','blasting','minecraft:iron_ingot')]:
 case,x,z=bay(title);p=machine([x+8,65,z+10],'encased_fan',facing='north');drive(case,p)
 if medium in ['minecraft:water','minecraft:lava']:put([x+8,65,z+9],medium,level=0)
 else:
  put([x+8,64,z+9],'minecraft:soul_soil' if 'soul' in medium else 'minecraft:netherrack');put([x+8,65,z+9],medium,**({} if 'soul' in medium else dict(age=0,east=False,north=False,south=False,up=False,west=False)))
 # Glass contains the medium without obstructing the stream itself.
 for q in [[x+7,65,z+9],[x+9,65,z+9]]:put(q,'minecraft:glass')
 depot=machine([x+8,65,z+7],'depot')
 if station in ('smoking','blasting'):
  inventory(depot,{0:('minecraft:beef' if station=='smoking' else 'minecraft:raw_iron',8)});recipe={'id':'minecraft:cooked_beef' if station=='smoking' else 'minecraft:iron_ingot_from_blasting_raw_iron'}
 else:recipe=seed(depot,station,output,8)
 case['targets']=[{'pos':p,'rpm':32}];case['recipe_id']=recipe['id'];case['expected']=['Fan-processed output in depot middle row']
case,x,z=bay('Belt pressing')
for dx in range(6,15):machine([x+dx,65,z+8],'belt',facing='east')
drive(case,[x+6,65,z+8]);p=machine([x+10,67,z+8],'mechanical_press',facing='north');drive(case,p)
chest([x+4,65,z+8],{0:('minecraft:iron_ingot',32)});f=machine([x+5,65,z+8],'brass_funnel',facing='east');state(f,extracting=True,transfer_limit=16);lever(case,[x+5,65,z+9],'north')
chest([x+16,65,z+8],{});f=machine([x+15,65,z+8],'brass_funnel',facing='west');state(f,extracting=False,item_filter='create:iron_sheet');case['targets']=[{'pos':p,'rpm':32},{'pos':[x+14,65,z+8],'rpm':32}];case['expected']=['Run both drives, then feed; sheets reach east chest']
case,x,z=bay('Belt spout')
for dx in range(6,15):machine([x+dx,65,z+8],'belt',facing='east')
drive(case,[x+6,65,z+8]);p=machine([x+10,67,z+8],'spout');state(p,fluid_in={'minecraft:water':8000})
chest([x+4,65,z+8],{0:('minecraft:glass_bottle',8)});f=machine([x+5,65,z+8],'brass_funnel',facing='east');state(f,extracting=True,transfer_limit=1);lever(case,[x+5,65,z+9],'north')
chest([x+16,65,z+8],{});f=machine([x+15,65,z+8],'brass_funnel',facing='west');state(f,item_filter='minecraft:potion');case['targets']=[{'pos':[x+14,65,z+8],'rpm':32}];case['expected']=['Water bottles reach east chest; spout fluid decreases']
case,x,z=bay('Branched pump')
for dx in range(3,6):
 for dz in range(7,10):put([x+dx,65,z+dz],'minecraft:water',level=0)
p=machine([x+6,65,z+8],'mechanical_pump',facing='east');cog=machine([x+6,66,z+8],'cogwheel',axis='x');drive(case,cog,back=(1,0,0),direction=-1)
for q in [[x+7,65,z+8],[x+8,65,z+8],[x+8,65,z+7],[x+8,65,z+9]]:machine(q,'fluid_pipe')
for dz in [7,9]:machine([x+9,65,z+dz],'fluid_tank')
case['targets']=[{'pos':p,'rpm':32}];case['expected']=['Both outlet tanks fill from renewable water; one shared pump budget']
case,x,z=bay('Shared tank')
for dx in range(2):
 for dz in range(2):
  for dy in range(3):
   p=machine([x+7+dx,65+dy,z+7+dz],'fluid_tank')
   if dy==0 and dz==0:state(p,fluid_in={'minecraft:water':8000})
chest([x+3,65,z+8],{i:('minecraft:water_bucket' if i<4 else 'minecraft:bucket',1 if i<4 else 16) for i in range(8)});case['expected']=['Shared volume starts at 16000 / 96000 mB; any member is a port']
case,x,z=bay('Item drain')
p=machine([x+8,65,z+8],'item_drain');dropper(case,[x+8,68,z+8],'minecraft:water_bucket',1);inventory([x+8,68,z+8],{i:('minecraft:water_bucket',1) for i in range(8)},'dropper');case['expected']=['Feed one bucket; collect returned bucket on top; 1000 mB stored']
case,x,z=bay('World deployer')
p=machine([x+8,65,z+8],'deployer',facing='north');inventory(p,{0:('minecraft:stone',16),1:('minecraft:stone',1,'Do not erase my name')});drive(case,p,back=(0,1,0));put([x+8,64,z+6],'minecraft:gold_block');case['targets']=[{'pos':p,'rpm':32}];case['expected']=['One plain stone is placed above gold target; named input remains']
case,x,z=bay('5x5 crafting')
pattern=[' AAA ','AAPAA','APSPA','AAPAA',' AAA ']
for row in range(5):
 for col in range(5):
  p=machine([x+11-col,69-row,z+8],'mechanical_crafter',facing='north');ch=pattern[row][col];inventory(p,{} if ch==' ' else {0:({'A':'create:andesite_alloy','P':'minecraft:oak_planks','S':'minecraft:stone'}[ch],2)})
controller=[x+11,69,z+8];drive(case,controller);case['targets']=[{'pos':controller,'rpm':32}];case['expected']=['Four wheels collect in top-left middle row after two crafts'];case['crafting_grid']=True
case,x,z=bay('Small water wheel','wheel')
p=machine([x+8,67,z+8],'water_wheel',axis='x');r=1
for dy in range(-1,4):put([x+8,67+dy,z+8+r],'minecraft:water',level=0 if dy==3 else 8);put([x+8,67+dy,z+9+r],'minecraft:glass')
# An output clutch controls the downstream press; the wheel itself remains live.
shaft=machine([x+9,67,z+8],'shaft',axis='x');cl=machine([x+10,67,z+8],'clutch',axis='x');lever(case,[x+10,67,z+9],'north');machine([x+11,67,z+8],'mechanical_press',facing='east');depot=machine([x+11,65,z+8],'depot');inventory(depot,{0:('minecraft:iron_ingot',8)});case['wheel']=p;case['expected']=['Falling water rotates wheel; releasing clutch drives a depot press']
case,x,z=bay('Large water wheel','wheel')
p=machine([x+8,68,z+8],'large_water_wheel',axis='x');r=2
for dy in range(-2,5):put([x+8,68+dy,z+8+r],'minecraft:water',level=0 if dy==4 else 8);put([x+8,68+dy,z+9+r],'minecraft:glass')
machine([x+9,68,z+8],'shaft',axis='x');cl=machine([x+10,68,z+8],'clutch',axis='x');lever(case,[x+10,68,z+9],'north');machine([x+11,68,z+8],'mechanical_press',facing='east');depot=machine([x+11,66,z+8],'depot');inventory(depot,{0:('minecraft:iron_ingot',8)});case['wheel']=p;case['expected']=['Large wheel rotates at prototype 4 RPM and powers press']
case,x,z=bay('Gearbox and chain')
p=machine([x+8,65,z+8],'gearbox',axis='y');drive(case,p,back=(-1,0,0));machine([x+8,65,z+9],'shaft',axis='z');case['targets']=[{'pos':[x+8,65,z+9],'rpm':-32}]
for dx in range(3):machine([x+14+dx,65,z+8],'encased_chain_drive',axis='z');machine([x+14+dx,65,z+9],'shaft',axis='z')
drive(case,[x+14,65,z+8],back=(0,0,-1));case['targets'].append({'pos':[x+16,65,z+9],'rpm':32});case['expected']=['Gearbox turns the drive; chain cases share RPM across side outputs']
# Second hall: small control demonstrations and gallery.
case,x,z=bay('Gearshift reversal')
p=machine([x+8,65,z+8],'gearshift',axis='x');drive(case,p,back=(-1,0,0));machine([x+9,65,z+8],'shaft',axis='x');lever(case,[x+8,65,z+9],'north',False);case['targets']=[{'pos':[x+9,65,z+8],'rpm':32}];case['expected']=['Run drive, then toggle second lever to reverse output']
case,x,z=bay('Source conflict','conflict')
a1=machine([x+4,65,z+8],'creative_motor',facing='east');state(a1,motor_rpm=32);machine([x+5,65,z+8],'shaft',axis='x');cl=machine([x+6,65,z+8],'clutch',axis='x');lever(case,[x+6,65,z+9],'north')
for dx in [7,8]:machine([x+dx,65,z+8],'shaft',axis='x')
b=machine([x+9,65,z+8],'creative_motor',facing='west');state(b,motor_rpm=32,direction=-1);case['targets']=[{'pos':[x+8,65,z+8],'rpm':0}];case['expected']=['Releasing clutch connects opposing sources and reports Conflict']
case,x,z=bay('Hand crank stress','stress')
p=machine([x+10,65,z+8],'hand_crank',facing='east');machine([x+9,65,z+8],'shaft',axis='x');machine([x+8,65,z+8],'mechanical_press',facing='east');machine([x+7,65,z+8],'cogwheel',axis='x');machine([x+7,66,z+8],'cogwheel',axis='x');machine([x+8,66,z+8],'mechanical_drill',facing='east');case['crank']=p;case['expected']=['Click crank: load exceeds capacity, so network reports Overstressed']
case,x,z=bay('Compacting basin')
basin=machine([x+8,65,z+8],'basin');p=machine([x+8,67,z+8],'mechanical_press',facing='north');drive(case,p);recipe=seed(basin,'compacting',None,1);case['targets']=[{'pos':p,'rpm':32}];case['recipe_id']=recipe['id'];case['expected']=['Compacting result in basin middle row']
case,x,z=bay('Tree saw')
p=machine([x+8,65,z+8],'mechanical_saw',facing='north');drive(case,p,back=(0,1,0));
for dy in range(4):put([x+8,65+dy,z+7],'minecraft:oak_log',axis='y')
for dx in range(-1,2):
 for dz in range(-1,1):
  if dx==0 and dz==0:continue
  put([x+8+dx,68,z+7+dz],'minecraft:oak_leaves',distance=1,persistent=True,waterlogged=False)
case['targets']=[{'pos':p,'rpm':32}];case['expected']=['Tree logs fall through ordinary engine drops; roots below cut remain']
case,x,z=bay('Depot deployer')
p=machine([x+10,67,z+8],'deployer',facing='down');inventory(p,{0:('minecraft:oak_planks',16)});drive(case,p,back=(1,0,0));depot=machine([x+10,65,z+8],'depot');inventory(depot,{0:('create:shaft',16)});case['targets']=[{'pos':p,'rpm':32}];case['application']={'deployer':p,'target':depot,'input':'create:shaft','count':16,'output':'create:cogwheel','hand_used':16};case['expected']=['16 shafts plus held planks become 16 cogwheels in depot output row']
case,x,z=bay('Belt deployer')
for dx in range(6,15):machine([x+dx,65,z+8],'belt',facing='east')
drive(case,[x+6,65,z+8]);p=machine([x+10,67,z+8],'deployer',facing='down');inventory(p,{0:('minecraft:oak_planks',32)});drive(case,p,back=(1,0,0))
chest([x+4,65,z+8],{0:('create:shaft',32)});f=machine([x+5,65,z+8],'brass_funnel',facing='east');state(f,extracting=True,transfer_limit=16);lever(case,[x+5,65,z+9],'north')
chest([x+16,65,z+8],{});f=machine([x+15,65,z+8],'brass_funnel',facing='west');state(f,item_filter='create:cogwheel');case['targets']=[{'pos':p,'rpm':32},{'pos':[x+14,65,z+8],'rpm':32}];case['application']={'deployer':p,'input':'create:shaft','count':32,'output':'create:cogwheel','hand_used':32};case['expected']=['Run both drives, release feed: 32 shafts become cogwheels in east chest']
case,x,z=bay('Reusable axe')
p=machine([x+8,67,z+8],'deployer',facing='down');inventory(p,{0:('minecraft:iron_axe',1,'Reusable test axe')});drive(case,p,back=(1,0,0));depot=machine([x+8,65,z+8],'depot');inventory(depot,{0:('minecraft:exposed_copper',8)});case['targets']=[{'pos':p,'rpm':32}];case['application']={'deployer':p,'target':depot,'input':'minecraft:exposed_copper','count':8,'output':'minecraft:copper_block','hand_used':0};case['expected']=['8 copper blocks appear in depot output; named axe remains unchanged']
case,x,z=bay('Casing application')
p=machine([x+8,67,z+8],'deployer',facing='down');inventory(p,{0:('create:andesite_alloy',8)});drive(case,p,back=(1,0,0));depot=machine([x+8,65,z+8],'depot');inventory(depot,{0:('minecraft:stripped_oak_log',8)});case['targets']=[{'pos':p,'rpm':32}];case['application']={'deployer':p,'target':depot,'input':'minecraft:stripped_oak_log','count':8,'output':'create:andesite_casing','hand_used':8};case['expected']=['8 stripped logs and held alloys become andesite casings in depot output']
case,x,z=bay('Precision assembly')
controllers=[]
for stage in range(15):
 p=machine([x+6+stage,67,z+8],'deployer',facing='down');inventory(p,{0:(["create:cogwheel","create:large_cogwheel","minecraft:iron_nugget"][stage%3],16)});controllers.append(p)
drive(case,controllers[-1],back=(1,0,0))
for dx in range(5,22):machine([x+dx,65,z+8],'belt',facing='east')
drive(case,[x+5,65,z+8]);source=chest([x+3,65,z+8],{0:('create:golden_sheet',16)});f=machine([x+4,65,z+8],'brass_funnel',facing='east');state(f,extracting=True,transfer_limit=1);lever(case,[x+4,65,z+9],'north')
chest([x+23,65,z+8],{});f=machine([x+22,65,z+8],'brass_funnel',facing='west');state(f,extracting=False,transfer_limit=16)
case['targets']=[{'pos':controllers[-1],'rpm':32},{'pos':[x+21,65,z+8],'rpm':32}];case['assembly']={'controllers':controllers,'input':source,'count':16};case['expected']=['Run both drives, then release feed. Fifteen deployers perform five cog/large-cog/nugget cycles; 16 final weighted outputs reach east chest. Progress travels with intermediate items.']
# Match signs to the controls actually supplied in the few drive-free bays.
for case in cases:
 x,_,z=case['origin'];panel=containers[key([x+3,65,z+2])]
 if case['title']=='Item drain':panel['lines']=[f"{case['id']} Item drain",'Button = 1 bucket','1000 mB per bucket','Read LAB_GUIDE.md']
 elif case['title']=='Shared tank':panel['lines']=[f"{case['id']} Shared tank",'16000 / 96000 mB','Use bucket on tank','Read LAB_GUIDE.md']
 elif case['kind']=='stress':panel['lines']=[f"{case['id']} Crank stress",'Click the crank','Expect overstress','Read LAB_GUIDE.md']
# The map covers 6 rows of machinery bays within the bounded lab arena.
for i,entry in enumerate(config['items']):
 p=[8+(i%16)*3,65,224+(i//16)*3];name=entry['id']
 if name in config['blocks']:
  props={}
  variants=config['blocks'][name]
  if 'north' in variants:props={'facing':'north'}
  elif 'x' in variants:props={'axis':'x'}
  machine(p,name,**props)
  if name=='create:creative_motor':state(p,motor_rpm=0)
# Tool/sample stores near spawn, three full registry chests and a vanilla kit.
for group in range(3):
 entries=config['items'][group*27:(group+1)*27];chest([8+group*3,65,12],{i:(v['id'],1 if 'bucket' in v['id'] or v['id'].endswith('wrench') or v['id']=='create:incomplete_precision_mechanism' else 16) for i,v in enumerate(entries)})
chest([18,65,12],{0:('create:wrench',1),1:('minecraft:diamond_pickaxe',1),2:('minecraft:bucket',16),3:('minecraft:water_bucket',1),4:('minecraft:lava_bucket',1),5:('minecraft:coal',64),6:('create:blaze_cake',16),7:('minecraft:glass_bottle',16),8:('minecraft:iron_ingot',64),9:('minecraft:andesite',64),10:('create:andesite_alloy',64),11:('minecraft:stone',64)})
sign([8,65,8],['CREATE RUST LAB','Development world','Not a complete port','Read LAB_GUIDE.md'])
sign([18,65,9],['Registry chests','Wrench and tools','All cases prefilled','Levers ON = paused'])
plan={'schema':1,'name':'Create Rust Automation Lab','width':192,'length':248,'spawn':[8,65,5],'blocks':list(blocks.values()),'containers':list(containers.values()),'machines':list(states.values()),'cases':cases,'registered_items':len(config['items']),'registered_blocks':list(config['blocks'])}
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(plan,indent=2)+'\n');print(f"Plan: {len(cases)} bays, {len(blocks)} authored blocks, {len(containers)} containers/signs, {len(states)} machine records")
