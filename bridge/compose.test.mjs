import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { compose, demoEvaluator, evaluator, loadConfig } from './compose.mjs';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { candidatesFor, initialState } from './catalog.mjs';
import { generateContent, prepareContent } from './generate.mjs';
import { extendedComponents, extendedSamples } from './extended.mjs';
import { groundOperations } from './ground.mjs';
import { checkActions } from './actions.mjs';

const prepared = async request => ({ candidates: candidatesFor(request.prompt), state: request.initialSpec?.state ?? initialState });
const palette = {background:'#f3eee3',surface:'#dfd6c4',foreground:'#29251d',muted:'#645b49',border:'#8a7a5c',primary:'#40683e',focus:'#b64c2d',danger:'#a32d3b',success:'#366d42',warning:'#896009'};

test('palettes validate, survive refinement, and can be explicitly replaced', async () => {
  const original={root:'heading',elements:{heading:{type:'Text',props:{text:'Hello'},children:[]}},state:{},theme:palette};
  const draft={theme:null,state:{},widgets:[]};
  const content=prepareContent(draft,original);
  assert.deepEqual(content.theme,palette);
  assert.deepEqual(prepareContent({...draft,theme:{...palette,primary:'#771133'}},original).theme.primary,'#771133');
  for(const primary of ['red','#123','#gg1133','\u001b[31m']) {
    assert.throws(()=>prepareContent({...draft,theme:{...palette,primary}},original));
  }
  let final;
  for await(const event of compose({prompt:'Keep this interface',initialSpec:original},async()=>({answers:{next:{choice:'finish'}}}),false,async()=>content)) final=event;
  assert.deepEqual(final.spec.theme,palette);
  assert.deepEqual(original.theme,palette);
});

test('invalid append bindings identify the state type and repair only the broken content', async () => {
  const binding = {action:'compute',params:{statePath:'/expr',op:'append',args:[{$state:'/expr'},'(']}};
  assert.throws(()=>checkActions(binding,{expr:false}), /append argument 1 \(\/expr\).*got boolean/);
  assert.doesNotThrow(()=>checkActions(binding,{expr:''}));
  const draft = {theme:palette,guidance:'Display above buttons',state:{expr:false},widgets:[
    {id:'display',type:'Text',description:'Expression',props:{text:{$state:'/expr'},muted:false}},
    {id:'open',type:'Button',description:'Append open parenthesis',props:{label:'(',intent:'neutral',disabled:false},on:{press:binding}},
  ]};
  for (const openai of [false,true]) {
    let calls = 0;
    const repaired = await generateContent({prompt:'a calculator'},openai ? {OPENAI_API_KEY:'test-secret'} : {}, async (_url,options)=>{
      const body=JSON.parse(options.body);
      if(calls++) {
        assert.match(body.messages.at(-1).content,/append argument 1.*\/expr.*boolean/);
        assert.match(body.messages.at(-1).content,/ONLY corrected state entries/);
      }
      const response = calls===1 ? structuredClone(draft) : {theme:null,guidance:'',state:{expr:''},widgets:[]};
      if(openai) response.state=Object.entries(response.state).map(([key,value])=>({key,value}));
      return Response.json({choices:[{message:{content:JSON.stringify(response)}}]});
    });
    assert.equal(calls,2);
    assert.equal(repaired.state.expr,'');
    assert.equal(repaired.guidance,draft.guidance);
    assert.deepEqual(repaired.theme,palette);
    assert.deepEqual(repaired.candidates.at(-1).element.on.press,binding);
    assert.equal(repaired.candidates.length,3);
  }
  const noOp=structuredClone(draft); noOp.state.expr='';
  noOp.widgets[1].on.press={action:'setState',params:{statePath:'/expr',value:{$state:'/expr'}}};
  assert.throws(()=>prepareContent(noOp),/assigns state to itself/);
});

test('Jev connects a proposed constant operand to the actual adjustable input', async () => {
  const content=prepareContent({state:{count:'2',result:'0'},widgets:[
    {id:'count',description:'Adjustable sample count',type:'Input',props:{label:'Count',value:{$bindState:'/count'}}},
    {id:'result',description:'Result',type:'Text',props:{text:{$state:'/result'},muted:false}},
    {id:'roll',description:'Sample according to count',type:'Button',props:{label:'Sample'},on:{press:{action:'compute',params:{statePath:'/result',op:'randomInt',args:[1,6,1]}}}},
  ]});
  const connected=await groundOperations(content,async ({questions})=>({answers:Object.fromEntries(Object.entries(questions).map(([key,q])=>[key,{choice:Object.keys(q.criteria).find(key=>q.criteria[key].includes('samples to sum = Input')) ?? 'keep'}]))}),'Sample with adjustable count',AbortSignal.timeout(1000));
  assert.equal(content.candidates.at(-1).element.on.press.params.args[2],1);
  assert.deepEqual(connected.candidates.at(-1).element.on.press.params.args[2],{$state:'/count'});
  await assert.rejects(groundOperations(content,async()=>({answers:{}}),'Sample',AbortSignal.timeout(1000)),/invalid operation operand/);
});

test('expanded catalog validates every recipe and rejects inconsistent charts and controls', () => {
  assert.equal(Object.keys(extendedComponents).length,14);
  for (const sample of extendedSamples) {
    extendedComponents[sample.type].props.parse(sample.props);
    const widget={id:'sample',description:sample.type,...structuredClone(sample)};
    const fields={Popup:'open',Select:'value',MultiSelect:'value',Tabs:'value',Slider:'value'};
    const state={};
    if(fields[sample.type]) {const field=fields[sample.type];state.selected=widget.props[field];widget.props[field]={$bindState:'/selected'};}
    assert.doesNotThrow(()=>prepareContent({state,widgets:[widget]}));
  }
  assert.throws(()=>prepareContent({state:{},widgets:[{id:'bad',description:'Bad chart',type:'BarChart',props:{title:'Bad',labels:['A'],values:[1,2]}}]}),/equal lengths/);
  assert.throws(()=>prepareContent({state:{selected:'X'},widgets:[{id:'bad',description:'Bad selection',type:'Select',props:{title:'Choose',options:['A','B'],value:{$bindState:'/selected'}}}]}),/Selection/);
  assert.throws(()=>extendedComponents.BigText.props.parse({text:'你好',font:'pixel'}));
  assert.throws(()=>extendedComponents.PlayingCards.props.parse({title:'Hand',cards:[{rank:'A',suit:'spades',unexpected:true}]}));
  assert.throws(()=>prepareContent({state:{result:'0'},widgets:[{id:'bad',description:'Result',type:'Text',props:{text:'Result: {$state:"/result"}'}}]}),/placeholders/);
});

test('missing bound-control state gets native defaults and refinement preserves existing values', () => {
  const widgets=[
    {id:'travel',description:'Travel mode',type:'Select',props:{title:'Travel mode',options:['Warp','Trade lane'],value:{$bindState:'/jumpStyle'}}},
    {id:'tab',description:'Views',type:'Tabs',props:{title:'View',tabs:[{label:'Map',text:'Sector map'}],value:{$bindState:'/view'}}},
    {id:'toggle',description:'Sound',type:'Switch',props:{label:'Sound',disabled:false,checked:{$bindState:'/sound'}}},
    {id:'slider',description:'Power',type:'Slider',props:{label:'Power',min:10,max:100,step:1,value:{$bindState:'/power'}}},
    {id:'input',description:'Name',type:'Input',props:{label:'Name',value:{$bindState:'/name'}}},
    {id:'popup',description:'Help',type:'Popup',props:{title:'Help',label:'Help',body:'Help text',open:{$bindState:'/help'}}},
  ];
  const draft={state:{},widgets};
  assert.deepEqual(prepareContent(draft).state,{jumpStyle:'Warp',view:'Map',sound:false,power:10,name:'',help:false});
  assert.deepEqual(prepareContent({...draft,state:{name:2,power:'20',sound:'false'}}).state,{name:'2',power:20,sound:false,jumpStyle:'Warp',view:'Map',help:false});
  assert.equal(prepareContent(draft,{state:{jumpStyle:'Trade lane'},elements:{}}).state.jumpStyle,'Trade lane');
  assert.doesNotThrow(()=>prepareContent({...draft,widgets:[...widgets,{id:'boost',description:'Increase power',type:'Button',props:{label:'Boost'},on:{press:{action:'compute',params:{statePath:'/power',op:'add',args:[{$state:'/power'},1]}}}}]}));
  assert.throws(()=>prepareContent({...draft,state:{jumpStyle:'Bogus'}}),/Selection/);
});

test('Jev assembles generic controls and preserves executable operation sequences', async () => {
  const draft = { theme: palette, guidance: 'Display first; Roll and Clear belong in the two-column controls grid.', state: { result: 'Ready' }, widgets: [
    {id:'result',type:'Text',description:'Result display above controls',props:{text:{$state:'/result'},muted:false}},
    {id:'controls',type:'Grid',description:'Two-column container for Roll and Clear',props:{columns:2}},
    {id:'roll',type:'Button',description:'First button in controls grid',props:{label:'Roll',intent:'primary',disabled:false},on:{press:[
      {action:'compute',params:{statePath:'/result',op:'randomInt',args:[1,20,1]}},
      {action:'compute',params:{statePath:'/result',op:'add',args:[{$state:'/result'},3]}},
    ]}},
    {id:'clear',type:'Button',description:'Second button in controls grid',props:{label:'Clear',intent:'neutral',disabled:false},on:{press:{action:'setState',params:{statePath:'/result',value:'Ready'}}}},
  ] };
  const content=prepareContent(draft);
  assert.match(content.candidates.find(c=>c.id==='generated-roll').description,/randomInt.*add/);
  let sawGuidance=false, preservedOrder=false;
  const evaluate=async ({state,questions})=>{
    sawGuidance ||= state.context?.guidance===draft.guidance;
    if (state.selected_elements) {
      assert.ok(!Object.keys(questions).some(key=>key.startsWith('order_')));
      preservedOrder=true;
    }
    return {answers:Object.fromEntries(Object.entries(questions).map(([key,q])=>{
      let choice;
      if(key==='root') choice='layout-column';
      else if(key.startsWith('select_')) choice=Object.keys(q.criteria).find(k=>k.startsWith('use:generated-')) ?? (Object.hasOwn(q.criteria,'omit') ? 'omit' : '0');
      else if(key.startsWith('parent_') && q.instructions.includes('[Button;')) choice=Object.keys(q.criteria).find(k=>q.criteria[k].includes('[Grid]'));
      else choice=Object.keys(q.criteria)[0];
      return [key,{choice}];
    }))};
  };
  let final;
  for await(const event of compose({prompt:'Build a random sampling interface'},evaluate,false,async()=>content)) {
    if (event.spec) assert.deepEqual(event.spec.theme,palette);
    final=event;
  }
  assert.equal(final.stopReason,'finish'); assert.ok(sawGuidance);
  const elements=Object.values(final.spec.elements);
  const grid=elements.find(e=>e.type==='Grid');assert.equal(grid.children.length,2);
  assert.ok(preservedOrder);
  assert.deepEqual(grid.children.map(id=>final.spec.elements[id].props.label),['Roll','Clear']);
  assert.deepEqual(elements.find(e=>e.type==='Button'&&e.props.label==='Roll').on.press,draft.widgets[2].on.press);
  assert.ok(!elements.some(e=>e.type==='Keypad'));
  let selections=0, repaired=false;
  const omitDisplayOnce=async input=>{
    if (Object.hasOwn(input.questions,'root')) selections++;
    const result=await evaluate(input);
    if(selections===1) for(const [key,q] of Object.entries(input.questions)) {
      if(Object.hasOwn(q.criteria,'use:generated-result')) result.answers[key].choice='omit';
    }
    return result;
  };
  for await(const event of compose({prompt:'Build a random sampling interface'},omitDisplayOnce,false,async()=>content)) {
    if(event.type==='status' && event.message.includes('reconnecting')) repaired=true;
    final=event;
  }
  assert.ok(repaired);assert.equal(selections,2);assert.equal(final.stopReason,'finish');
  assert.throws(()=>prepareContent({state:{display:'0'},widgets:[{id:'fake',description:'Dice roller',type:'Keypad',props:{title:'Dice roller',mode:'calculator',value:{$bindState:'/display'}}}]}));
  const bad=structuredClone(draft);bad.widgets[2].on.press[0].params.op='exec';assert.throws(()=>prepareContent(bad));
  bad.widgets[2].on.press[0].params.op='randomInt';bad.widgets[2].on.press[0].params.args=[1,20];assert.throws(()=>prepareContent(bad));
});

test('private configuration and OpenAI requests keep provider credentials and budgets separate', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'ratatui-config-'));
  const file = join(dir, 'config.json');
  try {
    assert.deepEqual(loadConfig({ LLM_MODEL: 'override' }, file), { LLM_MODEL: 'override' });
    writeFileSync(file, JSON.stringify({ OPENAI_API_KEY: 'test-secret', LLM_MODEL: 'saved', UNRELATED: 'ignored' }));
    assert.deepEqual(loadConfig({ LLM_MODEL: 'override' }, file), { OPENAI_API_KEY: 'test-secret', LLM_MODEL: 'override' });
    writeFileSync(file, '{invalid secret content');
    assert.throws(() => loadConfig({}, file), /Check its JSON syntax/);
    writeFileSync(file, '{"OPENAI_API_KEY":42}');
    assert.throws(() => loadConfig({}, file), /must be strings/);
  } finally { rmSync(dir, { recursive: true }); }
  for (const local of [false, true]) {
    const env = { OPENAI_API_KEY: 'test-secret', ...(local ? { LLM_BASE_URL: 'http://localhost:8000/v1' } : {}) };
    const result = await generateContent({ prompt: 'Build a screen' }, env, async (url, options) => {
      const body = JSON.parse(options.body);
      assert.equal(options.redirect, 'error');
      assert.equal(body.response_format.type, local ? 'json_object' : 'json_schema');
      if (!local) {
        assert.equal(body.response_format.json_schema.strict, true);
        assert.equal(body.response_format.json_schema.schema.properties.state.maxItems,64);
        assert.ok(!JSON.stringify(body.response_format.json_schema.schema).includes('"oneOf"'));
        assert.equal(body.response_format.json_schema.schema.properties.widgets.maxItems, 32);
      }
      assert.equal(options.headers.Authorization, local ? undefined : 'Bearer test-secret');
      assert.equal(body.model, local ? 'latest' : 'gpt-5.6-terra');
      assert.equal(body.reasoning_effort, local ? 'low' : 'none');
      assert.equal(body.thinking_token_budget, local ? 512 : undefined);
      assert.equal(body.max_tokens, local ? 4096 : undefined);
      assert.equal(body.max_completion_tokens, local ? undefined : 4096);
      assert.equal(url.origin, local ? 'http://localhost:8000' : 'https://api.openai.com');
      return Response.json({ choices: [{ message: { content: JSON.stringify({ theme: palette, guidance: 'Heading only', state: local ? {} : [], widgets: [{ id: 'heading', description: 'App heading', type: 'Text', props: { text: 'Demo', muted: false } }] }) } }] });
    });
    assert.equal(result.generation.model, local ? 'latest' : 'gpt-5.6-terra');
  }
  await assert.rejects(generateContent({ prompt: 'Test' }, { LLM_BASE_URL: 'https://api.openai.com/v1' }), /Set OPENAI_API_KEY/);
});

test('real compositor produces a bound, actionable spec and edits it without mutating the original', async () => {
  const events = [];
  for await (const event of compose({ prompt: 'Operations overview' }, demoEvaluator(), true)) events.push(event);
  const final = events.at(-1);
  assert.equal(final.stopReason, 'finish');
  assert.ok(events.filter(e => e.type === 'step').length > 1);
  const original = structuredClone(final.spec);
  assert.equal(original.elements.node_6.props.value.$bindState, '/name');
  assert.equal(original.elements.node_7.on.press.action, 'setState');
  const edit = async ({ questions }) => ({ answers: { next: { choice: Object.hasOwn(questions.next.criteria, 'remove:node_4') ? 'remove:node_4' : 'finish' } } });
  let edited;
  for await (const event of compose({ prompt: 'Remove traffic', initialSpec: final.spec }, edit, false, prepared)) edited = event;
  assert.equal(edited.stopReason, 'finish');
  assert.equal(edited.spec.elements.node_4, undefined);
  assert.deepEqual(final.spec, original);
  assert.deepEqual(edited.spec.state, original.state);
});

test('batch select/layout actually uses evaluator choices', async () => {
  const phases = [];
  const evaluate = async ({ questions }) => {
    phases.push(Object.keys(questions));
    return { answers: Object.fromEntries(Object.entries(questions).map(([key, q]) => {
      const choices = Object.keys(q.criteria);
      const choice = key === 'root' ? 'column'
        : key.startsWith('select_') ? (choices.includes('use:services') ? 'use:services' : choices.includes('use:cpu') ? 'use:cpu' : choices.includes('omit') ? 'omit' : '0')
        : key.startsWith('order_') ? (key.endsWith('_1') ? '2' : '1') : choices[0];
      return [key, { choice }];
    })) };
  };
  let final;
  for await (const event of compose({ prompt: 'Services before CPU' }, evaluate, false, prepared)) final = event;
  assert.equal(final.stopReason, 'finish');
  assert.equal(phases.length, 2);
  assert.equal(final.spec.elements[final.spec.elements[final.spec.root].children[0]].type, 'Table');
});

test('gateway adapter sends bounded choice requests and propagates HTTP errors', async () => {
  let received;
  const server = createServer(async (req, res) => {
    let raw = ''; for await (const chunk of req) raw += chunk;
    received = JSON.parse(raw);
    if (received.model === 'fail') { res.writeHead(503); res.end(); return; }
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ answers: { next: { choice: 'finish', confidence: 0.8 } }, usage: { input_tokens: 12 } }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const env = { JEV_ENDPOINT: `http://127.0.0.1:${server.address().port}` };
    const request = { state: { user_request: 'hello' }, questions: { next: { type: 'choice', criteria: { finish: 'done' } } }, signal: AbortSignal.timeout(1000) };
    assert.equal((await evaluator(env)(request)).usage.inputTokens, 12);
    assert.deepEqual(JSON.parse(received.state), request.state);
    assert.deepEqual(received.questions, request.questions);
    await assert.rejects(evaluator({ ...env, JEV_MODEL: 'fail' })(request), /HTTP 503/);
    assert.throws(() => evaluator({}), /Set JEV_ENDPOINT/);
  } finally { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); }
});

test('invalid choices fail instead of becoming executable UI', async () => {
  await assert.rejects(async () => {
    for await (const _ of compose({ prompt: 'test' }, async () => ({ answers: { root: { choice: 'run-shell' } } }), false, prepared)) {}
  });
});

test('arbitrary requests go to the LLM and become new validated content before Jev composition', async () => {
  let posted;
  const draft = {
    theme: palette,
    state: { status: 'Docked', destination: 'Sol' },
    widgets: [
      { id: 'ship', description: 'Starship command heading', type: 'Text', props: { text: 'STARSHIP COMMAND' } },
      { id: 'destination', description: 'Destination input', type: 'Input', props: { label: 'Destination', value: { $bindState: '/destination' } } },
      { id: 'status', description: 'Flight status', type: 'Text', props: { text: { $state: '/status' } } },
      { id: 'launch', description: 'Launch control', type: 'Button', props: { label: 'Launch' }, on: { press: { action: 'setState', params: { statePath: '/status', value: 'Launch sequence ready' } } } },
    ],
  };
  const request = { prompt: 'Build an interface for a modern tradewars 2002 clone' };
  const generate = value => generateContent(value, {}, async (url, options) => {
    assert.equal(url.href, 'http://100.115.205.43:8000/v1/chat/completions');
    posted = JSON.parse(options.body);
    return Response.json({ model: 'latest', choices: [{ finish_reason: 'stop', message: { content: JSON.stringify(draft) } }], usage: { prompt_tokens: 100, completion_tokens: 80 } });
  });
  const evaluate = async ({ questions }) => ({ answers: Object.fromEntries(Object.entries(questions).map(([key, question]) => {
    let choice;
    if (key === 'root') choice = 'layout-column';
    else if (key.startsWith('select_')) choice = Object.keys(question.criteria).find(c => c.startsWith('use:')) || '0';
    else if (key.startsWith('order_')) choice = key.split('_').at(-1);
    else choice = Object.keys(question.criteria)[0];
    return [key, { choice }];
  })) });
  const events = [];
  for await (const event of compose(request, evaluate, false, generate)) events.push(event);
  assert.equal(JSON.parse(posted.messages[1].content).request, request.prompt);
  assert.equal(posted.reasoning_effort, 'low');
  assert.equal(posted.thinking_token_budget, 512);
  assert.equal(posted.max_tokens, 4096);
  assert.equal(events[0].type, 'status');
  const final = events.at(-1);
  assert.equal(final.stopReason, 'finish');
  assert.match(JSON.stringify(final.spec), /STARSHIP COMMAND/);
  assert.doesNotMatch(JSON.stringify(final.spec), /Revenue|Services|Ada/);
  assert.equal(final.generation.usage.completion_tokens, 80);
  const edited = prepareContent({ state: { destination: 'Overwrite', newField: 'New' }, widgets: [] }, final.spec);
  assert.equal(edited.state.destination, 'Sol');
  assert.equal(edited.state.newField, 'New');
  assert.ok(edited.candidates.some(c => c.element.type === 'Button'));
  assert.throws(() => prepareContent({ ...draft, widgets: [draft.widgets[0], draft.widgets[0]] }), /unique/);
  const invalid = structuredClone(draft);
  invalid.widgets[3].on.press.params.value = { unsupported: true };
  assert.throws(() => prepareContent(invalid));
  invalid.widgets[3].on.press.action = 'exec';
  assert.throws(() => prepareContent(invalid));
  assert.throws(() => prepareContent(JSON.parse('{"state":{"__proto__":{}},"widgets":[]}'), final.spec), /Unsafe/);
  const table = { state: {}, widgets: [{ id: 't', description: 'Data', type: 'Table', props: { title: 'Test', headers: ['A', 'B'], rows: [['one']] } }] };
  assert.throws(() => prepareContent(table), /headers/);
  await assert.rejects(generateContent(request, {}, async () => Response.json({ choices: [{ finish_reason: 'length' }] })), /output limit/);
  await assert.rejects(generateContent(request, {}, async () => Response.json({ choices: [{ message: { content: 'not JSON' } }] })), /invalid JSON/);
  let repairCalls = 0;
  const repaired = await generateContent(request, {}, async (_url, options) => {
    const body = JSON.parse(options.body);
    assert.equal(body.reasoning_effort, 'low');
    assert.equal(body.thinking_token_budget, 512);
    assert.equal(body.max_tokens, 4096);
    if (repairCalls++) assert.match(body.messages.at(-1).content, /validation error/);
    return Response.json({ choices: [{ message: { content: repairCalls === 1 ? 'invalid JSON' : JSON.stringify(draft) } }], usage: { prompt_tokens: 100, completion_tokens: 80, total_tokens: 180 } });
  });
  assert.equal(repairCalls, 2);
  assert.equal(repaired.generation.calls.length, 2);
  assert.equal(repaired.generation.usage.total_tokens, 360);
});
