import { actionSummary, bindingSummary, checkActions, validateConnections } from './actions.mjs';

// Resolve unbound numeric operands as whole argument lists. Explicit state references
// remain fixed: choosing every operand independently can break an already valid program.
export async function groundOperations(content, evaluate, prompt, signal, eligible=null) {
  const next = structuredClone(content);
  const controls = new Map();
  for (const { element } of next.candidates) {
    const ref = element.props.value?.$bindState;
    const value = ref && next.state[ref.slice(1)];
    if (ref && ['Input', 'Select', 'Slider'].includes(element.type) &&
        ['number','string'].includes(typeof value) && String(value).trim() && Number.isFinite(Number(value))) {
      controls.set(ref, `${element.type} "${element.props.label ?? element.props.title}"`);
    }
  }
  const questions = {}, pending = [];
  for (const candidate of next.candidates) {
    if(eligible && !eligible.has(candidate.id)) continue;
    const binding = candidate.element.on?.press;
    if (!binding) continue;
    const sequence = Array.isArray(binding) ? binding : [binding];
    for (const [step, action] of sequence.entries()) {
      if (action.action !== 'compute') continue;
      const { op, args, statePath } = action.params;
      const variants = [args];
      if (op === 'randomInt') {
        const max = typeof args[1] === 'object' ? [args[1]] : [args[1], ...[...controls.keys()].map($state => ({$state}))];
        const count = typeof args[2] === 'object' ? [args[2]] : [args[2], ...[...controls.keys()].map($state => ({$state}))];
        for (const upper of max) for (const samples of count) variants.push([args[0],upper,samples]);
      } else if (['add','subtract','multiply','divide'].includes(op) && args.every(arg => typeof arg !== 'object')) {
        for (const delta of args) variants.push([{$state:statePath},delta]);
      }
      const choices = new Map([...new Map(variants.map(args => [JSON.stringify(args),args])).values()].slice(0,64).map((args,i)=>[i ? `bind_${i}` : 'keep',args]));
      if (choices.size === 1) continue;
      const roles = op === 'randomInt' ? ['minimum','maximum','number of independent samples to sum'] : ['left operand','right operand'];
      const describe = value => typeof value === 'object' ? `${controls.get(value.$state) ?? 'current state'} ${value.$state}` : `constant ${JSON.stringify(value)}`;
      const key = `binding_${pending.length}`;
      questions[key] = {
        type:'choice',
        instructions:`Connect the complete ${op} argument list for button "${candidate.element.props.label}", step ${step+1}, writing ${statePath}. Prefer actual adjustable controls when the request calls for adjustable parameters. Keep fixed constants for fixed quantities. A counter increment reads its current value; assigning a fixed sum is not an increment. Preserve the operation and explicit state bindings. Full sequence: ${actionSummary(binding)}.`,
        criteria:Object.fromEntries([...choices].map(([key,args])=>[key,args.map((arg,i)=>`${roles[i]} = ${describe(arg)}`).join('; ')])),
      };
      pending.push({key,params:action.params,choices});
    }
  }
  if (!pending.length) return next;
  const started = performance.now();
  const result = await evaluate({signal,state:{user_request:prompt,guidance:next.guidance,phase:'connect primitive operations'},questions});
  for (const {key,params,choices} of pending) {
    const choice=result.answers?.[key]?.choice;
    if(!choices.has(choice)) throw new Error('Jev returned an invalid operation operand.');
    params.args=structuredClone(choices.get(choice));
  }
  for (const candidate of next.candidates) {
    const {element}=candidate;
    if(!element.on?.press) continue;
    checkActions(element.on.press,next.state);
    candidate.description=`${candidate.description.split(' [')[0]} [${element.type}${bindingSummary(element.props)}; ${actionSummary(element.on.press)}]`;
  }
  validateConnections(next.candidates.map(candidate=>candidate.element));
  next.grounding={elapsedMs:Math.round(performance.now()-started),inputTokens:result.usage?.inputTokens??null,decisions:pending.length};
  return next;
}
