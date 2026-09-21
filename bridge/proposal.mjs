// Retain invalid proposals as data; only the normal compiler can accept their repairs.
export function patchProposal(proposal, patches) {
  const next=structuredClone(proposal);
  for(const patch of patches) {
    if(!patch.path.startsWith('/') || /~(?![01])/u.test(patch.path))throw new Error('Repair paths must be non-root JSON pointers.');
    const keys=patch.path.slice(1).split('/').map(k=>k.replace(/~1/g,'/').replace(/~0/g,'~'));
    if(keys.length>16 || keys.some(k=>['__proto__','constructor','prototype'].includes(k)))throw new Error('Unsafe repair path.');
    let parent=next;
    for(const key of keys.slice(0,-1)) {
      if(!parent || typeof parent!=='object' || !Object.hasOwn(parent,key))throw new Error(`Missing repair path ${patch.path}.`);
      parent=parent[key];
    }
    const key=keys.at(-1);
    if(!parent || typeof parent!=='object')throw new Error(`Invalid repair parent ${patch.path}.`);
    if(Array.isArray(parent)) {
      const index=key==='-'?parent.length:Number(key);
      if((key!=='-'&&!/^(0|[1-9][0-9]*)$/.test(key)) || !Number.isSafeInteger(index) || index<0 || index>parent.length || (patch.op!=='add'&&index===parent.length))throw new Error('Invalid repair array index.');
      if(patch.op==='add')parent.splice(index,0,structuredClone(patch.value));
      else if(patch.op==='remove')parent.splice(index,1);
      else parent[index]=structuredClone(patch.value);
    } else {
      // Optional widget properties may be absent in a compact proposal.
      if(patch.op==='remove'&&!Object.hasOwn(parent,key))throw new Error(`Missing repair path ${patch.path}.`);
      if(patch.op==='remove')delete parent[key];
      else parent[key]=structuredClone(patch.value);
    }
  }
  if(JSON.stringify(next).length>160000)throw new Error('Repaired proposal exceeds size limit.');
  return next;
}

export function normalizeWidgetProps(raw, catalog) {
  const next=structuredClone(raw);
  for(const widget of next.widgets??[]) {
    const shape=catalog.data.components[widget?.type]?.props.shape;
    if(!shape || !widget.props || typeof widget.props!=='object')continue;
    for(const [key,value] of Object.entries(widget)) {
      if(Object.hasOwn(shape,key)&&!Object.hasOwn(widget.props,key)) {
        widget.props[key]=value;delete widget[key];
      }
    }
  }
  return next;
}

export function compactProposal(raw) {
  const next=structuredClone(raw);
  for(const widget of next.widgets??[]) if(widget&&typeof widget==='object'&&widget.description===undefined)widget.description=widget.id;
  // Keep one public value format; reuse the legacy compiler's bounded JSON validation.
  for(const update of next.updates??[]) if(update&&typeof update.value==='object'&&update.value!==null&&!Object.hasOwn(update,'json')) {
    update.json=JSON.stringify(update.value);delete update.value;
  }
  delete next.final;
  return next;
}
