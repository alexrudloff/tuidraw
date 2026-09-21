import test from 'node:test';
import assert from 'node:assert/strict';
import {assignButtonHotkeys} from './actions.mjs';
import {applyDraft} from './document.mjs';
import {prepareDocument} from './generate.mjs';

test('button accelerators are unique, bounded and stable across refinement',()=>{
  const buttons=Array.from({length:127},()=>({type:'Button',props:{label:'Action'}}));
  buttons[126].props.hotkey='Alt+A';
  assignButtonHotkeys(buttons);
  assert.equal(buttons[126].props.hotkey,'Alt+A');
  assert.equal(new Set(buttons.map(e=>e.props.hotkey)).size,127);
  const before=structuredClone(buttons);
  assignButtonHotkeys(buttons);
  assert.deepEqual(buttons,before);
  assert.throws(()=>assignButtonHotkeys([{type:'Button',props:{hotkey:'Ctrl+C'}}]),/Invalid/);
  assert.throws(()=>assignButtonHotkeys([buttons[0],buttons[0]]),/Duplicate/);
  const initial={root:'root',state:{status:''},elements:{
    root:{type:'Column',props:{},children:['run','result']},
    run:{type:'Button',props:{label:'Run',hotkey:'Alt+R'},on:{press:{action:'setState',params:{statePath:'/status',value:'Ran'}}}},
    result:{type:'Text',props:{text:{$state:'/status'}}}
  }};
  const draft={state:{},widgets:[{id:'run',description:'Rename',...structuredClone(initial.elements.run),props:{label:'Execute'}}],placements:[],updates:[],remove:[],stateUpdates:[],layoutChecks:[]};
  const changed=applyDraft(draft,initial);
  prepareDocument(changed,initial.state);
  assert.equal(changed.elements.run.props.hotkey,'Alt+R');
  assert.equal(initial.elements.run.props.label,'Run');
  delete changed.elements.run.props.hotkey;
  prepareDocument(changed,initial.state);
  assert.equal(changed.elements.run.props.hotkey,'Alt+E');
});
