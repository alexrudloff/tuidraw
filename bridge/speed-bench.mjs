// Paid live comparison. Each run starts from immutable fixtures; never writes a user project.
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {performance} from 'node:perf_hooks';
import {isDeepStrictEqual} from 'node:util';
import {runAgent} from './agent.mjs';
import {loadConfig} from './compose.mjs';
import {prepareDocument} from './generate.mjs';
import {measureLayout} from './layout.mjs';
const [baselineDir,fixturePath,outDir,repeatsArg='2']=process.argv.slice(2);
if(!baselineDir||!fixturePath||!outDir)throw new Error('Usage: node bridge/speed-bench.mjs BASELINE_BRIDGE CHAT_FIXTURE OUT_DIR [REPEATS]');
const baseline=(await import(pathToFileURL(join(resolve(baselineDir),'agent.mjs')))).runAgent;
const env={...loadConfig(),RATATUI_JSON_NATIVE:process.env.RATATUI_JSON_NATIVE??resolve('target/release/tui-draw')};
const chat=JSON.parse(readFileSync(fixturePath));
// The old fixture predates visible accelerators; remove its obsolete button cap for both variants.
chat.elements['send-button'].props.maxWidth=0;prepareDocument(chat);
mkdirSync(outDir,{recursive:true});writeFileSync(join(outDir,'chat.json'),JSON.stringify(chat));
const tasks=[
 {name:'width',prompt:'Widen the channel rail to 26 cells',spec:chat,check:r=>r.layout.nodes['channel-rail'].bounds[2]===26},
 {name:'title',prompt:'Change the application title to Orbit Relay.',spec:chat,check:r=>r.spec.elements.title.props.text==='Orbit Relay'},
 {name:'new-game',prompt:'Create an interface for a modern clone of tradewars 2002',check:r=>Object.values(r.spec.elements).some(e=>['Scene','AnsiArt'].includes(e.type))&&Object.values(r.spec.elements).some(e=>e.type==='Button')},
];
const rows=[];
for(let repeat=0;repeat<Number(repeatsArg);repeat++)for(const task of tasks)for(const name of (repeat%2?['after','before']:['before','after'])) {
 const start=performance.now();let result,error;
 try {
  result=await (name==='before'?baseline:runAgent)({prompt:task.prompt,initialSpec:structuredClone(task.spec),engine:'jev',viewport:[110,35]},env);
  result.layout=measureLayout(result.spec,[110,35],env.RATATUI_JSON_NATIVE);
  if(!result.changed||result.layout.audits.some(a=>a.severity==='error')||!task.check(result))throw new Error('Acceptance check failed');
  if(task.spec) {
   if(!isDeepStrictEqual(result.spec.state,task.spec.state))throw new Error('State preservation failed');
   for(const [id,element] of Object.entries(task.spec.elements)) {
    const actual=structuredClone(result.spec.elements[id]);const expected=structuredClone(element);
    if(task.name==='width'&&id==='channel-rail')for(const key of ['width','maxWidth','widthPercent']){delete actual.props[key];delete expected.props[key];}
    if(task.name==='title'&&id==='title'){delete actual.props.text;delete expected.props.text;}
    if(!isDeepStrictEqual(actual,expected))throw new Error(`Unrelated widget changed: ${id}`);
   }
  }
 }catch(e){error=e.message;result??=e.diagnostics;}
 const calls=result?.generation?.calls??[];
 const row={variant:name,task:task.name,repeat,ok:!error,error,elapsedMs:Math.round(performance.now()-start),contentCalls:calls.length,outputTokens:calls.reduce((n,c)=>n+(c.usage?.completion_tokens??0),0),inputTokens:calls.reduce((n,c)=>n+(c.usage?.prompt_tokens??0),0),repairErrors:(result?.messages??[]).filter(m=>m.role==='tool'&&JSON.parse(m.content).error).length,jevReportedCostUsd:[result?.design,...(result?.decisions??[])].reduce((n,d)=>n+(d?.costUsd??0),0),decisions:result?.decisions??[]};
 rows.push(row);writeFileSync(join(outDir,`${repeat}-${task.name}-${name}.json`),JSON.stringify(result));writeFileSync(join(outDir,'results.json'),JSON.stringify(rows,null,2));console.log(JSON.stringify(row));
}
