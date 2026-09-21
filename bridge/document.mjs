import {isDeepStrictEqual} from 'node:util';
import {catalog} from './catalog.mjs';

// Apply a bounded edit proposal to a copy. Stable IDs and untouched nodes survive every turn.
export function applyDraft(draft, initial, allowUnchanged = false) {
  const spec = initial ? structuredClone(initial) : {
    root: 'root', elements: {root:{type:'Column',props:{},children:[]}}, state:{},
  };
  spec.state = {...draft.state, ...spec.state};
  if (draft.theme) spec.theme = draft.theme;
  for (const {key,value} of draft.stateUpdates ?? []) {
    if (['__proto__','constructor','prototype'].includes(key)) throw new Error('Unsafe state key.');
    spec.state[key]=value;
  }
  for (const {id,description,children,...element} of draft.widgets) {
    if (['__proto__','constructor','prototype'].includes(id)) throw new Error('Unsafe widget ID.');
    const previous=spec.elements[id];
    if(element.type==='Button' && element.props.hotkey==null && previous?.props.hotkey) element.props={...element.props,hotkey:previous.props.hotkey};
    spec.elements[id]={...element,children:previous?.children ?? [],...(previous?.visible!==undefined?{visible:previous.visible}:{})};
    if (!previous) spec.elements[spec.root].children.push(id);
  }
  // Explicit child lists express ownership once; no matching placement list is needed.
  const groups=draft.widgets.filter(widget=>Array.isArray(widget.children));
  const owner=new Map();
  for(const group of groups) {
    if(!['Column','Row','Grid','Panel'].includes(group.type)) throw new Error(`${group.id} is not a container.`);
    for(const child of group.children) {
      if(child===spec.root || !Object.hasOwn(spec.elements,child)) throw new Error(`Container ${group.id}: invalid child ${child}.`);
      if(owner.has(child)) throw new Error(`Widget ${child} has multiple declared parents: ${owner.get(child)}, ${group.id}.`);
      owner.set(child,group.id);
    }
  }
  for(const node of Object.values(spec.elements)) node.children=(node.children??[]).filter(child=>!owner.has(child));
  for(const group of groups) spec.elements[group.id].children=[...group.children];
  for (const update of draft.updates ?? []) {
    const {id,key}=update;
    if (!Object.hasOwn(spec.elements,id)) throw new Error(`Cannot update missing widget ${id}.`);
    if (['__proto__','constructor','prototype'].includes(key)) throw new Error('Unsafe property key.');
    let value=update.value;
    if(Object.hasOwn(update,'json')) {
      if(Object.hasOwn(update,'value') || typeof update.json!=='string' || update.json.length>16000) throw new Error('Use either value or json in a property update.');
      try { value=JSON.parse(update.json); }
      catch { throw new Error(`Property ${id}.${key}: json must contain valid JSON.`); }
      // Encoded values still pass the full widget/state/action validation after application.
      const inspect=(v,depth=0)=>{
        if(depth>12) throw new Error('Property JSON exceeds depth 12.');
        if(v && typeof v==='object') for(const [k,child] of Object.entries(v)) {
          if(['__proto__','constructor','prototype'].includes(k)) throw new Error('Unsafe property JSON key.');
          inspect(child,depth+1);
        }
      };
      inspect(value);
    }
    spec.elements[id].props[key]=value;
  }
  const moves=draft.placements ?? [];
  for(const {id,parent} of moves) {
    if(owner.has(id) || groups.some(group=>group.id===(parent??spec.root))) throw new Error(`Placement ${id} conflicts with an explicit children list. Describe each relationship once.`);
  }
  if (new Set(moves.map(m=>m.id)).size!==moves.length) throw new Error('Each widget may have only one placement.');
  for (const {id,parent} of moves) {
    if(id===spec.root || !Object.hasOwn(spec.elements,id)) throw new Error(`Invalid moved widget ${id}.`);
    if(!Object.hasOwn(spec.elements,parent ?? spec.root)) throw new Error(`Missing parent ${parent}.`);
    for (const node of Object.values(spec.elements)) node.children=(node.children??[]).filter(child=>child!==id);
  }
  for (const {id,parent,index} of [...moves].sort((a,b)=>a.index-b.index)) {
    spec.elements[parent ?? spec.root].children.splice(index,0,id);
  }
  const removed=new Set();
  function remove(id) {
    if(id===spec.root) throw new Error('Cannot remove the root.');
    if(removed.has(id)) return;
    if(!Object.hasOwn(spec.elements,id)) throw new Error(`Cannot remove missing widget ${id}.`);
    removed.add(id);
    for(const child of spec.elements[id].children??[]) remove(child);
  }
  for(const id of draft.remove??[]) remove(id);
  for(const id of removed) delete spec.elements[id];
  for(const node of Object.values(spec.elements)) node.children=(node.children??[]).filter(id=>!removed.has(id));
  validateTree(spec);
  // Apply the requested order to sibling groups, preserving their nested contents.
  for(const check of draft.layoutChecks??[]) {
    const {node,children}=layoutGroup(spec,check);
    const positions=children.map(id=>node.children.indexOf(id));
    positions.sort((a,b)=>a-b).forEach((position,index)=>{node.children[position]=children[index];});
  }
  validateLayout(spec,draft.layoutChecks??[]);
  const empty=Object.entries(spec.elements).filter(([id,node])=>id!==spec.root && ['Column','Row','Grid','Panel'].includes(node.type) && !node.children.length).map(([id])=>id);
  if(empty.length) throw new Error(`Empty containers: ${empty.join(', ')}. Put the intended content inside their children lists, or remove unused containers. Siblings are not inside a panel just because they follow it.`);
  const canonical = s=>({...s,elements:Object.fromEntries(Object.entries(s.elements).map(([id,node])=>{
    // Defaults introduced by structured output must not turn a no-op into a revision.
    const defaults=Object.fromEntries(Object.entries(catalog.data.components[node.type]?.props.shape??{}).flatMap(([key,schema])=>{
      if(Object.hasOwn(node.props,key)) return [];
      const parsed=schema.safeParse(undefined);
      return parsed.success && parsed.data!==undefined ? [[key,parsed.data]] : [];
    }));
    if(!['Column','Row','Grid','Panel'].includes(node.type)) defaults.grow=0;
    return [id,{...node,props:{...defaults,...node.props},children:node.children??[],on:node.on??{},visible:node.visible??true}];
  }))});
  if(initial && !allowUnchanged && isDeepStrictEqual(canonical(spec),canonical(initial))) throw new Error('The proposal makes no change to the current interface. Apply the requested edit instead of only describing it.');
  return spec;
}

// Project a nested reading order onto real sibling groups. A panel and its contents
// are one outer group; they are not separate siblings just because all IDs are listed.
function layoutGroup(spec,{container,children}) {
  const parents=new Map(Object.entries(spec.elements).flatMap(([id,node])=>(node.children??[]).map(child=>[child,id])));
  if(new Set(children).size!==children.length) throw new Error(`Layout ${container}: duplicate children.`);
  const node=spec.elements[container];
  if(!node) throw new Error(`Layout ${container}: missing container.`);
  const branches=children.map(id=>{
    let branch=id;
    while(parents.has(branch) && parents.get(branch)!==container) branch=parents.get(branch);
    if(parents.get(branch)!==container) throw new Error(`Layout ${container}: ${id} is not a descendant. Actual direct children: ${(node.children??[]).join(', ')}. Actual parent of ${id}: ${parents.get(id)??'missing'}.`);
    return branch;
  });
  const groups=branches.filter((id,i)=>i===0||id!==branches[i-1]);
  if(new Set(groups).size!==groups.length) throw new Error(`Layout ${container}: cannot interleave descendants of separate groups.`);
  if(groups.length===1) {
    const nested=children.filter(id=>id!==groups[0]);
    if(nested.length>1) return layoutGroup(spec,{container:groups[0],children:nested});
  }
  return {container,node,children:groups};
}

// Axes still come from actual container types, never labels or success prose.
export function validateLayout(spec, checks) {
  for(const check of checks) {
    const {container,node,children}=layoutGroup(spec,check);
    const {axis}=check;
    const positions=children.map(id=>node.children.indexOf(id));
    if(positions.some((p,i)=>i>0 && p<=positions[i-1])) throw new Error(`Layout ${container}: expected ordered direct children ${children.join(', ')}. Actual: ${node.children.join(', ')}.`);
    if(children.length<2) continue;
    const matches=axis==='horizontal' ? node.type==='Row' : ['Column','Panel'].includes(node.type);
    const grid=node.type==='Grid' && Number.isInteger(node.props.columns) && node.props.columns>0;
    const sameGridLine=grid && new Set(positions.map(p=>axis==='horizontal' ? Math.floor(p/node.props.columns) : p%node.props.columns)).size===1;
    if(!matches && !sameGridLine) throw new Error(`Layout ${container}: requested ${axis} arrangement of ${children.join(', ')}, but ${node.type} does not arrange them that way. Use a Row for side-by-side sections, or a Column for stacked sections, and move these children into it.`);
  }
}

export function validateTree(spec) {
  const seen=new Set();
  function visit(id,depth) {
    if(depth>8 || seen.has(id)) throw new Error(`Cycle, shared child or excessive nesting at ${id}.`);
    const node=Object.hasOwn(spec.elements,id) && spec.elements[id];
    if(!node) throw new Error(`Missing widget ${id}.`);
    seen.add(id);
    if(node.children?.length && !['Column','Row','Grid','Panel'].includes(node.type)) throw new Error(`${id} is not a container.`);
    for(const child of node.children??[]) visit(child,depth+1);
  }
  visit(spec.root,0);
  if(seen.size!==Object.keys(spec.elements).length || seen.size>128) throw new Error('Unreachable widgets or more than 128 widgets.');
}
