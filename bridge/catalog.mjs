import { defineCatalog, defineSchema } from '@json-render/core';
import { z } from 'zod';
import { extendedComponents } from './extended.mjs';
import { computeParams, setParams } from './actions.mjs';

const schema = defineSchema(s => ({
  spec: s.object({
    root: s.string(),
    elements: s.record(s.object({
      type: s.ref('catalog.components'), props: s.propsOf('catalog.components'),
      children: s.array(s.string()),
    })),
  }),
  catalog: s.object({
    components: s.map({ props: s.zod(), slots: s.array(s.string()), events: s.array(s.string()) }),
    actions: s.map({ params: s.zod() }),
  }),
}));

const text = z.string().max(4096);
const display = z.union([text, z.number().finite(), z.boolean()]);
const title = z.object({ title: text });
const valueColor=z.union([z.enum(['foreground','muted','primary','success','warning','danger','green','yellow','red']),z.string().regex(/^#[0-9a-fA-F]{6}$/)]);
const valueColors={color:valueColor.nullable().default(null),thresholds:z.array(z.object({min:z.number().finite(),color:valueColor}).strict()).max(16).default([])};
const intent = z.enum(['neutral', 'primary', 'danger', 'success', 'warning']);
export const catalog = defineCatalog(schema, {
  components: Object.fromEntries(Object.entries({
    ...extendedComponents,
    Column: { props: z.object({gap:z.number().int().min(0).max(4).nullable().default(null)}), slots: ['default'] },
    Row: { props: z.object({gap:z.number().int().min(0).max(4).nullable().default(null),collapseBelow:z.number().int().min(0).max(240).default(0)}), slots: ['default'] },
    Grid: { props: z.object({ columns: z.number().int().min(1).max(8),gap:z.number().int().min(0).max(4).nullable().default(null),minCellWidth:z.number().int().min(0).max(120).default(0) }), slots: ['default'] },
    Panel: { props: title.extend({surface:z.boolean().default(false),border:z.enum(['plain','rounded','heavy','double','none']).nullable().default(null),padding:z.number().int().min(0).max(4).nullable().default(null),gap:z.number().int().min(0).max(4).nullable().default(null),titleAlign:z.enum(['left','center','right']).default('left')}), slots: ['default'] },
    Text: { props: z.object({ text: display, muted: z.boolean().optional() }) },
    Metric: { props: z.object({ label: text, value: display }) },
    Gauge: { props: title.extend({ value: z.number().min(0).max(100) }) },
    Sparkline: { props: title.extend({ data: z.array(z.number().int().min(0).max(1e9)).max(128), dense: z.boolean().optional() }) },
    Table: { props: title.extend({ headers: z.array(text).min(1).max(12), rows: z.array(z.array(text).max(12)).max(200),columnStyles:z.array(z.object({width:z.number().int().min(0).max(240).default(0),align:z.enum(['left','center','right']).default('left'),color:z.enum(['foreground','muted','primary','success','warning','danger']).default('foreground')}).strict()).max(12).default([]),columnGap:z.number().int().min(0).max(4).nullable().default(null),padding:z.number().int().min(0).max(4).nullable().default(null),showHeader:z.boolean().default(true),border:z.enum(['plain','rounded','heavy','double','none']).nullable().default(null) }) },
    List: { props: title.extend({ items: z.array(text).max(200) }) },
    Input: { props: z.object({ label: text, value: text }) },
    Button: { props: z.object({ label: text, hotkey:z.string().regex(/^(((Ctrl\+)?Alt\+)[A-Z0-9]|((Ctrl\+)?Alt\+)?F([1-9]|1[0-9]|2[0-4]))$/).nullable().default(null), variant:z.enum(['solid','plain']).default('solid'), intent: intent.optional(), disabled: z.boolean().optional() }), events: ['press'] },
    Badge: { props: z.object({ label: text, intent }) },
    Switch: { props: z.object({ label: text, checked: z.boolean(), disabled: z.boolean() }) },
    Keypad: { props: z.object({ title: text, value: text.max(128), mode: z.enum(['calculator', 'numeric']) }) },
  }).map(([name, definition]) => [name, {...definition, props: definition.props.extend({
    ...(['Metric','Gauge','Sparkline','BarChart'].includes(name)?valueColors:{}),
    width:z.union([z.number().int().min(1).max(240),z.enum(['content','fill'])]).nullable().default(null),
    maxWidth:z.number().int().min(0).max(240).default(0),
    widthPercent:z.number().int().min(1).max(100).default(100),
    horizontalAlign:z.enum(['left','center','right']).default('left'),
    grow: z.number().int().min(0).max(16).optional().describe("Vertical extra-space weight; 0 keeps intrinsic height, 1 expands. Omitted on containers inherits their largest visible child weight."),
  })}])),
  actions: { setState: { params: setParams }, compute: { params: computeParams } },
});

export const initialState = {
  name: 'Ada', status: 'Ready', revenue: '$48,290', orders: '1,284', uptime: '99.98%',
  cpu: 42, traffic: [12, 18, 15, 24, 20, 35, 28, 42, 38, 54, 47, 62],
  services: [['API', 'Healthy', '24 ms'], ['Worker', 'Healthy', '81 ms'], ['Database', 'Healthy', '6 ms']],
  tasks: ['Review deployment', 'Check error budget', 'Ship the terminal renderer'],
};
const state = path => ({ $state: `/${path}` });
const candidate = (id, description, type, props, extra = {}) => ({
  id, description, root: false, element: { type, props }, ...extra,
});

export function candidatesFor(prompt) {
  const candidates = [
    candidate('column', 'Vertical layout. Use as the root for compact readable terminal screens.', 'Column', {}, { root: true, maxUses: 3 }),
    candidate('row', 'Horizontal layout grouping related metrics side by side.', 'Row', {}, { maxUses: 2 }),
    candidate('panel', 'Bordered overview panel that contains other widgets.', 'Panel', { title: 'Overview' }, { root: true }),
    candidate('heading', 'Heading: Operations overview', 'Text', { text: 'OPERATIONS OVERVIEW' }),
    candidate('revenue', 'Revenue metric from sample sales data.', 'Metric', { label: 'Revenue', value: state('revenue') }),
    candidate('orders', 'Orders metric from sample sales data.', 'Metric', { label: 'Orders', value: state('orders') }),
    candidate('uptime', 'Service uptime metric.', 'Metric', { label: 'Uptime', value: state('uptime') }),
    candidate('cpu', 'CPU utilization percentage gauge.', 'Gauge', { title: 'CPU utilization', value: state('cpu') }),
    candidate('traffic', 'Traffic trend sparkline.', 'Sparkline', { title: 'Traffic / last 12 hours', data: state('traffic') }),
    candidate('services', 'Service health table: API, Worker and Database with status and latency.', 'Table', { title: 'Services', headers: ['Service', 'Status', 'Latency'], rows: state('services') }),
    candidate('tasks', 'Task list with three pending work items.', 'List', { title: 'Tasks', items: state('tasks') }),
    candidate('name', 'Editable display name field for an account settings form.', 'Input', { label: 'Display name', value: { $bindState: '/name' } }),
    candidate('status', 'Local action status message, initially Ready.', 'Text', { text: state('status'), muted: true }),
    candidate('save', 'Save button, updates local status to Saved locally. Does not persist to a server.', 'Button', { label: 'Save changes' }, {
      element: { type: 'Button', props: { label: 'Save changes' }, on: { press: { action: 'setState', params: { statePath: '/status', value: 'Saved locally' } } } },
    }),
  ];
  const heading = prompt.match(/"([^"\n]{1,100})"/u)?.[1];
  if (heading) candidates.push(candidate('custom-heading', `Requested heading: ${heading}`, 'Text', { text: heading }));
  return candidates;
}
