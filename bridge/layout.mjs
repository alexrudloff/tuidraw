import {spawnSync} from 'node:child_process';
import {z} from 'zod';

const id=z.string().regex(/^[a-z][a-z0-9_-]{0,63}$/);
const axis=z.enum(['horizontal','vertical']);
const cells=z.number().int().min(0).max(4096);
const responsive={minViewportWidth:z.number().int().min(0).max(500).default(0)};
export const geometryChecks=z.array(z.union([
  z.object({kind:z.literal('gap'),from:id,to:id,axis,min:cells,max:cells,...responsive}).strict(),
  z.object({kind:z.literal('size'),id,axis,min:cells,max:cells,...responsive}).strict(),
  z.object({kind:z.literal('edge'),id,parent:id.nullable(),edge:z.enum(['left','right','top','bottom']),inset:cells,...responsive}).strict(),
])).max(16).default([]);

export function measureLayout(spec,viewport,executable) {
  const result=spawnSync(executable,['--measure-layout'],{
    input:JSON.stringify({spec,viewport}),encoding:'utf8',timeout:10000,maxBuffer:1_000_000,
  });
  if(result.error) throw new Error(`Native layout measurement failed: ${result.error.message}`);
  if(result.status!==0) throw new Error(`Native layout validation failed: ${result.stderr.trim().slice(0,2000)}`);
  return JSON.parse(result.stdout);
}

export function checkGeometry(checks,report) {
  const nodes=report.nodes;
  const box=id=>{
    if(!Object.hasOwn(nodes,id)) throw new Error(`Geometry: widget ${id} is missing or hidden.`);
    const bounds=nodes[id].bounds;
    if(bounds[2]===0 || bounds[3]===0) throw new Error(`Geometry: widget ${id} has no visible area: ${JSON.stringify(bounds)}.`);
    return bounds;
  };
  const failures=[];
  for(const check of geometryChecks.parse(checks)) {
    if('min' in check && check.min>check.max) throw new Error('Geometry minimum must not exceed maximum.');
    // A declared wide-screen relationship does not forbid the deliberate stacked mobile layout.
    if(report.viewport[0]<check.minViewportWidth) continue;
    const axis=check.axis==='vertical'?1:0;
    if(check.kind==='gap') {
      const a=box(check.from),b=box(check.to);
      const gap=b[axis]-a[axis]-a[axis+2];
      const cross=1-axis;
      const overlap=Math.min(a[cross]+a[cross+2],b[cross]+b[cross+2])-Math.max(a[cross],b[cross]);
      if(gap<check.min || gap>check.max || overlap<=0) failures.push(`${check.from} → ${check.to}: ${check.axis} gap ${gap}, expected ${check.min}..${check.max} with overlapping cross-axis. Bounds ${JSON.stringify(a)} → ${JSON.stringify(b)}.`);
    } else if(check.kind==='size') {
      const actual=box(check.id)[axis+2];
      if(actual<check.min || actual>check.max) failures.push(`${check.id}: ${check.axis} size ${actual}, expected ${check.min}..${check.max}.`);
    } else {
      const child=box(check.id);
      let parent=[0,0,...report.viewport];
      if(check.parent!==null) {
        box(check.parent);
        let ancestor=nodes[check.id].parent;
        while(ancestor!==null && ancestor!==check.parent) ancestor=nodes[ancestor]?.parent??null;
        if(ancestor!==check.parent) throw new Error(`Geometry: ${check.parent} is not an ancestor of ${check.id}.`);
        parent=nodes[check.parent].inner;
      }
      const i=['top','bottom'].includes(check.edge)?1:0;
      const trailing=['right','bottom'].includes(check.edge);
      const actual=trailing ? parent[i]+parent[i+2]-child[i]-child[i+2] : child[i]-parent[i];
      if(actual!==check.inset) failures.push(`${check.id}: ${check.edge} inset ${actual} from ${check.parent??'viewport'}, expected ${check.inset}. Bounds ${JSON.stringify(child)} within ${JSON.stringify(parent)}.`);
    }
  }
  if(failures.length) throw new Error(`Rendered layout at ${report.viewport.join('×')} failed: ${failures.slice(0,5).join(' ')}`);
}
