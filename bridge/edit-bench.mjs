// Paired end-to-end benchmarks. Native validation and repairs remain enabled.
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {isDeepStrictEqual} from 'node:util';
import {build} from './build.mjs';
import {generateContent} from './generate.mjs';
import {loadConfig,evaluator} from './compose.mjs';
import {catalog} from './catalog.mjs';
const [outArg,baselineArg,repeatsArg='3']=process.argv.slice(2);
if(!outArg||!baselineArg) throw Error('Usage: node bridge/edit-bench.mjs OUTPUT_DIR BASELINE_BRIDGE_DIR [REPEATS]');
const out=resolve(outArg),repeats=Number(repeatsArg);
if(!Number.isInteger(repeats)||repeats<1||repeats>10) throw Error('Repeats must be 1..10');
mkdirSync(out,{recursive:true});
const baseline=await import(pathToFileURL(join(resolve(baselineArg),'generate.mjs')));
const baselineBuild=await import(pathToFileURL(join(resolve(baselineArg),'build.mjs')));
const env=loadConfig();
if(!env.RATATUI_JSON_NATIVE) throw Error('RATATUI_JSON_NATIVE is required.');
// Capture immutable copies supplied by the caller, never write a user project.
const dashboard=JSON.parse(readFileSync(join(out,'dashboard.json'),'utf8'));
const chat=JSON.parse(readFileSync(join(out,'chat.json'),'utf8'));
const alerts=[['INFO','All nodes ready','ALL'],['WARN','Check cooling','SPARK-07']];
const cases=[
 {name:'thresholds',prompt:'Color code GPU utilization on both the metric and chart: green below 70%, yellow from 70%, red from 90%. Preserve everything else.',spec:dashboard,allowed:{gpu_util_metric:['color','thresholds'],utilization_chart:['color','thresholds']},check:s=>['gpu_util_metric','utilization_chart'].every(id=>{
  const p=s.elements[id].props,t=p.thresholds??[];
  // Judge rendered threshold semantics, not one preferred encoding (min:0 is valid).
  return [0,69.999,70,89.999,90,100,...t.map(x=>x.min).filter(n=>n>=0&&n<=100)].every(value=>{
   const color=t.filter(x=>value>=x.min).at(-1)?.color??p.color;
   return (value<70?['success','green']:value<90?['warning','yellow']:['danger','red']).includes(color);
  });
 })},
 {name:'table-rows',prompt:`Replace only the alerts table rows with ${JSON.stringify(alerts)}. Preserve its headers, styles, and everything else.`,spec:dashboard,allowed:{alerts_table:['rows']},check:s=>isDeepStrictEqual(s.elements.alerts_table.props.rows,alerts)},
 {name:'table-columns',prompt:'In the alerts table, set the severity column to 7 cells, the message column to flexible width (0), and the node column to 9 cells. Preserve their existing colors and alignment and all other content/layout.',spec:dashboard,allowed:{alerts_table:['columnStyles']},check:s=>isDeepStrictEqual(s.elements.alerts_table.props.columnStyles,dashboard.elements.alerts_table.props.columnStyles.map((c,i)=>({...c,width:[7,0,9][i]})))},
 {name:'rail-width',prompt:'Make the channel rail exactly 26 cells wide. Let the chat area fill the remaining width. Preserve the existing one-cell gap, all content, actions and state.',spec:chat,allowed:{'channel-rail':['width','maxWidth'],'chat-area':['width','maxWidth']},check:(s,r)=>r.layout.nodes['channel-rail'].bounds[2]===26&&r.layout.nodes['chat-area'].bounds[0]===27&&r.layout.nodes['chat-area'].bounds[2]===133},
];
function canonical(spec,allowed) {
 const s=structuredClone(spec);
 for(const [id,e] of Object.entries(s.elements)) {
  for(const [key,schema] of Object.entries(catalog.data.components[e.type].props.shape)) {
   if(!Object.hasOwn(e.props,key)) {const p=schema.safeParse(undefined);if(p.success&&p.data!==undefined)e.props[key]=p.data;}
  }
  for(const key of allowed[id]??[]) delete e.props[key];
  e.children??=[];e.on??={};e.visible??=true;
 }
 return s;
}
const variants=[
 {name:'baseline-luna',model:'gpt-5.6-luna',run:baselineBuild.build,generate:baseline.generateContent},
 {name:'patch-luna',model:'gpt-5.6-luna',run:build,generate:generateContent},
 {name:'patch-mini',model:'gpt-4.1-mini',run:build,generate:generateContent},
];
const rows=[];
for(let repeat=0;repeat<repeats;repeat++) for(const c of cases) {
 const order=variants.map((_,i)=>variants[(i+repeat)%variants.length]);
 for(const v of order) {
  const calls=[];let proposals=[];const jevCalls=[];
  const config={...env,LLM_MODEL:v.model};
  const evalJev=evaluator(config);
  const observed=async(url,options)=>{
   const request=JSON.parse(options.body);const t=performance.now();
   const response=await fetch(url,options);
   return {ok:response.ok,status:response.status,json:async()=>{
    const result=await response.json();calls.push({model:result.model,usage:result.usage??null,elapsedMs:Math.round(performance.now()-t)});
    try {proposals.push(JSON.parse(result.choices[0].message.content));}catch{}
    return result;
   }};
  };
  const started=performance.now();let row={repeat,case:c.name,variant:v.name,model:v.model,prompt:c.prompt};
  try {
   const result=await v.run({prompt:c.prompt,initialSpec:structuredClone(c.spec),engine:'jev',viewport:[160,60]},config,(r,e)=>v.generate(r,e,observed),async params=>{const t=performance.now();const result=await evalJev(params);jevCalls.push({elapsedMs:Math.round(performance.now()-t),model:result.model,usage:result.usage,costUsd:result.costUsd??null});return result;});
   const requested=c.check(result.spec,result),preserved=isDeepStrictEqual(canonical(result.spec,c.allowed),canonical(c.spec,c.allowed));
   row={...row,pass:requested&&preserved,requested,preserved,audits:result.layout.audits,design:result.design?.selection};
   writeFileSync(join(out,`${repeat}-${c.name}-${v.name}.json`),JSON.stringify({spec:result.spec,proposals},null,2)+'\n');
  } catch(error) {row={...row,pass:false,error:error.message};}
  if(row.error) writeFileSync(join(out,`${repeat}-${c.name}-${v.name}.json`),JSON.stringify({error:row.error,proposals},null,2)+'\n');
  row={...row,elapsedMs:Math.round(performance.now()-started),calls,jevCalls,outputTokens:calls.reduce((n,c)=>n+(c.usage?.completion_tokens??0),0),cachedTokens:calls.reduce((n,c)=>n+(c.usage?.prompt_tokens_details?.cached_tokens??0),0),updates:proposals.at(-1)?.updates??null,replacementWidgets:proposals.at(-1)?.widgets?.length??null};
  rows.push(row);writeFileSync(join(out,'results.json'),JSON.stringify({created:new Date().toISOString(),repeats,rows},null,2)+'\n');
  if(rows.length===1 && row.error==='fetch failed' && !calls.length && !jevCalls.length) throw new Error(`Benchmark infrastructure failure: ${row.error}`);
  console.log(JSON.stringify({repeat,case:c.name,variant:v.name,pass:row.pass,requested:row.requested,preserved:row.preserved,ms:row.elapsedMs,tokens:row.outputTokens,calls:calls.length,error:row.error}));
 }
}
for(const v of variants) {
 const r=rows.filter(r=>r.variant===v.name),times=r.map(r=>r.elapsedMs).sort((a,b)=>a-b);
 console.log(JSON.stringify({variant:v.name,passed:r.filter(r=>r.pass).length,total:r.length,medianMs:(times[Math.floor((times.length-1)/2)]+times[Math.floor(times.length/2)])/2,outputTokens:r.reduce((n,r)=>n+r.outputTokens,0),repairs:r.reduce((n,r)=>n+Math.max(0,r.calls.length-1),0)}));
}
