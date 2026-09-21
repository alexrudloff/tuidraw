import { z } from 'zod';
import { resolveElementProps } from '@json-render/core';

export const statePath = z.string().regex(/^\/[A-Za-z][A-Za-z0-9_]{0,63}$/);
export const scalar = z.union([z.string().max(4096), z.number().min(-1e9).max(1e9), z.boolean()]);
export const stateValue = z.union([scalar,z.array(z.string().max(80)).max(100)]);
export const reference = z.object({ $state: statePath }).strict();
const operand = z.union([z.string().max(4096), z.number().min(-1e9).max(1e9), reference]);
export const computeParams = z.union(Object.entries({
  add: 2, subtract: 2, multiply: 2, divide: 2, randomInt: 3, append: 2, backspace: 1, evaluate: 1,
}).map(([op, arity]) => z.object({ statePath, op: z.literal(op), args: z.array(operand).length(arity) }).strict()));
export const setParams = z.object({ statePath, value: z.union([stateValue, reference]) }).strict();
export const action = z.union([
  z.object({ action: z.literal('setState'), params: setParams }).strict(),
  z.object({ action: z.literal('compute'), params: computeParams }).strict(),
]);
export const press = z.union([action, z.array(action).min(1).max(8)]);

// Check contracts without consuming randomness or evaluating incomplete editable expressions.
export function checkActions(binding, state) {
  const parsed = press.parse(binding);
  const next = { ...state };
  for (const step of Array.isArray(parsed) ? parsed : [parsed]) {
    const key = step.params.statePath.slice(1);
    if (!Object.hasOwn(next, key)) throw new Error('Action must target existing state.');
    const resolved = resolveElementProps(step.params, { stateModel: next });
    if (step.action === 'setState') next[key] = resolved.value;
    else {
      const { op, args } = resolved;
      const arity = op === 'randomInt' ? 3 : ['backspace', 'evaluate'].includes(op) ? 1 : 2;
      if (args.length !== arity) throw new Error(`${op} requires exactly ${arity} arguments.`);
      for (const [index, value] of args.entries()) {
        if (!['string', 'number'].includes(typeof value)) {
          const source = step.params.args[index]?.$state;
          throw new Error(`${op} argument ${index + 1}${source ? ` (${source})` : ''} must resolve to a string or number; got ${value === null ? 'null' : typeof value}. Correct the referenced state or choose a compatible operand.`);
        }
      }
      if (!['string', 'number'].includes(typeof next[key])) throw new Error(`Compute target ${step.params.statePath} must be string or numeric state; got ${typeof next[key]}.`);
      // Compute preserves the target type. Its value depends on future input/randomness;
      // a fabricated zero would incorrectly reject a slider whose minimum is above zero.
    }
  }
  return next;
}

export function actionSummary(binding) {
  return (Array.isArray(binding) ? binding : [binding]).map(step =>
    `${step.action === 'compute' ? `${step.params.op}(${step.params.args.map(arg => typeof arg === 'object' ? arg.$state : JSON.stringify(arg)).join(',')})` : 'set'} -> ${step.params.statePath}`
  ).join(' then ');
}

export function bindingSummary(props) {
  const paths = new Set();
  function walk(value) {
    if (!value || typeof value !== 'object') return;
    if (value.$state || value.$bindState) paths.add(value.$state ?? value.$bindState);
    Object.values(value).forEach(walk);
  }
  walk(props);
  return paths.size ? `; reads ${[...paths].join(',')}` : '';
}

export function validateConnections(elements) {
  const displayed = new Set();
  const buttons = [];
  const flows = new Map();
  function references(value, into) {
    if (!value || typeof value !== 'object') return;
    if (value.$state) into.add(value.$state);
    if (value.$bindState) into.add(value.$bindState);
    for (const child of Object.values(value)) references(child, into);
  }
  for (const element of elements) {
    references(element.props, displayed);
    references(element.visible, displayed);
    if (!element.on?.press) continue;
    const sequence = Array.isArray(element.on.press) ? element.on.press : [element.on.press];
    if (sequence.every(step => step.action === 'setState' && step.params.value?.$state === step.params.statePath)) {
      throw new Error(`Button ${element.props.label} only assigns state to itself and does nothing. Bind the intended operation and its operands.`);
    }
    const writes = new Set();
    buttons.push({ label: element.props.label, writes });
    for (const step of sequence) {
      writes.add(step.params.statePath);
      const read = new Set();
      references(step.params, read);
      for (const source of read) {
        if (!flows.has(source)) flows.set(source, new Set());
        flows.get(source).add(step.params.statePath);
      }
      if (step.action === 'compute' && step.params.args.some(arg => typeof arg === 'string' && /\$state|\$bindState/.test(arg))) throw new Error('Action arguments must use state reference objects, not placeholders inside strings.');
    }
  }
  for (const button of buttons) {
    const pending = [...button.writes], seen = new Set();
    let connected = false;
    while (pending.length) {
      const key = pending.pop();
      if (seen.has(key)) continue;
      seen.add(key);
      if (displayed.has(key)) { connected = true; break; }
      pending.push(...(flows.get(key) ?? []));
    }
    if (!connected) throw new Error(`Button ${button.label} writes ${[...button.writes].join(',')}, but has no path to a display or control. Bind a display or control to its result.`);
  }
}

// Assign stable accelerators in code; the model can request an explicit mnemonic.
// Reserve all explicit keys before filling gaps, including currently hidden/disabled actions.
export function assignButtonHotkeys(elements) {
  const buttons=elements.filter(e=>e.type==='Button');
  const used=new Set();
  for(const button of buttons) {
    const key=button.props.hotkey;
    if(key==null) continue;
    if(typeof key!=='string' || !/^(((Ctrl\+)?Alt\+)[A-Z0-9]|((Ctrl\+)?Alt\+)?F([1-9]|1[0-9]|2[0-4]))$/.test(key)) throw new Error('Invalid Button.hotkey; use Alt+letter/digit or F1–F24.');
    if(used.has(key)) throw new Error(`Duplicate button hotkey ${key}. Choose a unique accelerator.`);
    used.add(key);
  }
  const letters=[...'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'];
  const functions=Array.from({length:24},(_,i)=>`F${i+1}`);
  const pool=[...letters.map(c=>`Alt+${c}`),...functions,...functions.map(k=>`Alt+${k}`),...[...letters,...functions].map(k=>`Ctrl+Alt+${k}`)];
  for(const button of buttons) {
    if(button.props.hotkey!=null) continue;
    const mnemonic=[...String(button.props.label).toUpperCase()].filter(c=>letters.includes(c)).map(c=>`Alt+${c}`);
    const key=[...mnemonic,...pool].find(k=>!used.has(k));
    if(!key) throw new Error('No free button hotkeys remain.');
    button.props.hotkey=key;
    used.add(key);
  }
}
