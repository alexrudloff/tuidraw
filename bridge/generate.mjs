import { resolveElementProps } from '@json-render/core';
import { z } from 'zod';
import { catalog } from './catalog.mjs';
import { extendedInstructions } from './extended.mjs';
import { press, scalar, stateValue, checkActions, actionSummary, bindingSummary, validateConnections, assignButtonHotkeys } from './actions.mjs';
import { applyDraft } from './document.mjs';
import {designSchema, designInstructions, usabilityInstructions} from './design.mjs';
import { geometryChecks, measureLayout, checkGeometry } from './layout.mjs';
const bindings = { Input: ['value', 'string'], Keypad: ['value', 'string'], Switch: ['checked', 'boolean'], Popup: ['open', 'boolean'], Select: ['value', 'string'], MultiSelect:['value','array'], Tabs: ['value', 'string'], Slider: ['value', 'number'] };

const path = z.string().regex(/^\/[A-Za-z][A-Za-z0-9_]{0,63}$/);
const themeSchema = z.object(Object.fromEntries(
  ['background','surface','foreground','muted','border','primary','focus','danger','success','warning']
    .map(role => [role, z.string().regex(/^#[0-9a-fA-F]{6}$/)]),
)).extend({design:designSchema.nullable().optional()}).strict();
const blueprint = z.object({
  guidance: z.string().max(2000).optional(),
  theme: themeSchema.nullable().optional(),
  state: z.record(z.string(), z.unknown()),
  widgets: z.array(z.object({
    id: z.string().regex(/^[a-z][a-z0-9_-]{0,63}$/),
    description: z.string().min(1).max(400),
    type: z.enum(Object.keys(catalog.data.components).filter(type => type !== 'Keypad')),
    props: z.record(z.string(), z.unknown()),
    on: z.object({ press }).strict().optional(),
    children:z.array(z.string().regex(/^[a-z][a-z0-9_-]{0,63}$/)).max(128).nullable().optional(),
  }).strict()).max(128),
}).strict();
const id = z.string().regex(/^[a-z][a-z0-9_-]{0,63}$/);
const changes = {
  geometryChecks,
  layoutChecks: z.array(z.object({container:id,axis:z.enum(['horizontal','vertical']),children:z.array(id).min(2).max(128)}).strict()).max(16)
    .describe('Requested spatial relationships to verify against the completed tree. Side-by-side columns require horizontal; top-to-bottom sections require vertical. Empty only when no spatial relationship is requested.'),
  placements: z.array(z.object({id,parent:id.nullable(),index:z.number().int().min(0).max(127)}).strict()).max(128),
  updates: z.array(z.union([
    z.object({id,key:z.string().regex(/^[A-Za-z][A-Za-z0-9_]*$/),value:scalar.nullable()}).strict(),
    z.object({id,key:z.string().regex(/^[A-Za-z][A-Za-z0-9_]*$/),json:z.string().min(1).max(16000).describe('JSON-encoded property value, for an array or object. Replaces only this property, preserving the rest of the widget.')}).strict(),
  ])).max(128),
  remove: z.array(id).max(128),
  stateUpdates: z.array(z.object({key:z.string().regex(/^[A-Za-z][A-Za-z0-9_]{0,63}$/),value:stateValue}).strict()).max(64),
};
export const documentBlueprint = blueprint.extend(changes);
// A repair can keep a whole edit list with null, or replace it with an array (including []).
const repairChanges=Object.fromEntries(Object.entries(changes).map(([key,schema])=>[key,schema.nullable().describe(`null preserves the previous ${key}; an array replaces that entire list, [] cancels it.`)]));
const repairBlueprint=blueprint.extend(repairChanges);

// Strict API output keeps a small model inside the catalog; code restores the state map.
const stringRef = z.object({ $state: path }).strict();
const structuredBlueprint = z.object({
  guidance: z.string().max(2000),
  theme: themeSchema.extend({design:designSchema.nullable().default(null)}).nullable(),
  state: z.array(z.object({ key: z.string().regex(/^[A-Za-z][A-Za-z0-9_]{0,63}$/), value: stateValue }).strict()).max(64),
  widgets: z.array(z.union(Object.entries(catalog.data.components)
    .filter(([type]) => type !== 'Keypad')
    .map(([type, definition]) => {
      let props = definition.props;
      if (type === 'Text') props = props.extend({ text: z.union([scalar, stringRef]), muted: z.boolean() });
      if (type === 'Metric') props = props.extend({ value: z.union([scalar, stringRef]) });

      if (type === 'ScrollView') props = props.extend({ text: z.union([props.shape.text, stringRef]) });
      if (bindings[type]) props = props.extend({ [bindings[type][0]]: z.object({ $bindState: path }).strict() });
      if (['Button', 'Sparkline'].includes(type)) props = props.required();
      return z.object({
        id: z.string().regex(/^[a-z][a-z0-9_-]{0,63}$/), description: z.string().max(100),
        type: z.literal(type), props: props.extend({grow: z.number().int().min(0).max(16).default(0)}).strict(),
        ...(['Column','Row','Grid','Panel'].includes(type) ? {children:z.array(id).max(128).nullable().default(null).describe('Exact ordered children. null preserves current membership. Supply IDs for new containers.')} : {}),
        ...(type === 'Button' ? { on: z.object({ press }).strict() } : {}),
      }).strict();
    }))).max(32),
}).strict();
const outputSchema = z.toJSONSchema(structuredBlueprint);
delete outputSchema.$schema;
const documentSchema = z.toJSONSchema(structuredBlueprint.extend(changes));
delete documentSchema.$schema;
const repairSchema=z.toJSONSchema(structuredBlueprint.extend(repairChanges));
delete repairSchema.$schema;

const documentInstructions = `You are editing a persistent terminal application. Code applies your proposal deterministically; do not delegate missing layout or behavior decisions to another model.
For a NEW app, a Column named root already exists. Supply all needed primitive widgets. Prefer explicit children lists on containers to express membership and order once. Each placement is {id,parent,index}; parent:null means root, index is zero-based sibling position. Grids fill row-major. Never move or remove root. Widget IDs must be stable, descriptive and unique. Container children lists are at widget level beside type/props, never inside props. children:null preserves membership; an array sets the exact membership. Do not repeat those relationships in placements. When replacing a child list, move or remove any omitted existing children; do not leave orphan widgets. Every non-root container must contain real content. A Panel followed by siblings does not contain those siblings.
Layout semantics: Row places its children LEFT-TO-RIGHT. Column and Panel stack their children TOP-TO-BOTTOM. Grid fills left-to-right in rows of props.columns cells. Names and descriptions never affect geometry. Multiple Column widgets under a Column remain vertically stacked. To make N side-by-side columns, put those N columns under ONE Row (or Grid with columns:N), then place that horizontal container in the outer page. Keep headings/footers outside it. Within each sidebar, use Column for vertically stacked items.
For changed spatial relationships only, supply concise layoutChecks with the actual common container ID, horizontal or vertical axis, and the ordered group IDs. Prefer direct child IDs. Nested descendants are projected onto their containing sibling groups; code orders those groups and verifies the actual container axis. Unknown IDs or references outside the container are rejected; use children lists to establish membership. Example: two side-by-side sections use a Row named body with children left/right, placements of left/right into body, and layoutChecks:[{container:"body",axis:"horizontal",children:["left","right"]}]. Do not assert a vertical relationship when the user asks for columns side-by-side. Changing names or wrapping existing content without changing geometry does not fulfill a layout request. Do not enumerate the whole page in a root check or flatten descendants into their ancestors. Use separate checks for nested groups. Color/content-only edits can use layoutChecks:[].
Measured layout: currentLayout contains actual native bounds, inner bounds (after panel borders/padding), and allocated slots, each [x,y,width,height] in terminal cells. viewport is the user's preview size. Use these measurements to diagnose the actual cause. For requested spacing, sizing or pinning changes, supply geometryChecks: {kind:"gap",from:"rail-list",to:"chat",axis:"horizontal",min:0,max:1,minViewportWidth:72}; {kind:"size",id:"rail",axis:"horizontal",min:24,max:24,minViewportWidth:72}; or {kind:"edge",id:"composer",parent:null,edge:"bottom",inset:0,minViewportWidth:72}. An edge parent:null means viewport; a parent ID means that ancestor's INNER bounds. Check visible content IDs when a wrapper is wider than its contents. minViewportWidth:0 always checks; use a breakpoint only for deliberately responsive layouts. The native renderer checks these after applying your edit and returns actual failed measurements for repair. geometryChecks:[] is appropriate only when no measurable spatial change is requested. During repair return geometryChecks:null to preserve the original requirements; do not relax or delete them.
For FEEDBACK, return ONLY changes. widgets contains complete new/replacement widgets with the SAME IDs as existing replacements. updates contains property edits: {id,key,value} for a scalar/null; {id,key,json} for an array or object, with its JSON encoded as a string. For example {id:"table",key:"headers",json:"[\\\"Name\\\",\\\"Value\\\"]"}. Each update REPLACES only the named property; it is not a deep merge. Prefer updates over replacement widgets for any property-only edit. Preserve bindings and all unmentioned properties. Use a complete replacement widget only to change its type or event actions. placements moves existing or new widgets; omitted placements preserve existing layout and new widgets default to root. remove deletes a widget and its descendants. Existing children are preserved when children is null; provide an exact children list when changing grouping. placements is an alternative for surgical moves where the destination does not have an explicit children list.
state declares ONLY new state keys; existing user values are preserved. stateUpdates explicitly sets values when requested, creating missing keys. theme:null preserves colors. Empty arrays mean no changes. A theme-only request needs no widgets, updates or placements. Respect the original goal and prior feedback; preserve everything outside the requested change. Never create duplicate substitutes for an existing widget. No explanation panels or pretend backend features.
For validation REPAIR, return null for each unchanged edit list (placements, updates, remove, stateUpdates, layoutChecks); an array replaces that entire list and [] explicitly cancels it. Only corrected widgets/state need repeating. Keep all unrelated proposed edits. Fix the actual tree to satisfy the requested relationship, never weaken a layout check to excuse the wrong geometry. Return every schema field. Keep guidance as one short sentence describing the change.`;

export const sizingInstructions = `Presentation: Every widget supports width: 1-240 for a fixed cell count, "content" to fit text/controls and their container, "fill" for remaining space, or null for legacy automatic sizing. Prefer explicit widths on Row children: sidebar width:24, main width:"fill"; input width:"fill", button width:"content". Content sizing measures terminal display cells including borders. Fluid charts without natural width continue to fill their slot. width overrides widthPercent; maxWidth remains an upper bound. Omitted/null width preserves legacy sizing. Every widget also supports maxWidth (0=no limit, otherwise terminal cells <=240), widthPercent (1-100, default100), horizontalAlign (left|center|right, placement inside its allocated area). Prefer restrained borders and compact spacing; do not wrap every widget in nested boxes.
In a horizontal Row, children with maxWidth>0 occupy at most that many cells; uncapped siblings divide the remaining width. Use maxWidth on the sidebar container itself and 0 on the main content; use a capped action button beside an uncapped input. Grid keeps equal-width tracks. widthPercent scales within the allocated slot, not the entire row, and can deliberately leave blank space. grow controls HEIGHT only, not row width. For full-screen applications leave the root and main workspace uncapped. To remove a gap, inspect width limits on the actual siblings and their nested children, not just the parent's gap.
Application titles should normally be compact Text, not BigText banners, unless the user requests display lettering. Keep navigation lists at intrinsic height unless stretching them serves the request; spend extra height on the main working area. Decorative headings and empty bordered areas must not crowd out content.
Column/Row/Panel support gap (0-4 cells). Row supports collapseBelow (0 disables; otherwise stack children vertically when its allocated width is smaller). Grid supports minCellWidth (0 disables; otherwise automatically reduce columns to fit; columns is the maximum), gap (0-4). Use these responsive properties for layouts that must work in narrow terminals. Layout checks describe the wide-screen intended axis; code handles narrow screens deterministically.
Metric, Gauge, Sparkline and BarChart support color (null to use theme, a theme role foreground|muted|primary|success|warning|danger, literal green|yellow|red, or #RRGGBB) and thresholds:[{min:number,color:...}] with strictly increasing inclusive lower bounds, at most16. Highest matching min wins; below all thresholds uses color or the default. Example utilization: color:"green", thresholds:[{min:70,color:"yellow"},{min:90,color:"red"}]. Choose thresholds appropriate to the request; no GPU-specific rules exist in code. When color-coding a quantity, apply the same rules to its existing metric readouts and matching charts unless the user restricts the target. Use thresholds rather than painting only the current value a static color. Metric thresholds read its live numeric value (numeric string or trailing % also accepted); BarChart evaluates each bar separately; Sparkline colors the whole series from its latest sample. Prefer bound numeric values and put units in labels. For threshold edits update BOTH color (the below-threshold base color) and thresholds on EVERY requested readout/chart. Adding yellow/red thresholds alone leaves the existing base color unchanged; it does not make lower values green. Use scalar updates for color and updates with key:"thresholds" and json containing the complete array. Alternatively include the lowest range explicitly (e.g. min:0,color:"success" for nonnegative percentages). Do not resend whole widgets. Compact output must still include every requested change. A local value color request should preserve the shared theme.
Panel supports border plain|rounded|heavy|double|none, padding 0-4, titleAlign left|center|right. Table supports border, padding (cell padding), columnGap 0-4, showHeader boolean, columnStyles [] or exactly one {width:0 for flexible or 1-240 cells,align:left|center|right,color:foreground|muted|primary|success|warning|danger} per header. Right-align quantities and left-align names. Preserve readable widths on small screens.
Select supports searchable:true for local fuzzy filtering, arrows to highlight, Enter to commit. MultiSelect binds value to an array of exact option strings, supports searchable, and Space toggles choices. Declare string-array state; never encode a selection array as a comma-separated string. MultiSelect can start empty; Select must start with one exact option. Both support bordered:false to omit their frame/title inside an already labeled region; default true retains the frame. Both support up to100 unique options. Use setState with [] to clear multiple choices; numeric compute cannot operate on arrays.
Every widget supports props.grow: integer 0-16 (default 0). Omitted grow on a container inherits the largest visible child grow; explicit grow:0 stops inheritance. Vertical Column/Panel children keep their intrinsic minimum heights and share remaining viewport height in proportion to grow. Grid rows grow by their largest child weight. A Row passes its full height to all children. The root already receives the viewport. To fill the screen, set grow:1 on the expanding content AND every intervening container; leave fixed headers, footers and input/composer groups at grow:0. A growing ScrollView uses height as its minimum: use height:4 for a flexible conversation/log so compact terminals still have room for controls. To pin a composer to the bottom, place it after a growing content sibling in a growing Column/Panel. Do not fake expansion with blank text or a larger fixed height. Property updates can set grow and height on existing widgets without replacing their content or actions.`;

const strictSystem = `Supply guidance and primitive instances for a native terminal UI. Return the supplied JSON schema, never executable code.
Choose a coherent theme for this specific request: ten #RRGGBB colors for background, surface, foreground, muted text, border, primary accent, focus, danger, success and warning. Vary the palette to suit the subject and mood instead of defaulting to amber/cyan or the same dark theme. Both light and dark themes are supported. Keep foreground/muted text readable on background and surface; distinguish focus and status colors. Coordinate Scene/AnsiArt colors with the palette. For refinement preserve the current theme by returning null, unless the user requests a color change. New interfaces require a complete theme. For validation repairs return null to preserve the proposed theme.
Use 4-10 useful instances unless the requested button grid requires more. No unrequested logs, history, presets or explanatory panels. Keep guidance below 80 words. Describe the intended group and linear reading position of each item. A Grid fills left-to-right, then top-to-bottom; supply its Buttons separately. Use ordinary controls rather than pretending one component has unsupported behavior.
Preserve the familiar controls and spatial structure of the requested interface. For a device normally operated through a keypad or control pad, supply its individual Buttons in a Grid, each with its working operation. Do not replace the control pad with a text input or draw fake controls in artwork. List widgets in their intended reading order, including row-major order for grid cells. Use as many instances as those controls require, within the schema limit.
State is an array of {key,value}; values may be scalars or arrays of option strings. Declare every referenced key, with only one key per quantity. Inputs bind strings, switches/popups booleans, sliders numbers. Select starts at an exact option; Tabs at an exact tab label. Text and Metric may show state through {$state:"/key"}; editable controls use {$bindState:"/key"}. Headings should be literal text. Mutable results must be bound to state, never a static copy.
Every Button has on.press, either one action or an ordered array of at most eight actions. Operations execute locally and atomically:
- setState: {statePath,value}; value is a literal or an actual {$state:"/key"} object.
- compute: {statePath,op,args}. add/subtract/multiply/divide take two numeric operands; append joins two text operands; backspace takes one text operand; evaluate takes one arithmetic-expression string (+ - * / parentheses). Numeric strings are accepted. randomInt takes [minimum,maximum,count] and sums count independent inclusive random integers; count 1-32. Computed results keep the target's string or number type.
Read adjustable operands DIRECTLY from their Input/Select state. To increment a counter use add with args:[{$state:"/counter"},1], not constants. To concatenate use append with two arguments, including actual reference objects. NEVER embed $state, interpolation syntax, or placeholders INSIDE a string. For multi-step computation, write the intermediate result, then read it in the next action. Each button needs an observable effect on a bound display or control. Do not fake an operation with a fixed answer. Do not create shadow copies of inputs.
For example, sampling with an adjustable maximum uses randomInt args:[1,{$state:"/maximum"},{$state:"/count"}] targeting /result. A second add can use [{$state:"/result"},{$state:"/modifier"}]. A Text bound to /result displays the output.
For game interfaces include one prominent Scene or AnsiArt. Scene rows are compact ASCII maps; legend symbols identify colors and labels. AnsiArt accepts ANSI color escapes and newlines. Other display widgets use plain content.
Refinement: return only requested changes and preserve existing values. No invented actions, network calls or game backend. Extended ASCII/BBS requests use AnsiArt with Unicode box-drawing/block characters and ANSI SGR colors (e.g. JSON \\u001b[36m). BigText only accepts printable ASCII and makes very wide headings; use Text or AnsiArt for compact CP437-style banners. Scene maps only support printable ASCII; never put extended glyphs in Scene. Respect explicit entity/player counts in the visible art. State/operation correctness takes priority over decoration.`;

const system = `You guide Jev in constructing native terminal interfaces from reusable primitives for ANY user request.
Choose a request-appropriate color scheme in top-level theme: {background,surface,foreground,muted,border,primary,focus,danger,success,warning}, all #RRGGBB strings. Both light and dark themes work. Vary hues and surfaces to fit the subject; do not default to amber/cyan. Maintain readable text and distinct focus/status colors. Coordinate artwork with the palette. For refinements or validation repairs return theme:null to preserve the current palette unless a color change is requested.
Create specific, convincing content and plausible fictional sample data for the requested subject.
Return ONLY one JSON object: {"theme":{...},"guidance":"Required capabilities, groups and ordering for Jev", "state":{...},"widgets":[{"id":"unique-id","description":"what this widget shows and its purpose","type":"...","props":{...}}]}.
Produce only the primitive instances the request needs (normally 4-10; up to 32 ONLY when requested controls require many buttons). Do not add unrequested presets, history, logs, explanation panels or duplicate readouts. Keep JSON concise: descriptions under 12 words, short labels, tables at most 3 rows. Minify JSON. No generic revenue/services dashboard unless requested.
You provide guidance, content, state and operation bindings using the primitives below. Keep guidance under 80 words. Declare ONLY state referenced by displayed controls or actions, never unused domain variables. Use literal Text for headings. Jev chooses membership, grouping and order; do not return a tree or children. Describe each control group clearly so Jev can assemble it. Do not claim unsupported behavior or substitute a related widget for a missing capability.
Available widget types and EXACT props (no extra properties):
Column {} and Row {}: vertical/horizontal grouping. Grid {columns:integer1-8}: generic equal-width cells, ordered left-to-right then top-to-bottom. Supply separate Button instances; describe which grid they belong in and their desired order. Grids may contain any primitives.
Panel {title:string}: optional section container. Prefer few panels, never nested.
Text {text:string,muted?:boolean}: heading, prose or status. Newlines supported. NEVER put ANSI escapes in Text; use AnsiArt for styled artwork or Scene for maps.
Metric {label:string,value:string}: small labeled readout, numbers formatted as strings.
Gauge {title:string,value:number}: 0-100 percentage.
Sparkline {title:string,data:number[],dense:boolean}: dense=false is a compact trend; dense=true uses a taller braille graph. Data: 1-24 nonnegative integers <=1000000000.
Table {title:string,headers:string[],rows:string[][]}: 1-6 columns, 2-6 concise rows with EXACTLY as many cells as headers.
List {title:string,items:string[]}: 2-6 concise entries.
Input {label:string,value:{"$bindState":"/key"}}: editable string; MUST initialize state.key as a string.
Button props are {label:string,hotkey:string|null,variant:"solid"|"plain",intent:"neutral"|"primary"|"danger"|"success"|"warning",disabled:boolean}. Put on at the WIDGET level alongside id, description, type and props, NEVER inside props. variant defaults to solid (three rows); plain is a one-row bracketed button with visible focus. Use width:"content" for compact actions. A Button MUST have on:{"press":action} or on:{"press":[action,...]} with at most 8 sequential actions. Every action uses initialized state.
Badge {label:string,intent:"neutral"|"primary"|"danger"|"success"|"warning"}: compact labeled status.
Switch {label:string,checked:{"$bindState":"/key"},disabled:boolean}: native toggle; initialize a BOOLEAN state value. No on needed.
There is no prebuilt calculator or dice roller in this generation catalog. Compose interfaces from Input, Text, Button and Grid, plus general operations. A numeric input does not imply a calculator. A control label must describe the behavior implemented by its binding.
${extendedInstructions}
Actions are reusable local operations, NEVER code, network or shell:
setState {statePath:"/key",value:literal or {"$state":"/other"}} assigns state.
compute {statePath:"/key",op:"add"|"subtract"|"multiply"|"divide"|"randomInt"|"append"|"backspace"|"evaluate",args:[...]} reads literals or {"$state":"/key"} at press time. add/subtract/multiply/divide take two numeric arguments (numeric strings accepted); append joins two strings; backspace removes one character from one string; evaluate parses one arithmetic string (+ - * / parentheses, max128 chars). randomInt takes EXACTLY [minimum,maximum,count], returns the SUM of count independent inclusive random integers (count1-32, min/max integers within ±1000000). Computed results become text for string targets or numbers for numeric targets. A sequence can sample into /result and then add /modifier into /result. Use numeric strings in Input or Select for adjustable numeric arguments. Text can display {$state:"/result"}. Bind actual random/arithmetic operations instead of fixed sample answers or fabricated feedback. All operations in a press commit atomically. Choose the SAME primitives for any domain requiring these capabilities, without inventing a specialized widget.
Use state for inputs and meaningful button interactions. For example a status Text with text:{"$state":"/status"} and two buttons that set /status to different messages. A button can also change a selected-view Text or Table through state. Initialize every referenced state key. State paths are /simpleKey, not nested paths. Do not put every static label into state. Never nest $state references inside state values.
Every state value changed by an action MUST reach a display or control, directly or through other actions. Bind live Metric.value, Text.text or ScrollView.text using {$state:"/key"}; never hardcode a copy of mutable state into a display. Computations MUST read their adjustable parameters directly from the corresponding Input or Select bindings. One state key per quantity: never create a separate shadow copy for computation. Use literals for fixed constants. To increment /turn use compute args:[{$state:"/turn"},1], NEVER [1,1]. To append to /log use args:[{$state:"/log"},"new text"]. To include a state value in a message, use separate append operations with actual {$state:"/key"} objects, never string interpolation. Keep interfaces compact and implement the requested core interactions before decorative controls.
Props may use {"$state":"/key"} for dynamic values; all other props must be literal values. Strings are displayed verbatim: NEVER use template interpolation or placeholders inside strings. Put an entire dynamic message in a state string and bind the whole text prop. No repeat, visibility, expressions outside the documented actions, slots or callbacks.
Include an application heading and relevant controls. Every Button has a working accelerator. Set hotkey to a unique Alt+letter/digit or F1–F24, or null for automatic assignment. Keep the key out of label; the renderer displays it and measures the extra width. Never invent shortcut labels. For view-changing buttons, set displayed state to substantive view content, not internal IDs like "map". Use implemented local operations for actual interactions; clearly label simulation-only messages for missing backend capabilities.
For a refinement, you receive the current spec. Return ONLY new or replacement widgets and new state keys needed for the requested edit. Existing widgets remain available to Jev. Existing state keys and user-entered values are preserved, so use new keys if changing a data binding's type or initial value. For removal/reordering-only requests, return {"state":{},"widgets":[]}.
Descriptions are shown to Jev: make the intended section/order and role clear without relying on raw state. Keep all JSON concise.`;

function safeValues(value, state, inState = false) {
  if (!inState && typeof value === 'string' && /\{\s*["']?\$(?:bindState|state)["']?\s*:/.test(value)) {
    throw new Error('State references must be objects, not placeholders inside text or artwork. Use a separate bound Text or Metric for the live value.');
  }
  if (!value || typeof value !== 'object') return;
  for (const [key, child] of Object.entries(value)) {
    if (['__proto__', 'prototype', 'constructor'].includes(key)) throw new Error('Unsafe object key in generated content.');
    if (key.startsWith('$')) {
      if (inState || !['$state', '$bindState'].includes(key) || Object.keys(value).length !== 1) throw new Error('Unsupported generated expression.');
      const pointer = path.parse(child);
      if (!Object.hasOwn(state, pointer.slice(1))) throw new Error(`Missing generated state: ${pointer}`);
    }
    safeValues(child, state, inState);
  }
}

function checkProps(element, state) {
  const definition = catalog.data.components[element.type];
  const resolved = resolveElementProps(element.props, { stateModel: state });
  const parsed = definition.props.strict().safeParse(resolved);
  if (!parsed.success) {
    const details = [...new Set(parsed.error.issues.map(issue => issue.path[0]))].flatMap(key => {
      const pointer = element.props[key]?.$state ?? element.props[key]?.$bindState;
      if (!pointer || !definition.props.shape[key]) return [];
      const value = resolved[key];
      const type = Array.isArray(value) ? 'array' : value === null ? 'null' : typeof value;
      const schema = z.toJSONSchema(definition.props.shape[key]); delete schema.$schema;
      return [`${element.type}.${key} reads ${pointer}, which resolves to ${type}; required value contract: ${JSON.stringify(schema)}. Use a compatible binding or widget, or explicitly correct the state value. Arrays of strings can bind to List.items; Text.text requires a string, number or boolean.`];
    });
    if (details.length) throw new Error(`${validationMessage(parsed.error)}. ${details.join(' ')}`);
    throw parsed.error;
  }
  const props = parsed.data;
  if (props.thresholds?.length) {
    if(props.thresholds.some((t,i)=>i>0 && t.min<=props.thresholds[i-1].min)) throw new Error('Color thresholds need strictly increasing min values.');
    if(element.type==='Metric' && (typeof props.value==='boolean' || !/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?\s*%?$/i.test(String(props.value).trim()) || !Number.isFinite(Number(String(props.value).trim().replace(/%$/, '').trim())))) throw new Error('Metric thresholds require a numeric value (optionally suffixed with %).');
  }
  if (element.type === 'Table' && (props.rows.some(row => row.length !== props.headers.length) || (props.columnStyles.length && props.columnStyles.length !== props.headers.length))) throw new Error('Generated table rows must match its headers.');
  if (element.type === 'BarChart' && props.labels.length !== props.values.length) throw new Error('BarChart labels and values must have equal lengths.');
  const options = element.type === 'Select' ? props.options : element.type === 'Tabs' ? props.tabs.map(tab => tab.label) : null;
  if (options && (new Set(options).size !== options.length || !options.includes(props.value))) throw new Error('Selection must match a unique option or tab label.');
  if (element.type==='MultiSelect' && (new Set(props.options).size!==props.options.length || new Set(props.value).size!==props.value.length || props.value.some(value=>!props.options.includes(value)))) throw new Error('MultiSelect value must contain unique exact options.');
  if (element.type === 'Slider' && (props.min >= props.max || props.value < props.min || props.value > props.max)) throw new Error('Slider requires min < max and a value in range.');
  if (element.type === 'Scene' && (new Set(props.legend.map(item => item.symbol)).size !== props.legend.length)) throw new Error('Scene legend symbols must be unique.');
}

function checkWidget(element, state) {
  safeValues(element, state);
  checkProps(element, state);
  if (bindings[element.type]) {
    const [field, type] = bindings[element.type];
    const pointer = path.parse(element.props[field]?.$bindState);
    if (type==='array' ? !Array.isArray(state[pointer.slice(1)]) : typeof state[pointer.slice(1)] !== type) throw new Error('Generated control must bind a compatible state value.');
  }
  if (element.on && element.type !== 'Button') throw new Error('Only buttons support generated events.');
  if (element.on?.press) checkActions(element.on.press, state);
}

export function prepareContent(raw, initialSpec) {
  safeValues(raw?.state, {}, true);
  const draft = blueprint.parse(raw);
  if (!initialSpec && !draft.widgets.length) throw new Error('The content model returned no interface widgets.');
  if (new Set(draft.widgets.map(w => w.id)).size !== draft.widgets.length) throw new Error('Generated widget IDs must be unique.');
  safeValues(draft.state, {}, true);
  const state = { ...draft.state, ...(initialSpec?.state ?? {}) };
  // Bound controls own useful defaults. Missing model bookkeeping must not break a selector.
  for (const widget of draft.widgets) {
    const field = bindings[widget.type]?.[0];
    const pointer = field && widget.props[field]?.$bindState;
    if (pointer === undefined) continue;
    const key = path.parse(pointer).slice(1);
    if (Object.hasOwn(state, key)) {
      // Normalize unambiguous model-produced scalar representations, never user state.
      if (!Object.hasOwn(initialSpec?.state ?? {}, key)) {
        const type = bindings[widget.type][1];
        if (type === 'string' && typeof state[key] === 'number') state[key] = String(state[key]);
        else if (type === 'boolean' && ['true','false'].includes(state[key])) state[key] = state[key] === 'true';
        else if (type === 'number' && typeof state[key] === 'string' && state[key].trim() && Number.isFinite(Number(state[key]))) state[key] = Number(state[key]);
      }
      continue;
    }
    const props = widget.props;
    const value = widget.type === 'MultiSelect' ? [] : widget.type === 'Select' ? props.options?.[0]
      : widget.type === 'Tabs' ? props.tabs?.[0]?.label
      : widget.type === 'Slider' ? props.min
      : ['Switch', 'Popup'].includes(widget.type) ? false : '';
    if (value !== undefined) state[key] = value;
  }
  safeValues(state, {}, true);
  const generated = draft.widgets.map(({ id, description, children, ...element }, index) => ({ id: `generated-${id}`, description: `Reading position ${index + 1}: ${description} [${element.type}${bindingSummary(element.props)}${element.on?.press ? `; ${actionSummary(element.on.press)}` : ""}]`, root: false, element }));
  const existing = Object.entries(initialSpec?.elements ?? {}).map(([id, element]) => ({
    id: `existing-${id}`, root: false,
    description: `Existing ${element.type}: ${['title', 'label', 'text'].map(k => element.props[k]).filter(v => typeof v === 'string').join(' ').slice(0, 300) || id}`,
    element: { type: element.type, props: structuredClone(element.props), ...(element.on ? { on: element.on } : {}), ...(element.visible !== undefined ? { visible: element.visible } : {}) },
  }));
  const content = [...existing, ...generated];
  assignButtonHotkeys(content.map(candidate=>candidate.element));
  const errors=[];
  for (const candidate of generated) {
    try { checkWidget(candidate.element, state); }
    catch (error) { errors.push(`Widget ${candidate.id}: ${validationMessage(error)}`); }
  }
  // Report the whole batch: first-error-only validation burns one model turn per widget.
  if(errors.length)throw new Error(errors.join('\n'));
  validateConnections(content.map(candidate => candidate.element));
  // Verify actions against all displayed prop types before accepting the generated recipes.
  for (const { element } of generated) {
    if (element.on?.press) {
      const next = checkActions(element.on.press, state);
      for (const { element: other } of content) {
        checkProps(other, next);
      }
    }
  }
  return {
    state,
    theme: draft.theme ?? (initialSpec?.theme ? themeSchema.parse(initialSpec.theme) : undefined),
    guidance: draft.guidance ?? '',
    candidates: [
      { id: 'layout-column', description: 'Root vertical layout for the requested primitive groups.', element: { type: 'Column', props: {} }, maxUses: 1 },
      ...content,
    ],
  };
}

// Native serialization includes empty event maps. Visibility and child relationships
// stay in the document and receive their final validation in the native renderer.
export function prepareDocument(spec, initialState) {
  // Thresholds describe ranges; ascending order is compiler bookkeeping, not a model decision.
  for(const element of Object.values(spec.elements)) {
    if(!Array.isArray(element.props.thresholds))continue;
    const parsed=catalog.data.components[element.type]?.props.shape.thresholds?.safeParse(element.props.thresholds);
    if(parsed?.success)element.props.thresholds=[...parsed.data].sort((a,b)=>a.min-b.min);
  }
  assignButtonHotkeys(Object.values(spec.elements));
  const existing=initialState ? Object.fromEntries(Object.keys(initialState).map(key=>[key,spec.state[key]])) : undefined;
  return prepareContent({state:spec.state,theme:spec.theme,widgets:Object.entries(spec.elements).map(([id,{children,visible,on,...element}])=>({id,description:id,...element,...(on?.press?{on}:{})}))},existing ? {state:existing,elements:{}} : undefined);
}

function validationMessage(error) {
  return error.issues ? error.issues.slice(0, 3).map(issue => `${issue.path.join('.') || 'widget'}: ${issue.message}`).join('; ') : error.message;
}

export async function generateContent(request, env = process.env, fetcher = globalThis.fetch, measure = env.RATATUI_JSON_NATIVE ? (spec,viewport)=>measureLayout(spec,viewport,env.RATATUI_JSON_NATIVE) : null) {
  const base = (env.LLM_BASE_URL || (env.OPENAI_API_KEY ? 'https://api.openai.com/v1' : 'http://100.115.205.43:8000/v1')).replace(/\/$/, '');
  const url = new URL(`${base}/chat/completions`);
  if (!['http:', 'https:'].includes(url.protocol)) throw new Error('LLM_BASE_URL must be HTTP(S).');
  const openai = url.origin === 'https://api.openai.com';
  const model = env.LLM_MODEL || (openai ? 'gpt-5.6-terra' : 'latest');
  // OpenAI credentials are never forwarded to the local or a custom provider.
  const apiKey = openai ? (env.OPENAI_API_KEY || env.LLM_API_KEY) : env.LLM_API_KEY;
  if (openai && !apiKey) throw new Error('Set OPENAI_API_KEY to use the OpenAI content model.');
  const viewport=z.tuple([z.number().int().min(1).max(500),z.number().int().min(1).max(200)]).parse(request.viewport??[100,36]);
  const currentLayout=request.direct && request.initialSpec && measure ? await measure(request.initialSpec,viewport) : undefined;
  const messages = [
    { role: 'system', content: (openai ? strictSystem : system) + '\n' + designInstructions + '\n' + usabilityInstructions + '\n' + sizingInstructions + (request.direct ? '\n'+documentInstructions : '') },
    { role: 'user', content: JSON.stringify({ request: request.prompt, selectedTheme:request.selectedTheme, viewport, currentLayout, ...(request.goal ? {originalGoal:request.goal} : {}), ...(request.history?.length ? {priorFeedback:request.history.slice(-6).map(turn=>({prompt:String(turn.prompt).slice(0,1000)}))} : {}), ...(request.initialSpec ? { currentSpec: request.initialSpec } : {}) }) },
  ];
  const started = performance.now();
  const signal = AbortSignal.timeout(120000);
  const calls = [];
  let draft;
  for (let attempt = 0; attempt < 2; attempt++) {
    if (Buffer.byteLength(JSON.stringify(messages)) > 64000) throw new Error('Current interface exceeds the model context budget. Reduce the interface or start a new project.');
    let response;
    const callStarted=performance.now();
    try {
      response = await fetcher(url, {
        method: 'POST', signal, redirect: 'error',
        headers: { 'Content-Type': 'application/json', ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}) },
        body: JSON.stringify({
          model, messages, response_format: openai
            ? { type: 'json_schema', json_schema: { name: 'ui_content', strict: true, schema: request.direct ? (draft ? repairSchema : documentSchema) : outputSchema } }
            : { type: 'json_object' },
          ...(openai ? { ...(model.startsWith('gpt-5') ? { reasoning_effort: 'none' } : {}), max_completion_tokens: 4096 }
            : { reasoning_effort: 'low', thinking_token_budget: 512, max_tokens: 4096 }),
        }),
      });
    } catch (error) {
      throw new Error(error.name === 'TimeoutError' ? 'Content generation timed out. Try again.' : `Cannot reach the content model at ${url.origin}. Check LLM_BASE_URL.`);
    }
    if (!response.ok) throw new Error(`Content model returned HTTP ${response.status}.`);
    const result = await response.json();
    calls.push({ model: result.model ?? model, usage: result.usage ?? null, elapsedMs:Math.round(performance.now()-callStarted) });
    const choice = result.choices?.[0];
    if (choice?.finish_reason === 'length') throw new Error('Content model hit its output limit. Ask for a smaller interface.');
    const text = choice?.message?.content;
    if (typeof text !== 'string') throw new Error('Content model returned no JSON content.');
    try {
      let raw;
      try { raw = JSON.parse(text.trim().replace(/^```(?:json)?\s*/i, '').replace(/\s*```$/, '')); }
      catch { throw new Error('Content model returned invalid JSON.'); }
      if (openai) {
        raw = (request.direct ? structuredBlueprint.extend(draft ? repairChanges : changes) : structuredBlueprint).parse(raw);
        if (new Set(raw.state.map(item => item.key)).size !== raw.state.length) throw new Error('State keys must be unique.');
        raw.state = Object.fromEntries(raw.state.map(({ key, value }) => [key, value]));
      }
      raw = (request.direct ? (draft ? repairBlueprint : documentBlueprint) : blueprint).parse(raw);
      if(raw.theme && !raw.theme.design) {
        const inherited=draft?.theme?.design??request.initialSpec?.theme?.design;
        if(inherited) raw.theme.design=inherited;
        else delete raw.theme.design;
      }
      if (new Set(raw.widgets.map(widget => widget.id)).size !== raw.widgets.length) throw new Error('Generated widget IDs must be unique.');
      if (draft) {
        if(request.direct && raw.geometryChecks!==null && JSON.stringify(raw.geometryChecks)!==JSON.stringify(draft.geometryChecks)) throw new Error('Preserve the original geometryChecks during repair (return null). Fix the layout instead of relaxing its checks.');
        // A repair replaces only named widgets/state. Revalidate the entire merged draft.
        raw = {
          ...raw,
          guidance: raw.guidance || draft.guidance,
          theme: raw.theme ?? draft.theme,
          state: { ...draft.state, ...raw.state },
          widgets: [...new Map([...draft.widgets, ...raw.widgets.map(widget=>{
            const previous=draft.widgets.find(old=>old.id===widget.id);
            return widget.children==null && previous?.children ? {...widget,children:previous.children} : widget;
          })].map(widget => [widget.id, widget])).values()],
          ...(request.direct ? Object.fromEntries(Object.keys(changes).map(key=>[key,raw[key]??draft[key]])) : {}),
        };
      }
      if(request.selectedTheme) raw.theme=request.selectedTheme;
      draft = raw;
      if (!draft.theme && !request.initialSpec?.theme) throw new Error('Choose a complete theme palette for this new interface.');
      let content;
      if (request.direct) {
        const spec=applyDraft(draft,request.initialSpec);
        content=prepareDocument(spec,request.initialSpec?.state);
        spec.state=content.state;
        const layout=measure ? await measure(spec,viewport) : undefined;
        if(draft.geometryChecks.length && !layout) throw new Error('Geometry checks require the native renderer. Run through the tui-draw CLI.');
        if(layout) {
          checkGeometry(draft.geometryChecks,layout);
          const errors=(layout.audits??[]).filter(a=>a.severity==='error');
          if(errors.length) throw new Error(`Native audit failed: ${errors.slice(0,5).map(a=>`${a.id}: ${a.message}`).join('; ')}`);
        }
        content={...content,spec,layout,geometryChecks:draft.geometryChecks,guidance:draft.guidance};
      } else content = prepareContent(draft, request.initialSpec);
      const sum = key => calls.every(c => Number.isFinite(c.usage?.[key])) ? calls.reduce((total, c) => total + c.usage[key], 0) : null;
      return { ...content, generation: {
        elapsedMs: Math.round(performance.now() - started), model: calls.at(-1).model,
        usage: { prompt_tokens: sum('prompt_tokens'), completion_tokens: sum('completion_tokens'), total_tokens: sum('total_tokens') },
        calls,
      } };
    } catch (error) {
      const message = validationMessage(error);
      if (attempt === 1) throw new Error(`Generated content failed validation: ${message}`);
      const repairGuide=draft ? 'Return ONLY corrected state entries and replacement widgets using their original IDs; unchanged state/widgets are preserved. Empty state/widgets and empty guidance preserve their previous content. '
        + (request.direct ? 'For each edit list (placements, updates, remove, stateUpdates, layoutChecks), return null to PRESERVE the previous list, or return the COMPLETE corrected list to replace it. [] CANCELS that list. Return geometryChecks:null to preserve the original measurable requirements; they cannot be weakened during repair. Keep unrelated proposed edits. ' : '')
        + 'Fix all instances of this error.' : 'Return the complete corrected JSON object.';
      messages.push({ role: 'assistant', content: text }, { role: 'user', content: `Correct this validation error: ${message.slice(0, 1500)}. ${repairGuide} Keep the requested interface and only the allowed widget props. Put event bindings on the widget, not inside props.` });
    }
  }
}
