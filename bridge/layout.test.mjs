import {test} from 'node:test';
import assert from 'node:assert/strict';
import {checkGeometry,measureLayout} from './layout.mjs';

const report={viewport:[100,36],nodes:{
  root:{parent:null,bounds:[0,0,100,36],inner:[1,1,98,34]},
  rail:{parent:'root',bounds:[1,1,24,30],inner:[2,2,22,28]},
  list:{parent:'rail',bounds:[2,2,20,8],inner:[2,2,20,8]},
  main:{parent:'root',bounds:[26,1,73,30],inner:[26,1,73,30]},
  input:{parent:'root',bounds:[1,32,98,3],inner:[1,32,98,3]},
}};
test('geometry checks measure rendered edges, not allocated wrapper slots',()=>{
  const gap={kind:'gap',from:'rail',to:'main',axis:'horizontal',min:1,max:1};
  const size={kind:'size',id:'rail',axis:'horizontal',min:24,max:24};
  const edge={kind:'edge',id:'input',parent:'root',edge:'bottom',inset:0};
  assert.doesNotThrow(()=>checkGeometry([gap,size,edge],report));
  assert.throws(()=>checkGeometry([{...gap,from:'list'}],report),/gap 4/);
  assert.throws(()=>checkGeometry([{...gap,to:'input'}],report),/failed/);
  assert.throws(()=>checkGeometry([{...gap,to:'missing'}],report),/missing or hidden/);
  assert.throws(()=>checkGeometry([{...size,max:20}],report),/minimum/);
  assert.throws(()=>checkGeometry([{...size,min:25,max:25}],report),/size 24/);
  assert.throws(()=>checkGeometry([{...edge,parent:'rail'}],report),/not an ancestor/);
  assert.throws(()=>checkGeometry([{...edge,parent:null}],report),/inset 1/);
  assert.doesNotThrow(()=>checkGeometry([{...gap,min:0,max:0,minViewportWidth:120}],report));
  const empty=structuredClone(report);empty.nodes.main.bounds[2]=0;
  assert.throws(()=>checkGeometry([gap],empty),/no visible area/);
  assert.throws(()=>measureLayout({},[100,36],'/definitely-not-a-renderer'),/measurement failed/);
});
