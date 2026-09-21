import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {modelTurn,runAgent,normalizeIds} from './agent.mjs';
const initial=JSON.parse(readFileSync(new URL('../examples/primitives.json',import.meta.url)));
const env={LLM_BASE_URL:'http://localhost:8000/v1',LLM_MODEL:'fake',OPENAI_API_KEY:'must-not-leak'};
const tool=(id,name,args)=>({role:'assistant',content:null,tool_calls:[{id,type:'function',function:{name,arguments:JSON.stringify(args)}}]});
const say=content=>({role:'assistant',content});
function provider(messages,requests=[]) {
  let cursor=0;
  return async(url,options)=>{
    assert.equal(options.headers.Authorization,undefined);
    const request=JSON.parse(options.body);requests.push(request);
    assert.equal(request.stream,true);assert.equal(request.reasoning_effort,'low');
    const message=messages[cursor++];assert.ok(message,'unexpected model call');
    return Response.json({choices:[{message,finish_reason:message.tool_calls?'tool_calls':'stop'}]});
  };
}
const measure=async(spec,viewport)=>({viewport,nodes:{heading:{bounds:[0,0,30,1],inner:[0,0,30,1],parent:'root'}},audits:[]});
const edit=text=>({updates:[{id:'heading',key:'text',value:text}]});
const request={prompt:'Rename heading',initialSpec:initial,engine:'llm'};
test('stream parser joins split UTF-8 and tool args, rejects incomplete responses',async()=>{
  const frames=[{choices:[{delta:{content:'Hello 🎲'}}]},
    {choices:[{delta:{tool_calls:[{index:0,id:'a',function:{name:'edit_interface',arguments:'{"updates":'}}]}}]},
    {choices:[{delta:{tool_calls:[{index:0,function:{arguments:'[]}'}}]},finish_reason:'tool_calls'}]}, {usage:{total_tokens:12}}];
  const bytes=new TextEncoder().encode(frames.map(f=>'data: '+JSON.stringify(f)+'\r\n\r\n').join('')+'data: [DONE]\r\n\r\n');
  const events=[];
  const result=await modelTurn([],env,{emit:e=>events.push(e),fetcher:async()=>new Response(new ReadableStream({start(c){for(const b of bytes)c.enqueue(Uint8Array.of(b));c.close();}}))});
  assert.equal(result.message.content,'Hello 🎲');assert.deepEqual(JSON.parse(result.message.tool_calls[0].function.arguments),{updates:[]});
  assert.equal(result.metrics.usage.total_tokens,12);assert.equal(events[0].text,'Hello 🎲');
  for(const finish_reason of [null,'length']) await assert.rejects(modelTurn([],env,{fetcher:async()=>Response.json({choices:[{message:tool('a','edit_interface',edit('bad')),finish_reason}]})}),/incomplete/);
});
test('OpenAI agent defaults to Terra while preserving model overrides',async()=>{
  for(const [override,expected] of [[undefined,'gpt-5.6-terra'],['gpt-5.6-luna','gpt-5.6-luna']]){
    const config={OPENAI_API_KEY:'test-secret',...(override?{LLM_MODEL:override}:{})};
    await modelTurn([],config,{fetcher:async(url,options)=>{
      assert.equal(url.origin,'https://api.openai.com');
      const body=JSON.parse(options.body);
      assert.equal(body.model,expected);assert.equal(body.reasoning_effort,'none');
      return Response.json({choices:[{message:{content:'Ready'},finish_reason:'stop'}]});
    }});
  }
});
test('inspect, schema lookup, failed atomic edit and repaired batch produce one final draft',async()=>{
  const requests=[],events=[];
  const original=structuredClone(initial);
  const result=await runAgent(request,env,{measure,emit:e=>events.push(e),fetcher:provider([
    tool('1','inspect_interface',{ids:['heading']}),tool('2','lookup_widgets',{types:['Text']}),
    tool('3','edit_interface',{updates:[...edit('should roll back').updates,{id:'heading',key:'unsupportedProp',value:'bad'}]}),
    tool('4','edit_interface',{widgets:[{id:'heading',description:'Correct the heading',type:'Text',props:{...initial.elements.heading.props,text:'Repaired'}}]}),say('Renamed the heading.')
  ],requests)});
  assert.equal(result.changed,true);assert.equal(result.spec.elements.heading.props.text,'Repaired');assert.deepEqual(initial,original);
  assert.ok(JSON.parse(requests[3].messages.at(-1).content).error);
  assert.equal(result.messages.filter(m=>m.role==='tool').length,4);
  assert.equal(events.filter(e=>e.type==='tool_end'&&e.ok===false).length,1);
  assert.equal(events.find(e=>e.type==='tool_end'&&!e.ok).message,'Checking and correcting the proposed changes…');
});
test('conversation-only reply has no edit, native measurement or Jev round trip on an empty project',async()=>{
  const previous=[[{role:'user',content:'Hello'},{role:'assistant',content:'Hi!'}]],requests=[];
  const result=await runAgent({prompt:'What can you build?',conversation:previous},env,{fetcher:provider([say('Terminal interfaces.')],requests),evaluate:()=>assert.fail('Unexpected Jev call')});
  assert.equal(result.changed,false);assert.equal(result.spec,null);assert.deepEqual(requests[0].messages.slice(1,3),previous[0]);
});
test('failed geometry requirements survive repairs, rejected batches do not publish',async()=>{
  const strict={kind:'size',id:'heading',axis:'horizontal',min:50,max:50};
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider([
    tool('1','edit_interface',{...edit('bad width'),geometryChecks:[strict]}),
    tool('2','edit_interface',edit('cannot discard check')),say('Done')
  ])}),/still fails validation/);
});
test('tool failures and turn count are bounded, duplicate ids and aborts cannot publish',async()=>{
  const bad={updates:[{id:'missing',key:'text',value:'oops'}]};
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider([1,2,3,4,5,6].map(i=>tool(String(i),'edit_interface',bad)))}),/six-turn/);
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider([tool('same','edit_interface',edit('draft')),tool('same','edit_interface',edit('again'))])}),/repeated tool-call/);
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider(Array.from({length:6},(_,i)=>tool(String(i),'inspect_interface',{})))}),/six-turn/);
  await assert.rejects(runAgent(request,env,{signal:AbortSignal.abort(),measure,fetcher:()=>assert.fail('request after abort')}),/abort/i);
});
test('a contradictory repair cannot poison the original layout requirements',async()=>{
  const initialSpec={root:'root',state:{},elements:{root:{type:'Column',props:{},children:['a','b']},a:{type:'Text',props:{text:'A'}},b:{type:'Text',props:{text:'B'}}}};
  const draft=(type,axis)=>({widgets:[{id:'root',description:'Arrange items',type,props:{},children:['a','b']}],layoutChecks:[{container:'root',axis,children:['a','b']}],geometryChecks:[{kind:'size',id:'a',axis:'horizontal',min:50,max:50}]});
  const measure=async(spec,viewport)=>({viewport,audits:[],nodes:{a:{bounds:[0,0,spec.elements.a.props.width??30,1]}}});
  const result=await runAgent({...request,initialSpec},env,{measure,fetcher:provider([
    tool('1','edit_interface',draft('Row','horizontal')),
    tool('2','edit_interface',draft('Column','vertical')),
    tool('3','edit_interface',{...draft('Row','horizontal'),updates:[{id:'a',key:'width',value:50}]}),say('Placed side by side.')
  ])});
  assert.equal(result.changed,true);assert.equal(result.spec.elements.root.type,'Row');
});

test('invalid new IDs are compiled consistently without a model repair or changes to labels/state',async()=>{
  const aliases=new Map();
  const raw={widgets:[{id:'7',type:'Text',description:'Number key',props:{text:'7'}},{id:'Key pad',type:'Column',description:'Group',props:{},children:['7']}],
    placements:[{id:'7',parent:'Key pad',index:0}],updates:[{id:'7',key:'text',value:'7'}],remove:['7'],
    layoutChecks:[{container:'Key pad',axis:'vertical',children:['7','heading']}],
    geometryChecks:[{kind:'gap',from:'7',to:'heading',axis:'vertical',min:0,max:1},{kind:'edge',id:'7',parent:'Key pad',edge:'left',inset:0}],state:{label:'Key pad'}};
  const fixed=normalizeIds(raw,initial,aliases),child=aliases.get('7'),parent=aliases.get('Key pad');
  assert.match(child,/^[a-z][a-z0-9_-]{0,63}$/);
  assert.equal(fixed.widgets[1].children[0],child);assert.equal(fixed.widgets[0].props.text,'7');
  assert.equal(fixed.placements[0].parent,parent);assert.equal(fixed.updates[0].id,child);assert.equal(fixed.remove[0],child);
  assert.equal(fixed.layoutChecks[0].container,parent);assert.equal(fixed.layoutChecks[0].children[0],child);
  assert.equal(fixed.geometryChecks[0].from,child);assert.equal(fixed.geometryChecks[1].parent,parent);assert.equal(fixed.geometryChecks[1].id,child);
  assert.equal(fixed.geometryChecks[0].to,'heading');assert.deepEqual(fixed.state,raw.state);assert.equal(raw.widgets[0].id,'7');
  assert.equal(normalizeIds({widgets:[raw.widgets[0]]},{elements:{[child]:{}}}).widgets[0].id,child+'_2');
  assert.throws(()=>normalizeIds({widgets:[raw.widgets[0],raw.widgets[0]]},initial),/unique/);
  assert.equal(normalizeIds({widgets:[{...raw.widgets[1],children:['Missing!']}]},initial).widgets[0].children[0],'Missing!');
  const events=[],requests=[];
  const result=await runAgent({...request,prompt:'Add a number label'},env,{measure,emit:e=>events.push(e),fetcher:provider([
    tool('add','edit_interface',{widgets:raw.widgets}),say('Added the label.')
  ],requests)});
  assert.equal(requests.length,2);assert.equal(result.generation.calls.length,2);
  assert.equal(events.filter(e=>e.type==='tool_end'&&!e.ok).length,0);
  assert.equal(result.spec.elements[parent].children[0],child);
  assert.equal(JSON.parse(result.messages.find(m=>m.role==='tool').content).idMap['7'],child);
  const again=normalizeIds({updates:[{id:'7',key:'text',value:'eight'}]},result.spec,aliases);
  assert.equal(again.updates[0].id,child);
});

test('schema diagnostics reach the repair model without leaking into chat progress',async()=>{
  const events=[],requests=[];
  await runAgent(request,env,{measure,emit:e=>events.push(e),fetcher:provider([
    tool('bad','edit_interface',{widgets:[{id:'group',description:'Group',type:'Column',props:{},children:['Unknown!']}]}),
    tool('fixed','edit_interface',edit('Fixed')),say('Updated.')
  ],requests)});
  const diagnostic=JSON.parse(requests[1].messages.at(-1).content).error;
  assert.match(diagnostic,/invalid_format/);assert.match(diagnostic,/children/);
  const status=events.find(e=>e.type==='tool_end'&&!e.ok).message;
  assert.equal(status,'Checking and correcting the proposed changes…');
  assert.doesNotMatch(status,/regex|invalid_format|\{|\[/);
});

test('private invalid drafts accept small repairs while retaining every other proposed widget',async()=>{
  const events=[],requests=[];
  const newWidgets=Array.from({length:31},(_,i)=>({id:'new_'+i,description:'New content',type:'Text',props:{text:'Content '+i}}));
  const measure=async(spec,viewport)=>({viewport,nodes:{new_0:{bounds:[0,0,20,1],inner:[0,0,20,1]}},audits:spec.elements.new_0?.props.width===10?[{id:'new_0',severity:'error',message:'Too narrow'}]:[]});
  const result=await runAgent(request,env,{measure,emit:e=>events.push(e),fetcher:provider([
    tool('1','lookup_widgets',{types:['Column','Row','Panel','Text','Metric','Button','Grid','Table','List','Input','Badge','Scene','AnsiArt','ScrollView']}),
    tool('2','edit_interface',{widgets:newWidgets,updates:[{id:'new_0',key:'width',value:10}]}),
    tool('3','inspect_interface',{ids:['new_0']}),
    tool('4','edit_interface',{updates:[{id:'new_0',key:'width',value:30}]}),say('Created the interface.')
  ],requests)});
  assert.equal(events.filter(e=>e.type==='tool_end'&&!e.ok).length,1);
  const failed=JSON.parse(requests[2].messages.at(-1).content);
  assert.equal(failed.pendingValidation,true);assert.equal(failed.rejectedDraftLayout.nodes.new_0.sizing.width,10);
  const inspected=JSON.parse(requests[3].messages.at(-1).content);
  assert.equal(inspected.pendingValidation,true);assert.equal(inspected.spec.elements.new_0.props.width,10);
  for(const widget of newWidgets)assert.ok(result.spec.elements[widget.id]);
  assert.equal(result.spec.elements.new_0.props.width,30);assert.equal(initial.elements.new_0,undefined);
});
test('new-screen checks can be corrected using measured ancestor width without relaxing refinement checks',async()=>{
  const events=[],requests=[];
  const theme=Object.fromEntries(['background','surface','foreground','muted','border','primary','focus','danger','success','warning'].map(k=>[k,'#112233']));
  const widgets=[{id:'root',type:'Column',props:{},children:['sidebar','status']},
    {id:'sidebar',type:'Panel',props:{title:'Controls',width:24,padding:1},children:['actions']},
    {id:'actions',type:'Row',props:{gap:1},children:['warp','scan']},
    ...['warp','scan'].map((id,i)=>({id,type:'Button',props:{label:id,hotkey:i?'Alt+S':'Alt+W'},on:{press:{action:'setState',params:{statePath:'/status',value:id}}}})),
    {id:'status',type:'Text',props:{text:{$state:'/status'}}}].map(w=>({...w,description:w.id}));
  const check=max=>[{kind:'size',id:'sidebar',axis:'horizontal',min:20,max}];
  const measure=async(spec,viewport)=>{
    const width=spec.elements.sidebar.props.width,w=Math.floor((width-5)/2);
    return {viewport,nodes:{sidebar:{parent:'root',bounds:[0,0,width,5],minimumControlWidth:37},actions:{parent:'sidebar',bounds:[2,2,width-4,3]},warp:{parent:'actions',bounds:[2,2,w,3]}},audits:w<16?[{id:'warp',code:'control-fit',severity:'error',requiredSize:[16,3],message:'Control is clipped'}]:[]};
  };
  const result=await runAgent({prompt:'Build a command panel',engine:'llm'},env,{measure,emit:e=>events.push(e),fetcher:provider([
    tool('1','edit_interface',{theme,state:{status:''},widgets,geometryChecks:check(30)}),
    tool('2','edit_interface',{updates:[{id:'sidebar',key:'width',value:30}],geometryChecks:check(30)}),
    tool('3','edit_interface',{updates:[{id:'sidebar',key:'width',value:37}],geometryChecks:check(36)}),
    tool('4','edit_interface',{geometryChecks:check(37)}),
    tool('5','edit_interface',{updates:[{id:'sidebar',key:'width',value:40}],geometryChecks:check(40)}),say('Built the panel.')
  ],requests)});
  const failure=JSON.parse(requests[1].messages.at(-1).content);
  assert.equal(failure.checksPinned,false);assert.equal(failure.rejectedDraftLayout.audits[0].repairHint.id,'sidebar');
  assert.equal(failure.rejectedDraftLayout.audits[0].repairHint.minimum,37);
  assert.equal(result.spec.elements.sidebar.props.width,40);
  assert.equal(events.filter(e=>e.type==='tool_end'&&!e.ok).length,3);
  assert.equal(requests.length,6);
});

test('all invalid widgets are reported together and repaired in one batch',async()=>{
  const requests=[];
  const widgets=['location','target','log'].map(id=>({id,type:'Text',description:id,props:{text:'Current: {$state:/'+id+'}'}}));
  const result=await runAgent(request,env,{measure,fetcher:provider([
    tool('1','edit_interface',{state:{location:'Sol',target:'Vega',log:'Ready'},widgets}),
    tool('2','edit_interface',{widgets:widgets.map(w=>({...w,props:{text:{$state:'/'+w.id}}}))}),say('Built all three displays.')
  ],requests)});
  const failure=JSON.parse(requests[1].messages.at(-1).content).error;
  for(const widget of widgets)assert.match(failure,new RegExp('Widget generated-'+widget.id));
  assert.equal(requests.length,3);assert.equal(result.spec.elements.target.props.text.$state,'/target');
});

test('array binding repairs retain typed diagnostics, checks and permit explicit new state',async()=>{
  const requests=[];
  const widget={id:'cargo_status',description:'Cargo',type:'Text',props:{text:{$state:'/cargo'}}};
  const newScreen={prompt:'Build a cargo manifest',engine:'llm'};
  const theme=Object.fromEntries(['background','surface','foreground','muted','border','primary','focus','danger','success','warning'].map(k=>[k,'#112233']));
  const geometryChecks=[{kind:'size',id:'cargo_status',axis:'horizontal',min:1,max:100}];
  const result=await runAgent(newScreen,env,{measure:async(spec,viewport)=>({viewport,nodes:{cargo_status:{bounds:[0,0,30,1]}},audits:[]}),fetcher:provider([
    tool('1','edit_interface',{theme,state:{cargo:['Food','Ore'],credits:12500},widgets:[widget],geometryChecks}),
    tool('2','edit_interface',{widgets:[widget]}),
    tool('3','edit_interface',{state:{credits:0},stateUpdates:[{key:'cargo_summary',value:'Food · Ore'}],widgets:[{...widget,props:{text:{$state:'/cargo_summary'}}}]}),say('Built the manifest.')
  ],requests)});
  for(const index of [1,2]) {
    const failure=JSON.parse(requests[index].messages.at(-1).content);
    assert.match(failure.error,/Text.text reads \/cargo, which resolves to array/);
    assert.match(failure.error,/List.items/);
    assert.doesNotMatch(failure.error,/makes no change/);
    assert.equal(failure.requiredChecks.geometry[0].id,'cargo_status');
  }
  assert.equal(result.spec.state.cargo_summary,'Food · Ore');
  assert.deepEqual(result.spec.state.cargo,['Food','Ore']);
  assert.equal(result.spec.state.credits,12500);
  assert.equal(result.spec.elements.cargo_status.props.text.$state,'/cargo_summary');
  assert.equal(result.geometryChecks[0].id,'cargo_status');
});

test('a final-turn validated edit completes without a seventh model call; pending failures still roll back',async()=>{
  const requests=[];
  const result=await runAgent(request,env,{measure,fetcher:provider([
    ...Array.from({length:5},(_,i)=>tool(String(i),'inspect_interface',{})),
    tool('last','edit_interface',edit('Saved on turn six'))
  ],requests)});
  assert.equal(requests.length,6);
  assert.equal(result.type,'complete');assert.equal(result.stopReason,'finish');
  assert.equal(result.limitReached,'turns');assert.equal(result.changed,true);
  assert.equal(result.spec.elements.heading.props.text,'Saved on turn six');
  assert.equal(result.messages.at(-1).content,result.summary);
  assert.match(result.summary,/turn limit/);
  assert.equal(result.messages.filter(m=>m.role==='tool').length,6);
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider([
    tool('first','edit_interface',edit('Valid intermediate draft')),
    ...Array.from({length:4},(_,i)=>tool(String(i),'inspect_interface',{})),
    tool('last','edit_interface',{updates:[{id:'heading',key:'unsupportedProp',value:'bad'}]})
  ])}),/six-turn limit/);
  assert.notEqual(initial.elements.heading.props.text,'Saved on turn six');
  assert.notEqual(initial.elements.heading.props.text,'Valid intermediate draft');
});

test('direct JSON edits and final batches validate and finish in one model call',async()=>{
  const requests=[];
  const result=await runAgent(request,env,{measure,fetcher:provider([
    tool('final','edit_interface',{final:true,guidance:'Bound the heading.',state:{title:'Live title'},updates:[{id:'heading',key:'text',value:{$state:'/title'}}]})
  ],requests)});
  assert.equal(requests.length,1);assert.equal(result.generation.calls.length,1);
  assert.equal(result.spec.elements.heading.props.text.$state,'/title');
  assert.match(requests[0].messages[0].content,/Core widget contracts already loaded/);
  assert.equal(result.summary,'Bound the heading.');
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider([
    tool('bad','edit_interface',{final:true,updates:[{id:'heading',key:'text',value:{$state:'/absent'}}]}),say('Done')
  ])}),/still fails validation/);
});

test('schema-invalid proposals can be surgically repaired without repeating widgets or duplicating moves',async()=>{
  const requests=[];
  const added={id:'new_label',type:'Text',props:{text:'Kept'}};
  const result=await runAgent(request,env,{measure,fetcher:provider([
    tool('bad','edit_interface',{final:true,widgets:[added],footer:{id:'footer',type:'Text',props:{text:'Footer'}}}),
    tool('read','inspect_interface',{proposal:true}),
    tool('repair','repair_proposal',{patches:[{op:'remove',path:'/footer'},{op:'add',path:'/widgets/-',value:{id:'footer',type:'Text',props:{text:'Footer'}}}]})
  ],requests)});
  assert.equal(requests.length,3);assert.equal(result.changed,true);
  const failure=JSON.parse(requests[1].messages.at(-1).content);
  assert.equal(failure.proposalRetained,true);assert.equal(failure.pendingValidation,false);
  assert.equal(JSON.parse(requests[2].messages.at(-1).content).proposal.widgets[0].id,'new_label');
  assert.equal(result.spec.elements.new_label.props.text,'Kept');assert.equal(result.spec.elements.footer.props.text,'Footer');
  assert.equal(result.spec.elements.root.children.filter(id=>id==='new_label').length,1);
});


test('known misplaced widget props are normalized before tool validation',async()=>{
  const result=await runAgent(request,env,{measure,fetcher:provider([
    tool('build','edit_interface',{final:true,widgets:[{id:'heading',type:'Text',props:{text:'Expanded'},grow:1}]})
  ])});
  assert.equal(result.spec.elements.heading.props.grow,1);
  assert.equal(result.spec.elements.heading.props.text,'Expanded');
  assert.equal(result.generation.calls.length,1);
  await assert.rejects(runAgent(request,env,{measure,fetcher:provider([
    tool('bad','edit_interface',{final:true,widgets:[{id:'heading',type:'Text',props:{text:'No'},nonsense:1}]}),say('done')
  ])}),/still fails validation/);
});
