import test from 'node:test';
import assert from 'node:assert/strict';
import {runAgent} from './agent.mjs';
import {chooseQuickEdit,editCandidates} from './quick-edit.mjs';
import {patchProposal,compactProposal} from './proposal.mjs';
const spec={root:'root',state:{},elements:{root:{type:'Row',props:{},children:['rail','main']},rail:{type:'Text',props:{text:'Channels',width:20}},main:{type:'Text',props:{text:'Chat',width:'fill'}}}};
const measure=async(s,viewport)=>({viewport,audits:[],nodes:{root:{bounds:[0,0,100,30],parent:null},rail:{bounds:[0,0,s.elements.rail.props.width==='fill'?50:s.elements.rail.props.width,1],parent:'root'},main:{bounds:[50,0,50,1],parent:'root'}}});
const env={LLM_BASE_URL:'http://localhost:8000/v1'};
test('Jev selects a prevalidated small edit without a content call, or abstains without changing state',async()=>{
  const before=structuredClone(spec);
  const result=await runAgent({prompt:'Widen the channel rail to 26 cells',initialSpec:spec},env,{measure,fetcher:()=>assert.fail('Unnecessary LLM call'),evaluate:async({questions})=>{
    const choice=Object.entries(questions.edit.criteria).find(([,v])=>v.includes('rail (Text'))[0];return {answers:{edit:{choice}},costUsd:0.001};
  }});
  assert.equal(result.generation.calls.length,0);assert.equal(result.decisions[0].kind,'edit');
  assert.equal(result.spec.elements.rail.props.width,26);assert.equal(result.changed,true);assert.deepEqual(spec,before);
  let calls=0;
  const fallback=await runAgent({prompt:'Widen the channel rail to 26 cells',initialSpec:spec},env,{measure,evaluate:async()=>({answers:{edit:{choice:'fallback'}}}),fetcher:async()=>{calls++;return Response.json({choices:[{message:{role:'assistant',content:'Clarify the layout.'},finish_reason:'stop'}]});}});
  assert.equal(calls,1);assert.equal(fallback.changed,false);assert.deepEqual(spec,before);
  const layout=await measure(spec,[100,30]);
  for(const prompt of ['How would you widen this?','Do not widen the rail','Make a game with a wide map','Make the rail 999 cells wide'])assert.equal(editCandidates(spec,prompt,layout).length,0);
  const invalid=await chooseQuickEdit({spec,prompt:'Widen the rail',layout,viewport:[100,30],measure,evaluate:async()=>({answers:{edit:{choice:'invented'}}}),signal:AbortSignal.timeout(1000)});
  assert.equal(invalid.draft,null);
});
test('direct values and proposal patches reject unsafe paths and preserve their sources',()=>{
  const raw={widgets:[{id:'a',type:'Text',props:{text:'A'}}],footer:{id:'b'},updates:[{id:'a',key:'text',value:{$state:'/title'}}]};
  const normalized=compactProposal(raw);
  assert.deepEqual(JSON.parse(normalized.updates[0].json),{$state:'/title'});assert.equal(raw.widgets[0].description,undefined);
  const fixed=patchProposal(raw,[{op:'remove',path:'/footer'},{op:'add',path:'/widgets/-',value:{id:'b'}},{op:'replace',path:'/widgets/0/props/text',value:'B'}]);
  assert.equal(fixed.widgets[0].props.text,'B');assert.equal(raw.widgets[0].props.text,'A');
  assert.equal(patchProposal(raw,[{op:'replace',path:'/widgets/0/props/width',value:26}]).widgets[0].props.width,26);assert.ok(raw.footer);
  for(const path of ['/__proto__/x','/widgets/99','/widgets/-1','/widgets/0/missing/text','/widgets/0/~2',''])assert.throws(()=>patchProposal(raw,[{op:'replace',path,value:'bad'}]));
});

test('Jev can finish a clipped final batch using only validated sizing repairs; requested widths stay enforced',async()=>{
  const {documentBlueprint,prepareDocument}=await import('./generate.mjs');
  const {applyDraft}=await import('./document.mjs');
  const draft={final:true,state:{status:'Ready'},widgets:[
    {id:'root',type:'Column',props:{},children:['panel','status']},
    {id:'panel',type:'Panel',props:{title:'Commands',width:20,padding:1},children:['action']},
    {id:'action',type:'Button',props:{label:'Travel to sector'},on:{press:{action:'setState',params:{statePath:'/status',value:'Travelled'}}}},
    {id:'status',type:'Text',props:{text:{$state:'/status'}}},
  ]};
  const measure=async(s,viewport)=>{const w=s.elements.panel.props.width;return {viewport,nodes:{root:{parent:null,bounds:[0,0,100,35],minimumControlWidth:32},panel:{parent:'root',bounds:[0,0,w,5],minimumControlWidth:32},action:{parent:'panel',bounds:[2,2,w-4,3],minimumControlWidth:28}},audits:w<32?[{id:'action',code:'control-fit',severity:'error',message:'Clipped',requiredSize:[28,3]}]:[]};};
  let calls=0,choices=0;
  const result=await runAgent({prompt:'Create a command panel'},env,{measure,fetcher:async()=>{calls++;assert.equal(calls,1);return Response.json({choices:[{message:{role:'assistant',tool_calls:[{id:'build',type:'function',function:{name:'edit_interface',arguments:JSON.stringify(draft)}}]},finish_reason:'tool_calls'}]});},evaluate:async({questions})=>{if(questions.edit)choices++;return {answers:Object.fromEntries(Object.entries(questions).map(([key,q])=>[key,{choice:key==='edit'?'edit_0':Object.keys(q.criteria)[0]}]))};}});
  assert.equal(result.spec.elements.panel.props.width,32);assert.equal(calls,1);assert.equal(choices,1);assert.equal(result.decisions[0].kind,'repair');
  const parsed=documentBlueprint.parse({guidance:'test',theme:null,state:{},widgets:[],placements:[],updates:[],remove:[],stateUpdates:[],layoutChecks:[],geometryChecks:[],...compactProposal(draft)});
  const narrow=applyDraft(parsed,null);prepareDocument(narrow);
  const noRepair=await chooseQuickEdit({spec:narrow,prompt:'Panel must be exactly 20 cells wide',layout:await measure(narrow,[100,35]),viewport:[100,35],measure,evaluate:()=>assert.fail('No legal repair exists'),signal:AbortSignal.timeout(1000),repair:true,geometryChecks:[{kind:'size',id:'panel',axis:'horizontal',min:20,max:20}]});
  assert.equal(noRepair,null);
});

test('Jev repair can stack a cramped button row while preserving actions and ordered children',async()=>{
  const spec={root:'root',state:{status:'Ready'},elements:{root:{type:'Column',props:{},children:['actions','status']},actions:{type:'Row',props:{gap:1},children:['scan','dock','trade']},status:{type:'Text',props:{text:{$state:'/status'}}},
    ...Object.fromEntries(['scan','dock','trade'].map(id=>[id,{type:'Button',props:{label:id+' sector'},on:{press:{action:'setState',params:{statePath:'/status',value:id}}}}]))}};
  const measure=async(s,viewport)=>({viewport,nodes:{root:{parent:null,bounds:[0,0,40,20],minimumControlWidth:60},actions:{parent:'root',bounds:[0,0,40,10],minimumControlWidth:60},...Object.fromEntries(['scan','dock','trade'].map(id=>[id,{parent:'actions',bounds:[0,0,s.elements.actions.type==='Row'?12:40,1],minimumControlWidth:18}]))},audits:s.elements.actions.type==='Row'?['scan','dock','trade'].map(id=>({id,code:'control-fit',severity:'error',requiredSize:[18,1],message:'Clipped'})):[]});
  const answer=await chooseQuickEdit({spec,prompt:'Create a command deck',layout:await measure(spec,[40,20]),viewport:[40,20],measure,signal:AbortSignal.timeout(1000),repair:true,evaluate:async({questions})=>({answers:{edit:{choice:Object.entries(questions.edit.criteria).find(([,v])=>v.includes('vertically'))[0]}}})});
  assert.equal(answer.draft.widgets[0].type,'Column');
  assert.deepEqual(answer.draft.widgets[0].children,['scan','dock','trade']);
  const {applyDraft}=await import('./document.mjs');
  const updated=applyDraft(answer.draft,spec);
  for(const id of ['scan','dock','trade'])assert.deepEqual(updated.elements[id].on,spec.elements[id].on);
  const pinned=await chooseQuickEdit({spec,prompt:'Keep actions side by side',layout:await measure(spec,[40,20]),viewport:[40,20],measure,signal:AbortSignal.timeout(1000),repair:true,layoutChecks:[{container:'actions',axis:'horizontal',children:['scan','dock','trade']}],evaluate:()=>assert.fail('The pinned row has no valid repair')});
  assert.equal(pinned,null);
});

test('Jev repair combines independent width and row fixes before asking for another model call',async()=>{
  const button=label=>({type:'Button',props:{label},on:{press:{action:'setState',params:{statePath:'/status',value:label}}}});
  const spec={root:'root',state:{status:'Ready'},elements:{root:{type:'Column',props:{},children:['nav','actions','status']},nav:{type:'Panel',props:{title:'Navigation',width:20},children:['warp']},warp:button('Warp sector'),actions:{type:'Row',props:{},children:['scan','dock']},scan:button('Scan sector'),dock:button('Dock station'),status:{type:'Text',props:{text:{$state:'/status'}}}}};
  const measure=async(s,viewport)=>{const narrow=s.elements.nav.props.width<24,sideBySide=s.elements.actions.type==='Row';return {viewport,nodes:{root:{parent:null,bounds:[0,0,40,20]},nav:{parent:'root',bounds:[0,0,s.elements.nav.props.width,4],minimumControlWidth:24},warp:{parent:'nav',bounds:[0,0,s.elements.nav.props.width-2,3],minimumControlWidth:24},actions:{parent:'root',bounds:[0,4,40,3]},scan:{parent:'actions',bounds:[0,4,sideBySide?19:40,3],minimumControlWidth:22},dock:{parent:'actions',bounds:[20,4,sideBySide?19:40,3],minimumControlWidth:22}},audits:[...(narrow?[{id:'warp',code:'control-fit',severity:'error'}]:[]),...(sideBySide?[{id:'scan',code:'control-fit',severity:'error'}]:[])]};};
  const answer=await chooseQuickEdit({spec,prompt:'Build a command screen',layout:await measure(spec,[40,20]),viewport:[40,20],measure,signal:AbortSignal.timeout(1000),repair:true,evaluate:async({questions})=>({answers:{edit:{choice:Object.entries(questions.edit.criteria).find(([,v])=>v.includes('vertically')&&v.includes('expanding'))[0]}}})});
  assert.equal(answer.draft.widgets[0].type,'Column');
  assert.deepEqual(answer.draft.updates,[{id:'nav',key:'width',value:24}]);
});
