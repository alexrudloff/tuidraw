import test from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { catalog } from './catalog.mjs';
import { widgetContracts } from './contracts.mjs';

const SHARED = ['width', 'maxWidth', 'widthPercent', 'horizontalAlign', 'grow'];

test('catalog data is immutable across calls', () => {
  const before = z.toJSONSchema(catalog.data.components.Input.props);
  const a = widgetContracts(['Input', 'Button']);
  const b = widgetContracts(['Input', 'Button']);
  assert.deepEqual(a, b);
  assert.deepEqual(z.toJSONSchema(catalog.data.components.Input.props), before);
  assert.deepEqual(a.widgets.Input, b.widgets.Input);
});

test('Input value is required and $schema marker is removed', () => {
  const { widgets } = widgetContracts(['Input']);
  assert.ok(!('$schema' in widgets.Input));
  assert.deepEqual(widgets.Input.required, ['label', 'value']);
  assert.equal(widgets.Input.properties.value.type, 'string');
});

test('shared sizing fields are emitted once and with their constraints', () => {
  const { sharedProps } = widgetContracts(['Text']);
  assert.deepEqual(Object.keys(sharedProps).sort(), [...SHARED].sort());
  assert.equal(sharedProps.maxWidth.maximum, 240);
  assert.equal(sharedProps.maxWidth.default, 0);
  assert.equal(sharedProps.widthPercent.minimum, 1);
  assert.equal(sharedProps.horizontalAlign.default, 'left');
  assert.equal(sharedProps.grow.maximum, 16);
  assert.ok('anyOf' in sharedProps.width);
});

test('shared fields are removed from each widget without repeating or breaking other constraints', () => {
  const { widgets } = widgetContracts(['Button', 'Table', 'Column']);
  for (const name of ['Button', 'Table', 'Column']) {
    for (const k of SHARED) assert.ok(!(k in widgets[name].properties), `${name} keeps shared ${k}`);
    if (widgets[name].required) for (const k of SHARED) assert.ok(!widgets[name].required.includes(k), `${name} required has ${k}`);
  }
  assert.deepEqual(widgets.Button.required, ['label']);
  assert.equal(widgets.Button.properties.variant.default, 'solid');
  assert.deepEqual(widgets.Table.properties.headers.items, { type: 'string', maxLength: 4096 });
  assert.equal(widgets.Table.additionalProperties, false);
  assert.equal(widgets.Column.properties.gap.anyOf[0].maximum, 4);
});

test('unknown widget names are rejected', () => {
  assert.throws(() => widgetContracts(['Nope']), /Unknown widget type: Nope/);
  assert.throws(() => widgetContracts('Nope'), /Unknown widget type/);
});

test('bindings guidance is preserved', () => {
  const { bindings } = widgetContracts(['Text']);
  assert.ok(bindings.includes('props describe resolved values'));
  assert.ok(bindings.some((b) => b.includes('$bindState')));
  assert.ok(bindings.some((b) => b.includes('$state')));
  assert.ok(bindings.includes('resolved types must match'));
  assert.ok(bindings.includes('Button.on.press belongs at widget level'));
});
