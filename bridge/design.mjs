import {z} from 'zod';

// Independent choices, shared by both selectors. No screen/domain presets.
export const axes={
  canvas:{dark:'Dark background',light:'Light background'},
  hue:{blue:'Blue accent',green:'Green accent',amber:'Amber/gold accent',violet:'Violet/purple accent',rose:'Rose/red accent',cyan:'Cyan/teal accent'},
  border:{plain:'Square single-line borders',rounded:'Rounded single-line borders',double:'Double-line BBS borders',heavy:'Heavy bold borders'},
  density:{compact:'Compact, no extra container spacing or padding',comfortable:'One cell of container spacing and panel/table padding'},
  emphasis:{quiet:'Restrained headings in foreground color',bold:'Bold accent-colored headings'},
};
export const designSchema=z.object(Object.fromEntries(['border','density','emphasis'].map(k=>[k,z.enum(Object.keys(axes[k]))]))).strict();
export const selectionSchema=z.object({change:z.enum(['apply','preserve']),...Object.fromEntries(Object.entries(axes).map(([k,v])=>[k,z.enum(Object.keys(v))]))}).strict();
export const designInstructions='theme.design is shared chrome: {border:plain|rounded|double|heavy,density:compact|comfortable,emphasis:quiet|bold}. New apps should choose it for the request. Preserve it during unrelated edits. Panel/Table border and padding, container gap, and Table columnGap accept null to inherit. Use null normally; numbers or border names are intentional local overrides. Every bordered primitive inherits shared border shape; panel titles and metric values inherit emphasis. Comfortable density adds one cell of spacing and padding; compact adds none. Explicit layout requests take precedence.';

// Shared by content generation/refinement and bounded Jev decisions. No extra model call.
export const usabilityInstructions = `Design around the user's main task and familiar spatial structure. Give the working area priority over branding. Use one compact heading per region; remove repeated titles, instructions that restate labels, and ornamental status panels. Group by task and scope: global actions belong together, selected-item actions stay near their item, and inputs stay with their results. Prefer spacing and alignment over nested borders; use borderless Select/MultiSelect inside a labeled Panel. Give each action group one clear primary action. Use compact content-width plain buttons for secondary actions, not equal-weight full-width button grids unless the task calls for a keypad. Keep destructive entry actions quiet; reserve strong danger emphasis for a destructive confirmation, with Cancel first. Labels must describe the actual outcome, including confirmation or typed-path steps. Every Button must have a working, visible hotkey. Button.hotkey accepts unique Alt+letter/digit or F1–F24; null lets code assign one. Existing shortcuts persist during refinement. The renderer adds the key to the label, so allocate width for it; do not put shortcut text in label. Use modified keys so text entry remains ordinary typing. Show only keyboard hints and number shortcuts the runtime implements; never imply Enter always submits regardless of focus. Keep focus identifiable without color alone. Put success/error feedback near its action without replacing instructions; bind feedback to real state changes. Offer cancellation or undo only when implemented, never fake recovery with a status message. Before returning, check the primary task, action priority, grouping, reading order, honest labels, empty state and recovery. Explicit user style and layout requests take precedence.`;

export function compileTheme(selection) {
  const s=selectionSchema.parse(selection);
  if(s.change==='preserve') return null;
  const light=s.canvas==='light';
  const hues={blue:['#2563eb','#79aaff'],green:['#15803d','#67dc92'],amber:['#925a00','#ffc65c'],violet:['#7e22ce','#cc9eff'],rose:['#be123c','#ff91ac'],cyan:['#0e7490','#6bdded']};
  return {
    background:light?'#f5f5f4':'#101419',surface:light?'#e5e7eb':'#242b35',
    foreground:light?'#171b23':'#f1f3f5',muted:light?'#4b5563':'#b3bdca',
    border:light?'#64748b':'#6e7c91',primary:hues[s.hue][light?0:1],focus:hues[s.hue][light?0:1],
    danger:light?'#b91c1c':'#ff8585',success:light?'#166534':'#6bdd97',warning:light?'#854d0e':'#ffd166',
    design:{border:s.border,density:s.density,emphasis:s.emphasis},
  };
}

export async function selectDesign(request,engine,env,evaluate,fetcher=globalThis.fetch,signal) {
  const start=performance.now();
  const timeout=signal?AbortSignal.any([signal,AbortSignal.timeout(15000)]):AbortSignal.timeout(15000);
  const state={request:request.prompt,currentTheme:request.initialSpec?.theme??null,usability:usabilityInstructions,
    instruction:'Choose terminal visual settings. Apply for a new interface or requested overall restyling; preserve for unrelated feedback and requests that only color individual widgets or data by thresholds. Honor explicit choices. Favor quiet emphasis and compact density for task-focused utilities unless the request benefits from more space or expressive styling. Do not use heavy borders or bold emphasis as a substitute for hierarchy. Axes are independent.'};
  let selection,usage,model,costUsd=null;
  if(engine==='jev') {
    const questions={change:{type:'choice',instructions:'Should shared styling change?',criteria:{apply:'New app or request changes visual styling',preserve:'Existing app without a shared style change; local widget colors and value thresholds preserve the overall theme'}},
      ...Object.fromEntries(Object.entries(axes).map(([key,criteria])=>[key,{type:'choice',instructions:`Choose ${key} for the request`,criteria}]))};
    const result=await evaluate({state,questions,signal:timeout});
    selection=Object.fromEntries(Object.keys(questions).map(key=>[key,result.answers?.[key]?.choice]));
    usage=result.usage??null; model=result.model??env.JEV_MODEL??'jev';costUsd=result.costUsd??null;
  } else {
    const base=(env.LLM_BASE_URL||(env.OPENAI_API_KEY?'https://api.openai.com/v1':'http://100.115.205.43:8000/v1')).replace(/\/$/,'');
    const url=new URL(`${base}/chat/completions`);
    if(!['http:','https:'].includes(url.protocol)) throw new Error('LLM_BASE_URL must be HTTP(S).');
    const openai=url.origin==='https://api.openai.com';
    model=env.LLM_MODEL||(openai?'gpt-5.6-terra':'latest');
    const key=openai?(env.OPENAI_API_KEY||env.LLM_API_KEY):env.LLM_API_KEY;
    const schema=z.toJSONSchema(selectionSchema);delete schema.$schema;
    const response=await fetcher(url,{method:'POST',redirect:'error',signal:timeout,headers:{'Content-Type':'application/json',...(key?{Authorization:`Bearer ${key}`}:{})},body:JSON.stringify({model,
      messages:[{role:'system',content:`Select bounded terminal design choices. Return only JSON matching the schema. Options: ${JSON.stringify(axes)}`},{role:'user',content:JSON.stringify(state)}],
      response_format:openai?{type:'json_schema',json_schema:{name:'terminal_design',strict:true,schema}}:{type:'json_object'},
      ...(openai?{max_completion_tokens:512,...(model.startsWith('gpt-5')?{reasoning_effort:'none'}:{})}:{max_tokens:512,reasoning_effort:'low',thinking_token_budget:512}),
    })});
    if(!response.ok) throw new Error(`Design model returned HTTP ${response.status}.`);
    const result=await response.json();
    selection=JSON.parse(result.choices?.[0]?.message?.content);usage=result.usage??null;model=result.model??model;
  }
  selection=selectionSchema.parse(selection);
  if(!request.initialSpec && selection.change==='preserve') throw new Error('A new interface requires an applied design.');
  return {selection,theme:compileTheme(selection),engine,model,usage,costUsd,elapsedMs:Math.round(performance.now()-start)};
}
