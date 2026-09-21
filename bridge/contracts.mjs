import { z } from 'zod';
import { catalog } from './catalog.mjs';

const SHARED = ['width', 'maxWidth', 'widthPercent', 'horizontalAlign', 'grow'];
const components = catalog.data.components;

// Build exact JSON schemas from the catalog's Zod props, lifting the sizing
// fields shared by every widget into a single map and stripping $schema markers.
export function widgetContracts(types) {
  const names = Array.isArray(types) ? types : [types];
  const sharedProps = {};
  const widgets = {};
  let seeded = false;
  for (const name of names) {
    if (!Object.hasOwn(components, name)) throw new Error(`Unknown widget type: ${name}`);
    const { $schema, properties, required, ...extra } = z.toJSONSchema(components[name].props.strict(), { io: 'input' });
    if (!seeded) { for (const k of SHARED) if (properties[k]) sharedProps[k] = properties[k]; seeded = true; }
    const own = { ...properties };
    for (const k of SHARED) delete own[k];
    const req = (required ?? []).filter((r) => !SHARED.includes(r));
    widgets[name] = { ...extra, properties: own, ...(req.length ? { required: req } : {}) };
  }
  return {
    sharedProps,
    widgets,
    bindings: [
      'props describe resolved values',
      "editable controls bind {'$bindState':'/key'}",
      "displays read {'$state':'/key'}",
      'resolved types must match',
      'Button.on.press belongs at widget level',
    ],
  };
}
