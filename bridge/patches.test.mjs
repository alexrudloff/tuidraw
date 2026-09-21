import test from 'node:test';
import assert from 'node:assert/strict';
import {applyDraft} from './document.mjs';
import {generateContent} from './generate.mjs';
const theme=Object.fromEntries(['background','surface','foreground','muted','border','primary','focus','danger','success','warning'].map(k=>[k,'#123456']));
const initial={root:'root',theme,state:{load:87},elements:{root:{type:'Column',props:{},children:['gpu']},gpu:{type:'Metric',props:{label:'GPU',value:{$state:'/load'}}}}};
const thresholds=[{min:70,color:'yellow'},{min:90,color:'red'}];
const patch={id:'gpu',key:'thresholds',json:JSON.stringify(thresholds)};
const empty={guidance:'Updated',theme:null,state:[],widgets:[],updates:[],placements:[],remove:[],stateUpdates:[],layoutChecks:[],geometryChecks:[]};
test('complex property patches preserve live bindings and pending edits through repair',async()=>{
 let calls=0;
 const result=await generateContent({prompt:'Color GPU usage',initialSpec:initial,direct:true},{OPENAI_API_KEY:'fake'},async()=>({ok:true,json:async()=>({model:'fake',choices:[{message:{content:JSON.stringify(calls++?{...empty,updates:[patch,{id:'gpu',key:'color',value:'green'}],placements:null,remove:null,stateUpdates:null,layoutChecks:null,geometryChecks:null}:{...empty,updates:[{...patch,json:'['},{id:'gpu',key:'color',value:'green'}]})}}]})}));
 assert.equal(calls,2);assert.deepEqual(result.spec.elements.gpu.props.thresholds,thresholds);
 assert.deepEqual(result.spec.elements.gpu.props.value,{$state:'/load'});
 assert.equal(result.spec.elements.gpu.props.color,'green');
 assert.deepEqual(result.spec.state,initial.state);
 assert.equal(initial.elements.gpu.props.thresholds,undefined);
 assert.ok(result.generation.calls.every(c=>Number.isFinite(c.elapsedMs)));
});
test('malformed, unsafe and overnested patches are rejected without mutating the source',()=>{
 const draft={...empty,state:{}};
 for(const json of ['{','{"__proto__":{"polluted":true}}','['.repeat(14)+'0'+']'.repeat(14)]) {
  assert.throws(()=>applyDraft({...draft,updates:[{...patch,json}]},initial));
 }
 assert.throws(()=>applyDraft({...draft,updates:[{...patch,value:0}]},initial));
 assert.equal({}.polluted,undefined);assert.equal(initial.elements.gpu.props.thresholds,undefined);
});

test('object patches remain subject to binding and widget validation',async()=>{
 const run=async json=>generateContent({prompt:'Change the readout binding',initialSpec:initial,direct:true},{OPENAI_API_KEY:'fake'},async()=>({ok:true,json:async()=>({choices:[{message:{content:JSON.stringify({...empty,updates:[{id:'gpu',key:'value',json}],state:[{key:'other',value:42}]})}}]})}));
 const result=await run(JSON.stringify({$state:'/other'}));
 assert.deepEqual(result.spec.elements.gpu.props.value,{$state:'/other'});
 assert.equal(result.spec.elements.gpu.props.label,'GPU');
 assert.deepEqual(initial.state,{load:87});
 await assert.rejects(run(JSON.stringify({$state:'/missing'})),/Missing generated state/);
});
