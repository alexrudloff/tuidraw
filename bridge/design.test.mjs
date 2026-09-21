import test from 'node:test';
import assert from 'node:assert/strict';
import {selectDesign,compileTheme,usabilityInstructions} from './design.mjs';
import {generateContent} from './generate.mjs';
import {build} from './build.mjs';
const selection={change:'apply',canvas:'dark',hue:'cyan',border:'double',density:'compact',emphasis:'bold'};
const theme=compileTheme(selection);
const draft={guidance:'Simple interface',theme,state:{},widgets:[{id:'title',description:'Title',type:'Text',props:{text:'Hello'}}],placements:[],updates:[],remove:[],stateUpdates:[],layoutChecks:[],geometryChecks:[]};
const env={LLM_BASE_URL:'http://localhost:8000/v1'};
test('bounded design choices reject malformed answers and preserve unrelated feedback',async()=>{
 const evaluate=async({state})=>{assert.equal(state.usability,usabilityInstructions);return {answers:Object.fromEntries(Object.entries(selection).map(([k,choice])=>[k,{choice}])),usage:{inputTokens:12}};};
 const selected=await selectDesign({prompt:'BBS'},'jev',{},evaluate);
 assert.deepEqual(selected.theme.design,{border:'double',density:'compact',emphasis:'bold'});
 assert.equal(compileTheme({...selection,change:'preserve'}),null);
 await assert.rejects(selectDesign({prompt:'BBS'},'jev',{},async()=>({answers:{}})));
 const result=await build({prompt:'BBS',engine:'jev'},env,async request=>{
   assert.deepEqual(request.selectedTheme,theme);
   return {spec:{root:'root',theme:request.selectedTheme,state:{},elements:{root:{type:'Column',props:{},children:['title']},title:{type:'Text',props:{text:'Hello'}}}},candidates:[]};
 },evaluate);
 assert.deepEqual(result.spec.theme,theme);
});
test('native audit errors trigger one repair; warnings do not, and recoloring preserves design',async()=>{
 let calls=0;
 const fake=async(_url,options)=>{assert.ok(JSON.parse(options.body).messages[0].content.includes(usabilityInstructions));return {ok:true,json:async()=>({choices:[{message:{content:JSON.stringify(draft)}}]})};};
 const result=await generateContent({prompt:'hello',direct:true},env,fake,async()=>({viewport:[80,24],nodes:{},audits:++calls===1?[{id:'send',severity:'error',message:'Control requires width 8'}]:[{id:'art',severity:'warning',message:'Clipped'}]}));
 assert.equal(calls,2);assert.equal(result.generation.calls.length,2);
 const recolor={...draft,theme:{...theme,background:'#121212'},widgets:[]};delete recolor.theme.design;
 const edit=await generateContent({prompt:'recolor',direct:true,initialSpec:result.spec},env,async(_url,options)=>{assert.ok(JSON.parse(options.body).messages[0].content.includes(usabilityInstructions));return {ok:true,json:async()=>({choices:[{message:{content:JSON.stringify(recolor)}}]})};});
 assert.deepEqual(edit.spec.theme.design,theme.design);
});
