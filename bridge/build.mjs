import {runAgent} from './agent.mjs';
import { pathToFileURL } from 'node:url';
import { loadConfig, evaluator } from './compose.mjs';
import { generateContent, prepareDocument } from './generate.mjs';
import {selectDesign} from './design.mjs';
import { groundOperations } from './ground.mjs';

export async function build(request, env=loadConfig(), generate=generateContent, evaluate) {
  if(typeof request.prompt!=='string' || !request.prompt.trim() || request.prompt.length>4000) throw new Error('Enter a prompt between 1 and 4,000 characters.');
  const engine=request.engine??'jev';
  if(!['llm','jev'].includes(engine)) throw new Error('Engine must be llm or jev.');
  const start=performance.now();
  const design=engine==='jev' ? await selectDesign(request,'jev',env,evaluate??evaluator(env)) : null;
  let content=await generate({...request,direct:true,...(design?{selectedTheme:design.theme??request.initialSpec?.theme}:{})},env);
  if(engine==='jev') {
    content=await groundOperations(content,evaluate??(params=>evaluator(env)(params)),request.prompt,AbortSignal.timeout(15000));
    for(const [id,element] of Object.entries(content.spec.elements)) {
      const candidate=content.candidates.find(c=>c.id===`generated-${id}`);
      if(candidate?.element.on) element.on=structuredClone(candidate.element.on);
    }
    prepareDocument(content.spec);
  }
  return {type:'complete',stopReason:'finish',spec:content.spec,summary:content.guidance,engine,elapsedMs:Math.round(performance.now()-start),generation:content.generation,design,grounding:content.grounding??null,layout:content.layout,geometryChecks:content.geometryChecks};
}

if(process.argv[1] && import.meta.url===pathToFileURL(process.argv[1]).href) {
  try {
    let input='';
    for await(const chunk of process.stdin) {input+=chunk;if(Buffer.byteLength(input)>2_000_000) throw new Error('Request too large.');}
    console.log(JSON.stringify({type:'status',message:'Thinking…'}));
    console.log(JSON.stringify(await runAgent(JSON.parse(input),loadConfig(),{emit:event=>console.log(JSON.stringify(event))})));
  } catch(error) { console.log(JSON.stringify({type:'error',message:error.message,...(error.diagnostics?{diagnostics:error.diagnostics}:{})}));process.exitCode=1; }
}
