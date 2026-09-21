import {usabilityInstructions} from './design.mjs';
import { experimental_composeSpec, experimental_createEvaluator } from '@json-render/core';
import { pathToFileURL } from 'node:url';
import { readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { catalog, candidatesFor, initialState } from './catalog.mjs';
import { generateContent } from './generate.mjs';
import { validateConnections } from './actions.mjs';
import { groundOperations } from './ground.mjs';

export function loadConfig(env = process.env, file = join(homedir(), '.config', 'ratatui-json', 'config.json')) {
  let saved = {};
  try { saved = JSON.parse(readFileSync(file, 'utf8')); }
  catch (error) {
    if (error.code !== 'ENOENT') throw new Error('Cannot read ~/.config/ratatui-json/config.json. Check its JSON syntax and permissions.');
  }
  const keys = ['OPENAI_API_KEY', 'LLM_BASE_URL', 'LLM_MODEL', 'LLM_API_KEY', 'JEV_ENDPOINT', 'JEV_MODEL', 'JEV_API_KEY', 'JEV_AI_GATEWAY_API_KEY', 'AI_GATEWAY_API_KEY'];
  if (!saved || typeof saved !== 'object' || Array.isArray(saved) || keys.some(key => saved[key] !== undefined && typeof saved[key] !== 'string')) {
    throw new Error('Model configuration values must be strings.');
  }
  return { ...Object.fromEntries(keys.filter(key => saved[key] !== undefined).map(key => [key, saved[key]])), ...env };
}

export function evaluator(env = process.env) {
  if (env.JEV_ENDPOINT) {
    const endpoint = new URL(env.JEV_ENDPOINT);
    if (!['http:', 'https:'].includes(endpoint.protocol)) throw new Error('JEV_ENDPOINT must be HTTP(S).');
    return async ({ state, questions, signal }) => {
      const response = await fetch(endpoint, {
        method: 'POST', signal,
        headers: { 'Content-Type': 'application/json', ...(env.JEV_API_KEY ? { Authorization: `Bearer ${env.JEV_API_KEY}` } : {}) },
        body: JSON.stringify({ model: env.JEV_MODEL || 'jev', state: JSON.stringify(state), questions }),
      });
      if (!response.ok) throw new Error(`Jev endpoint returned HTTP ${response.status}.`);
      const result = await response.json();
      return {
        answers: Object.fromEntries(Object.keys(questions).map(key => [key, {
          choice: result.answers?.[key]?.choice, confidence: result.answers?.[key]?.confidence,
        }])),
        usage: {...result.usage, inputTokens:result.usage?.input_tokens ?? result.usage?.inputTokens},
        model: result.model ?? env.JEV_MODEL ?? "jev",
        costUsd: result.usage?.cost ?? result.cost_usd ?? null,
      };
    };
  }
  const apiKey = env.JEV_AI_GATEWAY_API_KEY || env.AI_GATEWAY_API_KEY;
  if (!apiKey) throw new Error('Set JEV_ENDPOINT or JEV_AI_GATEWAY_API_KEY to compose with Jev. The demo works offline.');
  return experimental_createEvaluator({ apiKey, model: env.JEV_MODEL || 'typesafe-ai/jev', timeoutMs: 15000 });
}

// A scripted evaluator exercises the real composer without claiming model inference.
export function demoEvaluator() {
  const choices = ['column', 'heading', 'revenue', 'cpu', 'traffic', 'services', 'name', 'save', 'status', 'finish'];
  let index = 0;
  return async ({ questions }) => ({ answers: Object.fromEntries(Object.keys(questions).map(key => [key, {
    choice: key === 'next' ? choices[index++] : Object.keys(questions[key].criteria)[0],
  }])) });
}

export async function* compose(request, evaluate = evaluator(), demo = false, generate = generateContent) {
  if (typeof request.prompt !== 'string' || !request.prompt.trim() || request.prompt.length > 4000) {
    throw new Error('Enter a prompt between 1 and 4,000 characters.');
  }
  const started = performance.now();
  if (!demo) yield { type: 'status', message: 'Designing content for your request…' };
  let content = demo
    ? { candidates: candidatesFor(request.prompt), state: request.initialSpec?.state ?? initialState }
    : await generate(request);
  const signal = AbortSignal.timeout(45000);
  if (!demo) {
    yield { type: 'status', message: 'Guidance ready. Jev is connecting operations and composing the layout…', generation: content.generation };
    content = await groundOperations(content, evaluate, request.prompt, signal);
  }
  let correction = '';
  for (let attempt = 0; attempt < 2; attempt++) {
    let retry = false;
    for await (const event of experimental_composeSpec({
      catalog, candidates: content.candidates, prompt: request.prompt,
      // Palette metadata belongs to the native renderer, outside upstream's tree schema.
      initialSpec: request.initialSpec && { root: request.initialSpec.root, elements: request.initialSpec.elements, state: request.initialSpec.state },
      initialState: content.state,
      context: { guidance: content.guidance ?? "" },
      evaluate: async input => {
        // The proposal already supplies linear order. Jev chooses membership and parents;
        // asking it to reinterpret row/column prose can scramble otherwise valid controls.
        const ordered = input.state.selected_elements?.slice(1);
        if (!request.initialSpec && ordered?.length && ordered.every(item => /^Reading position \d+:/.test(item.content))) {
          const questions = { ...input.questions }, positions = {};
          ordered.sort((a, b) => Number(a.content.match(/^Reading position (\d+):/)[1]) - Number(b.content.match(/^Reading position (\d+):/)[1]));
          ordered.forEach((item, index) => {
            const key = `order_${item.id}`, choice = String(index + 1);
            if (questions[key]?.criteria[choice]) { positions[key] = { choice }; delete questions[key]; }
          });
          const result = Object.keys(questions).length ? await evaluate({ ...input, questions }) : { answers: {} };
          return { ...result, answers: { ...result.answers, ...positions } };
        }
        return evaluate(input);
      }, strategy: demo ? 'sequential' : 'batch',
      maxElements: 48, maxSteps: 14, maxDepth: 6,
      signal,
      instructions: {
        root: `This is a native terminal. Prefer Column as root. All records are sample data. ${usabilityInstructions}`,
        next: `Assemble the supplied primitives to satisfy the request and capability contracts. Include all controls needed for requested behavior; labels cannot change a primitive operation. Keep each control cluster with its display/inputs. For a selected action, include its state readers/displays or downstream controls; do not omit their dependencies. Guidance from the content model: ${content.guidance ?? ''}. ${correction}`,
        parent: 'Group by task and action scope, without redundant nested Panels. Honor the supplied guidance and candidate group descriptions. Grid containers own the corresponding Button primitives. Put the display and grid inside the same Column or Panel. Use Row for compact metrics/inputs; keep maps and tables full width.',
      },
    })) {
      if (event.spec && content.theme) event.spec = { ...event.spec, theme: content.theme };
      if (event.type === 'complete' && event.stopReason === 'finish' && !demo) {
        try { validateConnections(Object.values(event.spec.elements)); }
        catch (error) {
          if (attempt) throw error;
          correction = `Repair the composition: ${error.message} Include the corresponding reader candidate; preserve the requested controls.`;
          yield { type: 'status', message: 'Jev is reconnecting a missing result display…' };
          retry = true;
          break;
        }
      }
      yield event.type === 'complete'
        ? { ...event, elapsedMs: Math.round(performance.now() - started), generation: content.generation, grounding: content.grounding }
        : event;
    }
    if (!retry) return;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const demo = process.argv.includes('--demo');
    const env = demo ? process.env : loadConfig();
    let request = { prompt: 'Operations overview with metrics, CPU, traffic, services and account settings' };
    if (!demo) {
      let input = '';
      for await (const chunk of process.stdin) {
        input += chunk;
        if (input.length > 1_000_000) throw new Error('Composition request is too large.');
      }
      request = JSON.parse(input);
    }
    for await (const event of compose(request, demo ? demoEvaluator() : evaluator(env), demo, request => generateContent(request, env))) {
      process.stdout.write(`${JSON.stringify(event)}\n`);
    }
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ type: 'error', message: error.message })}\n`);
    process.exitCode = 1;
  }
}
