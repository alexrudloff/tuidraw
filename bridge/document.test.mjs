import {test} from 'node:test';
import assert from 'node:assert/strict';
import {applyDraft} from './document.mjs';
import {build} from './build.mjs';
import {generateContent} from './generate.mjs';

const theme={background:'#121212',surface:'#222222',foreground:'#ffffff',muted:'#aaaaaa',border:'#444444',primary:'#8844ff',focus:'#ffdd44',danger:'#ff4444',success:'#44ee88',warning:'#eeaa44'};
const empty={guidance:'Updated',theme:null,state:[],widgets:[],placements:[],updates:[],remove:[],stateUpdates:[],layoutChecks:[]};
const heading={id:'heading',description:'Heading',type:'Text',props:{text:'Counter',muted:false}};
const readout={id:'readout',description:'Live count',type:'Text',props:{text:{$state:'/count'},muted:false}};
const button={id:'increment',description:'Increment count',type:'Button',props:{label:'Add one',intent:'primary',disabled:false},on:{press:{action:'compute',params:{statePath:'/count',op:'add',args:[{$state:'/count'},1]}}}};
const first={...empty,theme,state:[{key:'count',value:0}],widgets:[heading,readout,button],placements:[{id:'heading',parent:null,index:0},{id:'readout',parent:null,index:1},{id:'increment',parent:null,index:2}]};
const env={OPENAI_API_KEY:'test'};
function model(answer,requests=[]) { return async (_url,options)=>{
  requests.push(JSON.parse(options.body));
  return {ok:true,json:async()=>({model:'test-model',choices:[{finish_reason:'stop',message:{content:JSON.stringify(answer)}}],usage:{prompt_tokens:10,completion_tokens:20,total_tokens:30}})};
}; }
const generate=(answer,requests)=>(request)=>generateContent(request,env,model(answer,requests));

test('measured gap failures repair the actual widths and cannot relax the requested checks',async()=>{
  const initial={root:'root',theme,state:{},elements:{
    root:{type:'Row',props:{gap:0},children:['sidebar','main']},
    sidebar:{type:'Column',props:{maxWidth:50},children:['list']},
    list:{type:'Text',props:{text:'Channels',maxWidth:24}},
    main:{type:'Text',props:{text:'Conversation'}},
  }};
  const check={kind:'gap',from:'list',to:'main',axis:'horizontal',min:0,max:1};
  const proposal={...empty,geometryChecks:[check],updates:[{id:'main',key:'text',value:'Chat'}]};
  const repair={...empty,geometryChecks:null,placements:null,remove:null,stateUpdates:null,layoutChecks:null,
    updates:[...proposal.updates,{id:'sidebar',key:'width',value:24}]};
  const measured=[];
  const measure=(spec,viewport)=>{
    measured.push(structuredClone(spec));
    const x=spec.elements.sidebar.props.width===24?24:50;
    return {viewport,nodes:{list:{parent:'sidebar',bounds:[0,0,24,2]},main:{parent:'root',bounds:[x,0,100-x,2]}}};
  };
  const requests=[];
  const response=await generateContent({prompt:'Close the gap to the actual channel list',direct:true,initialSpec:initial,viewport:[100,36]},env,
    (url,options)=>model(requests.length?repair:proposal,requests)(url,options),measure);
  assert.equal(response.generation.calls.length,2);
  assert.equal(response.spec.elements.main.props.text,'Chat');
  assert.equal(response.spec.elements.sidebar.props.width,24);
  assert.equal(measured.length,3);
  assert.equal(JSON.parse(requests[0].messages[1].content).currentLayout.nodes.main.bounds[0],50);
  assert.match(requests[1].messages.at(-1).content,/gap 26/);
  assert.equal(response.layout.nodes.main.bounds[0],24);
  assert.equal(response.geometryChecks.length,1);
  const attempts=[];
  await assert.rejects(()=>generateContent({prompt:'Close the gap',direct:true,initialSpec:initial},env,
    (url,options)=>model(attempts.length?{...repair,geometryChecks:[]}:proposal,attempts)(url,options),measure),/Preserve the original geometryChecks/);
  await assert.rejects(()=>generateContent({prompt:'Close the gap',direct:true,initialSpec:initial},env,model(proposal)),/require the native renderer/);
});

test('multiple feedback turns preserve IDs, layout, native empty event maps and live state',async()=>{
  const requests=[];
  const start=await build({engine:'llm',prompt:'A counter'},env,generate(first,requests),()=>{throw Error('Baseline must not call Jev')});
  const original=structuredClone(start.spec);
  start.spec.state.count=7;
  for(const node of Object.values(start.spec.elements)) node.on??={}; // Rust serialization
  const edited=await build({engine:'llm',prompt:'Call it Launch counter',initialSpec:start.spec,goal:'A counter',history:[{prompt:'A counter'}]},env,generate({...empty,updates:[{id:'heading',key:'text',value:'Launch counter'}]},requests));
  assert.deepEqual(Object.keys(edited.spec.elements),Object.keys(original.elements));
  assert.deepEqual(edited.spec.elements.root.children,original.elements.root.children);
  assert.equal(edited.spec.state.count,7);
  assert.deepEqual(edited.spec.elements.increment, start.spec.elements.increment);
  assert.equal(edited.spec.elements.heading.props.text,'Launch counter');
  assert.equal(start.spec.elements.heading.props.text,'Counter');
  const context=JSON.parse(requests[1].messages[1].content);
  assert.equal(context.originalGoal,'A counter'); assert.deepEqual(context.priorFeedback,[{prompt:'A counter'}]);
  const reset=await build({engine:'llm',prompt:'Set count to 3 and make it green',initialSpec:edited.spec},env,generate({...empty,theme:{...theme,primary:'#22aa66'},stateUpdates:[{key:'count',value:3}]}));
  assert.equal(reset.spec.state.count,3);
  assert.equal(reset.spec.theme.primary,'#22aa66');
  assert.deepEqual(reset.spec.elements,edited.spec.elements);
  assert.equal(reset.generation.calls.length,1);
});

test('move and remove subtrees atomically; reject malformed trees and unsafe keys',()=>{
  const initial=applyDraft({...first,state:{count:0}},null);
  const before=structuredClone(initial);
  const conditional=structuredClone(initial);
  conditional.elements.heading.visible={$state:'/count',gt:0};
  assert.deepEqual(applyDraft({...empty,state:{},widgets:[{...heading,props:{text:'Renamed',muted:false}}]},conditional).elements.heading.visible,conditional.elements.heading.visible);
  const grouped=applyDraft({...empty,state:{},widgets:[{id:'group',type:'Panel',props:{title:'Controls'}}],placements:[{id:'group',parent:null,index:1},{id:'readout',parent:'group',index:0},{id:'increment',parent:'group',index:1}]},initial);
  assert.deepEqual(grouped.elements.root.children,['heading','group']);
  assert.deepEqual(grouped.elements.group.children,['readout','increment']);
  const removed=applyDraft({...empty,state:{},remove:['group']},grouped);
  assert.deepEqual(Object.keys(removed.elements).sort(),['heading','root']);
  for(const invalid of [
    {placements:[{id:'group',parent:'group',index:0}]},
    {placements:[{id:'group',parent:'missing',index:0}]},
    {placements:[{id:'root',parent:'group',index:0}]},
    {placements:[{id:'readout',parent:'heading',index:0}]},
    {updates:[{id:'heading',key:'__proto__',value:'bad'}]},
    {widgets:[{id:'constructor',type:'Text',props:{text:'bad'}}]},
    {remove:['root']}, ...['__proto__','prototype','constructor'].map(key=>({stateUpdates:[{key,value:3}]})),
  ]) assert.throws(()=>applyDraft({...empty,state:{},...invalid},grouped));
  assert.deepEqual(initial,before);
});

test('Jev is the default and cannot alter an explicit operand binding',async()=>{
  let calls=0;
  const result=await build({prompt:'A counter'},env,generate(first),({questions})=>{calls++;return {answers:Object.fromEntries(Object.entries(questions).map(([k,q])=>[k,{choice:Object.keys(q.criteria)[0]}]))}});
  assert.equal(calls,1); assert.equal(result.design.selection.change,'apply'); assert.equal(result.engine,'jev'); assert.equal(result.grounding,null);
  await assert.rejects(()=>build({prompt:'',engine:'llm'},env,generate(first)),/prompt/);
  await assert.rejects(()=>build({prompt:'Counter',engine:'other'},env,generate(first)),/Engine/);
});

test('side-by-side intent rejects stacked columns and repairs the actual parent layout',async()=>{
  const initial={root:'root',state:{},theme,elements:{
    root:{type:'Column',props:{},children:['left','center','right']},
    left:{type:'Column',props:{},children:['stats']},
    center:{type:'Column',props:{},children:['picture']},
    right:{type:'Column',props:{},children:['commands']},
    stats:{type:'Text',props:{text:'Stats',muted:false}},
    picture:{type:'Text',props:{text:'Picture',muted:false}},
    commands:{type:'Text',props:{text:'Commands',muted:false}},
  }};
  const check={container:'root',axis:'horizontal',children:['left','center','right']};
  const wrong={...empty,guidance:'Now three columns',layoutChecks:[check]};
  assert.throws(()=>applyDraft({...wrong,state:{}},initial),/requested horizontal.*Column/);
  assert.throws(()=>applyDraft({...empty,state:{}},initial),/makes no change/);
  const repaired={...wrong,widgets:[{id:'root',description:'Side-by-side sections',type:'Row',props:{}}]};
  const requests=[];
  const generate=(request)=>generateContent(request,env,async(url,options)=>model(requests.length?repaired:wrong,requests)(url,options));
  const result=await build({engine:'llm',prompt:'Three columns: stats | picture | commands',initialSpec:initial},env,generate);
  assert.equal(result.generation.calls.length,2);
  assert.match(requests[1].messages.at(-1).content,/requested horizontal/);
  assert.equal(result.spec.elements.root.type,'Row');
  assert.deepEqual(result.spec.elements.root.children,check.children);
  assert.equal(initial.elements.root.type,'Column');
  const grid={...repaired,widgets:[{id:'root',type:'Grid',props:{columns:3}}],state:{}};
  assert.doesNotThrow(()=>applyDraft(grid,initial));
  assert.throws(()=>applyDraft({...grid,widgets:[{id:'root',type:'Grid',props:{columns:2}}]},initial),/requested horizontal/);
  assert.deepEqual(applyDraft({...grid,layoutChecks:[{...check,children:['center','left']}]},initial).elements.root.children,['center','left','right']);
});


test('flex sizing edits preserve content and actions and reject invalid/no-op weights', async()=>{
  const initial={root:'root',state:{message:'hello',draft:''},theme,elements:{
    root:{type:'Column',props:{},children:['body']},
    body:{type:'Column',props:{},children:['conversation','composer']},
    conversation:{type:'ScrollView',props:{title:'Conversation',text:{$state:'/message'},height:20}},
    composer:{type:'Input',props:{label:'Message',value:{$bindState:'/draft'}}},
  }};
  const answer={...empty,updates:[{id:'body',key:'grow',value:1},{id:'conversation',key:'grow',value:1},{id:'conversation',key:'height',value:4}],layoutChecks:[{container:'body',axis:'vertical',children:['conversation','composer']}]};
  const requests=[];
  const result=await build({engine:'llm',prompt:'move input to the bottom and fill remaining space with the conversation',initialSpec:initial},env,generate(answer,requests));
  assert.equal(result.spec.elements.body.props.grow,1);
  assert.equal(result.spec.elements.conversation.props.grow,1);
  assert.equal(result.spec.elements.conversation.props.height,4);
  assert.deepEqual(result.spec.state,initial.state);
  assert.deepEqual(result.spec.elements.composer,{...initial.elements.composer,children:[]});
  assert.deepEqual(result.spec.elements.conversation.props.text,{$state:'/message'});
  assert.match(requests[0].messages[0].content,/every intervening container/);
  for(const branch of requests[0].response_format.json_schema.schema.properties.widgets.items.anyOf) {
    assert.ok(branch.properties.props.required.includes('grow'));
  }
  for(const value of [-1,17,0.5,'fill']) {
    await assert.rejects(build({engine:'llm',prompt:'expand',initialSpec:initial},env,generate({...empty,updates:[{id:'body',key:'grow',value}]})),/validation/);
  }
  assert.throws(()=>applyDraft({...empty,state:{},updates:[{id:'composer',key:'grow',value:0}]},initial),/no change/);
});


test('adaptive presentation and multiple selections round-trip through model proposals',async()=>{
  const widgets=[
    {id:'body',description:'responsive body',type:'Row',props:{gap:2,collapseBelow:70}},
    {id:'choices',description:'search choices',type:'MultiSelect',props:{title:'Cargo',options:['Fuel','Food'],value:{$bindState:'/cargo'},searchable:true}},
    {id:'market',description:'prices',type:'Table',props:{title:'Market',headers:['Item','Price'],rows:[['Fuel','12']],columnStyles:[{width:0,align:'left',color:'foreground'},{width:8,align:'right',color:'success'}],border:'none',showHeader:true,padding:1,columnGap:2}},
    {id:'clear',description:'clear selection',type:'Button',props:{label:'Clear',intent:'neutral',disabled:false},on:{press:{action:'setState',params:{statePath:'/cargo',value:[]}}}},
  ];
  const requests=[];
  const answer={...empty,theme,state:[{key:'cargo',value:['Fuel']}],widgets,placements:[{id:'choices',parent:'body',index:0},{id:'market',parent:'body',index:1}],layoutChecks:[{container:'body',axis:'horizontal',children:['choices','market']}]};
  const result=await build({engine:'llm',prompt:'searchable cargo selection next to a compact market table; stack on narrow screens'},env,generate(answer,requests));
  assert.deepEqual(result.spec.state.cargo,['Fuel']);
  assert.equal(result.spec.elements.body.props.collapseBelow,70);
  assert.equal(result.spec.elements.market.props.columnStyles[1].align,'right');
  assert.deepEqual(result.spec.elements.clear.on.press.params.value,[]);
  const edit=await build({engine:'llm',prompt:'clear cargo',initialSpec:result.spec},env,generate({...empty,stateUpdates:[{key:'cargo',value:[]}]}));
  assert.deepEqual(edit.spec.state.cargo,[]);
  // The strict provider schema must require every declared property, including nested styles.
  function strict(schema) {
    if(!schema||typeof schema!=='object')return;
    if(schema.type==='object') {
      assert.equal(schema.additionalProperties,false);
      assert.deepEqual([...schema.required].sort(),Object.keys(schema.properties).sort());
    }
    for(const value of Object.values(schema)) if(Array.isArray(value))value.forEach(strict);else strict(value);
  }
  strict(requests[0].response_format.json_schema.schema);
  for(const change of [
    {id:'choices',key:'options',value:['Fuel','Fuel']},
    {id:'market',key:'columnStyles',value:[{width:0}]},
  ]) {
    const bad=structuredClone(widgets);bad.find(w=>w.id===change.id).props[change.key]=change.value;
    await assert.rejects(build({engine:'llm',prompt:'new',initialSpec:undefined},env,generate({...answer,widgets:bad})),/validation/);
  }
  assert.throws(()=>applyDraft({...empty,state:{},updates:[{id:'body',key:'widthPercent',value:100}]},result.spec),/no change/);
});


test('container child lists establish ownership and layout checks understand nested groups',()=>{
  const make=(id,type,children)=>({id,type,props:type==='Panel'?{title:id}:{},children});
  const initial={root:'root',state:{},theme,elements:{root:{type:'Column',props:{},children:['a','b']},a:{type:'Text',props:{text:'A'}},b:{type:'Text',props:{text:'B'}}}};
  const draft={...empty,state:{},widgets:[make('group','Panel',['a','b'])]};
  const result=applyDraft(draft,initial);
  assert.deepEqual(result.elements.root.children,['group']);
  assert.deepEqual(result.elements.group.children,['a','b']);
  assert.deepEqual(initial.elements.root.children,['a','b']);
  assert.throws(()=>applyDraft({...draft,layoutChecks:[{container:'group',axis:'vertical',children:['a','b']},{container:'group',axis:'vertical',children:['b','a']}]},initial),/ordered direct children/);
  assert.doesNotThrow(()=>applyDraft({...draft,layoutChecks:[{container:'root',axis:'vertical',children:['group','a','b']}]},initial));
  assert.throws(()=>applyDraft({...draft,layoutChecks:[{container:'root',axis:'horizontal',children:['a','b']}]},initial),/requested horizontal.*Panel/);
  assert.throws(()=>applyDraft({...draft,layoutChecks:[{container:'root',axis:'vertical',children:['a','missing']}]},initial),/not a descendant.*Actual parent/);
  assert.throws(()=>applyDraft({...draft,widgets:[...draft.widgets,make('other','Column',['a'])]},initial),/multiple declared parents/);
  assert.throws(()=>applyDraft({...draft,placements:[{id:'a',parent:'group',index:0}]},initial),/conflicts/);
  assert.throws(()=>applyDraft({...draft,widgets:[make('group','Panel',[])]},initial),/Empty containers: group/);
  assert.throws(()=>applyDraft({...draft,widgets:[make('group','Panel',['root'])]},initial),/invalid child root/);
  assert.throws(()=>applyDraft({...draft,widgets:[make('group','Panel',['a'])]},result),/Unreachable/);
  assert.throws(()=>applyDraft({...draft,widgets:[make('a','Text',['b'])]},initial),/not a container/);
  const changed=applyDraft({...empty,state:{},widgets:[{...make('group','Panel',null),props:{title:'Renamed'}}]},result);
  assert.deepEqual(changed.elements.group.children,['a','b']);
});

test('repair preserves every pending edit list and explicit membership while replacing one invalid widget',async()=>{
  const initial={root:'root',state:{count:0},theme,elements:{
    root:{type:'Column',props:{},children:['heading','metric','old']},
    heading:{type:'Text',props:{text:'Before',muted:false}},
    metric:{type:'Metric',props:{label:'Count',value:{$state:'/count'}}},
    old:{type:'Text',props:{text:'Remove me'}},
  }};
  const widgets=[
    {id:'group',description:'Grouped statistics',type:'Panel',props:{title:'Stats'},children:['metric','gauge']},
    {id:'gauge',description:'Value table',type:'Table',props:{title:'Value',headers:['Name','Value'],rows:[['Count']]}},
  ];
  const proposed={...empty,guidance:'Grouped and updated',widgets,updates:[{id:'heading',key:'text',value:'After'}],stateUpdates:[{key:'count',value:3}],remove:['old'],placements:[{id:'group',parent:null,index:1}],layoutChecks:[{container:'root',axis:'vertical',children:['heading','group']}]};
  const repair={...empty,guidance:'',placements:null,updates:null,stateUpdates:null,remove:null,layoutChecks:null,widgets:[{...widgets[1],props:{title:'Value',headers:['Name','Value'],rows:[['Count','50']]}}]};
  const requests=[];
  const result=await build({engine:'llm',prompt:'Group these and update the count',initialSpec:initial},env,request=>generateContent(request,env,async(url,options)=>model(requests.length?repair:proposed,requests)(url,options)));
  assert.equal(result.generation.calls.length,2);
  assert.equal(result.spec.elements.heading.props.text,'After');
  assert.equal(result.spec.state.count,3);
  assert.equal(result.spec.elements.old,undefined);
  assert.deepEqual(result.spec.elements.root.children,['heading','group']);
  assert.deepEqual(result.spec.elements.group.children,['metric','gauge']);
  assert.deepEqual(result.spec.elements.gauge.props.rows,[['Count','50']]);
  assert.match(requests[1].messages.at(-1).content,/null to PRESERVE/);
  assert.equal(result.summary,'Grouped and updated');
  // [] is an intentional cancellation, unlike null.
  let calls=0;
  const cancelled=await build({engine:'llm',prompt:'Group these',initialSpec:initial},env,request=>generateContent(request,env,async(url,options)=>model(calls++?{...repair,updates:[]}:proposed)(url,options)));
  assert.equal(cancelled.spec.elements.heading.props.text,'Before');
  assert.equal(cancelled.spec.state.count,3);
});
