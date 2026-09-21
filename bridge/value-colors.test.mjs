import test from 'node:test';
import assert from 'node:assert/strict';
import {prepareDocument,generateContent} from './generate.mjs';
import {catalog} from './catalog.mjs';
const thresholds=[{min:70,color:'yellow'},{min:90,color:'red'}];
const initial={root:'root',state:{gpu:87},elements:{root:{type:'Column',props:{},children:['gpu']},gpu:{type:'Metric',props:{label:'GPU utilization %',value:{$state:'/gpu'}}}}};
test('color edits and threshold replacements validate and preserve metric bindings',async()=>{
 const spec=structuredClone(initial);
 spec.elements.gpu.props={...spec.elements.gpu.props,color:'green',thresholds};
 prepareDocument(spec);
 for(const type of ['Metric','Gauge','Sparkline','BarChart']) {
  assert.ok(catalog.data.components[type].props.shape.thresholds);
  assert.ok(catalog.data.components[type].props.shape.color);
 }
 spec.elements.gpu.props.thresholds=[...thresholds].reverse();
 prepareDocument(spec);
 assert.deepEqual(spec.elements.gpu.props.thresholds,thresholds);
 for(const bad of [[{min:70,color:'red'},{min:70,color:'yellow'}],[{min:0,color:'invalid'}]]) {
  spec.elements.gpu.props.thresholds=bad;
  assert.throws(()=>prepareDocument(spec));
 }
 const theme=Object.fromEntries(['background','surface','foreground','muted','border','primary','focus','danger','success','warning'].map(k=>[k,'#112233']));
 const draft={theme:null,state:{},guidance:'Color utilization',widgets:[{id:'gpu',description:'Live GPU usage',...initial.elements.gpu,props:{...initial.elements.gpu.props,color:'green',thresholds}}],placements:[],updates:[],remove:[],stateUpdates:[],layoutChecks:[],geometryChecks:[]};
 const result=await generateContent({prompt:'color code GPU utilization green yellow red',direct:true,initialSpec:{...initial,theme}}, {LLM_BASE_URL:'http://localhost:8000/v1'},async()=>({ok:true,json:async()=>({choices:[{message:{content:JSON.stringify(draft)}}]})}));
 assert.deepEqual(result.spec.elements.gpu.props.value,{$state:'/gpu'});
 assert.deepEqual(result.spec.state,initial.state);
 assert.deepEqual(result.spec.theme,theme);
 assert.deepEqual(result.spec.elements.gpu.props.thresholds,thresholds);
});
