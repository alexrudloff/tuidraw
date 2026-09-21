// Sequential paired requests; reverse order on alternating repeats to reduce warm-up bias.
import {writeFileSync} from 'node:fs';
import {loadConfig,evaluator} from './compose.mjs';
import {selectDesign,compileTheme} from './design.mjs';
import {measureLayout} from './layout.mjs';
const env=loadConfig();
const cases=[
 ['Compact dark amber BBS with double borders and bold titles',{canvas:'dark',hue:'amber',border:'double',density:'compact',emphasis:'bold'}],
 ['Light blue interface with rounded borders, comfortable spacing, restrained headings',{canvas:'light',hue:'blue',border:'rounded',density:'comfortable',emphasis:'quiet'}],
 ['Dark violet console. Square single borders, compact layout, quiet headings',{canvas:'dark',hue:'violet',border:'plain',density:'compact',emphasis:'quiet'}],
 ['Light green terminal garden with heavy borders, comfortable spacing, bold headings',{canvas:'light',hue:'green',border:'heavy',density:'comfortable',emphasis:'bold'}],
 ['Move the Send button below the input. Keep the current styling.',{change:'preserve'}],
 ['Change only the shared style to dark cyan, rounded borders, compact spacing and bold headings',{change:'apply',canvas:'dark',hue:'cyan',border:'rounded',density:'compact',emphasis:'bold'}],
];
const initialTheme=compileTheme({change:'apply',canvas:'light',hue:'rose',border:'plain',density:'comfortable',emphasis:'quiet'});
const rows=[];
const output=process.argv[2]??'design-benchmark.json';
const repeats=Number(process.argv[3]??3);
if(!Number.isInteger(repeats)||repeats<1||repeats>10) throw Error('Repeats must be 1..10');
for(let repeat=0;repeat<repeats;repeat++) for(const [index,[prompt,expected]] of cases.entries()) {
 for(const engine of repeat%2?['jev','llm']:['llm','jev']) {
  const started=performance.now();
  let row={repeat,index,engine,prompt,expected};
  try {
   const result=await selectDesign({prompt,...(index>=4?{initialSpec:{theme:initialTheme}}:{})},engine,env,engine==='jev'?evaluator(env):null);
   row={...row,...result};
   let nativeAudits=null;
   if(env.RATATUI_JSON_NATIVE) {
    const spec={root:'root',theme:result.theme??initialTheme,state:{message:''},elements:{root:{type:'Panel',props:{title:'Shared style'},children:['metric','input']},metric:{type:'Metric',props:{label:'Score',value:'42'}},input:{type:'Input',props:{label:'Message',value:{$bindState:'/message'}}}}};
    nativeAudits=[...measureLayout(spec,[80,24],env.RATATUI_JSON_NATIVE).audits,...measureLayout(spec,[120,36],env.RATATUI_JSON_NATIVE).audits];
   }
   const checks=Object.entries(expected).map(([key,value])=>({key,pass:result.selection[key]===value}));
   row={...row,...result,checks,nativeAudits,pass:checks.every(c=>c.pass)&&!(nativeAudits??[]).some(a=>a.severity==='error')};
  } catch(error) {row={...row,error:error.message,pass:false,elapsedMs:Math.round(performance.now()-started)};}
  rows.push(row);writeFileSync(output,JSON.stringify({created:new Date().toISOString(),repeats,rows},null,2)+'\n');
  console.log(JSON.stringify({repeat,index,engine,pass:row.pass,elapsedMs:row.elapsedMs,error:row.error}));
 }
}
for(const engine of ['llm','jev']) {
 const all=rows.filter(r=>r.engine===engine),ok=all.filter(r=>!r.error),times=ok.map(r=>r.elapsedMs).sort((a,b)=>a-b);
 console.log(JSON.stringify({engine,passed:all.filter(r=>r.pass).length,total:all.length,errors:all.length-ok.length,medianMs:times.length?(times[Math.floor((times.length-1)/2)]+times[Math.floor(times.length/2)])/2:null,reportedCostUsd:ok.length && ok.every(r=>typeof r.costUsd==='number')?ok.reduce((sum,r)=>sum+r.costUsd,0):null}));
}
