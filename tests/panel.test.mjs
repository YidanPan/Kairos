import assert from 'node:assert/strict';
import { test } from 'node:test';
import { setImmediate } from 'node:timers/promises';
import { clearMocks, mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { connectPanel } from '../node_modules/.cache/kairos-tests/panel.js';

globalThis.window = globalThis;

const primary = {
  name: 'Primary', scaleFactor: 1,
  position: { x: 0, y: 0 }, size: { width: 1920, height: 1080 },
  workArea: { position: { x: 0, y: 0 }, size: { width: 1920, height: 1040 } },
};
const secondary = {
  name: 'Secondary', scaleFactor: 1.25,
  position: { x: -1600, y: 0 }, size: { width: 1600, height: 1000 },
  workArea: { position: { x: -1600, y: 20 }, size: { width: 1600, height: 900 } },
};

async function startPanel(context, { positionFailures = 0 } = {}) {
  const lifetime = new AbortController();
  let poll;
  let failuresLeft = positionFailures;
  let failResize = false;
  context.mock.method(globalThis, 'setInterval', (callback) => {
    poll = callback;
    return 0;
  });
  mockWindows('main');
  context.after(() => {
    lifetime.abort();
    clearMocks();
  });

  const state = {
    displays: [primary, secondary], positions: [], sizes: [], writes: [], errors: [],
    preferences: {
      display: secondary.name, theme: 'system', transparency: 'light',
      layouts: {
        [primary.name]: { left: 120, top: 80, width: 400, height: 350 },
        [secondary.name]: { left: 32, top: 24, width: 480, height: 650 },
      },
    },
  };
  mockIPC((command, args) => {
    switch (command) {
      case 'load_panel_preferences': return state.preferences;
      case 'save_panel_preferences': return state.writes.push(args.preferences);
      case 'plugin:window|available_monitors': return state.displays;
      case 'plugin:window|primary_monitor': return primary;
      case 'plugin:window|is_minimized': return false;
      case 'plugin:window|set_position': {
        if (failuresLeft > 0) {
          failuresLeft--;
          throw new Error('Display temporarily unavailable');
        }
        state.positions.push(args.value.toJSON().Physical);
        return;
      }
      case 'plugin:window|set_size': {
        if (failResize) {
          failResize = false;
          throw new Error('Could not resize the panel');
        }
        return state.sizes.push(args.value.toJSON().Logical);
      }
      case 'plugin:window|set_size_constraints':
      case 'plugin:window|show': return;
      default: throw new Error(`Unexpected command: ${command}`);
    }
  }, { shouldMockEvents: true });
  await connectPanel(lifetime.signal, (error) => state.errors.push(error));
  return {
    positions: state.positions, sizes: state.sizes, writes: state.writes, errors: state.errors,
    failNextPlacement() { failuresLeft++; },
    failNextResize() { failResize = true; },
    async changeDisplays(displays) {
      state.displays = displays;
      assert.ok(poll, 'the display watcher is running');
      poll();
      await setImmediate();
    },
  };
}

test('restores separate layouts after disconnect and reconnect at different scales', async (context) => {
  const panel = await startPanel(context);
  assert.deepEqual(panel.positions, [{ x: -1560, y: 50 }]);
  assert.deepEqual(panel.sizes, [{ width: 480, height: 503 }]);

  await panel.changeDisplays([primary]);
  assert.deepEqual(panel.positions.at(-1), { x: 120, y: 80 });
  assert.deepEqual(panel.sizes.at(-1), { width: 400, height: 350 });

  await panel.changeDisplays([primary, secondary]);
  assert.deepEqual(panel.positions.at(-1), { x: -1560, y: 50 });
  assert.deepEqual(panel.sizes.at(-1), { width: 480, height: 503 });
  assert.deepEqual(panel.writes, []);
  assert.deepEqual(panel.errors, []);
});

test('ignores changes in monitor enumeration order', async (context) => {
  const panel = await startPanel(context);
  await panel.changeDisplays([secondary, primary]);
  assert.deepEqual(panel.positions, [{ x: -1560, y: 50 }]);
  assert.deepEqual(panel.errors, []);
});

test('retries initial placement after a transient display failure', async (context) => {
  const panel = await startPanel(context, { positionFailures: 1 });
  assert.match(panel.errors[0]?.message, /temporarily unavailable/);
  assert.deepEqual(panel.positions, []);

  await panel.changeDisplays([primary, secondary]);
  assert.deepEqual(panel.positions, [{ x: -1560, y: 50 }]);
  assert.equal(panel.errors.length, 1);
});

test('retries the same topology when relocation fails during disconnect', async (context) => {
  const panel = await startPanel(context);
  panel.failNextPlacement();
  await panel.changeDisplays([primary]);
  assert.match(panel.errors[0]?.message, /temporarily unavailable/);
  assert.deepEqual(panel.positions, [{ x: -1560, y: 50 }]);

  await panel.changeDisplays([primary]);
  assert.deepEqual(panel.positions.at(-1), { x: 120, y: 80 });
  assert.deepEqual(panel.writes, []);
});

test('restores the layout after a temporary loss of all monitor information', async (context) => {
  const panel = await startPanel(context);
  await panel.changeDisplays([]);
  assert.equal(panel.positions.length, 1);

  await panel.changeDisplays([primary, secondary]);
  assert.deepEqual(panel.positions, [{ x: -1560, y: 50 }, { x: -1560, y: 50 }]);
  assert.deepEqual(panel.writes, []);
});

test('restores the original layout when a display returns after a partial relocation', async (context) => {
  const panel = await startPanel(context);
  panel.failNextResize();
  await panel.changeDisplays([primary]);
  assert.deepEqual(panel.positions.at(-1), { x: 120, y: 80 });
  assert.match(panel.errors[0]?.message, /Could not resize/);

  await panel.changeDisplays([primary, secondary]);
  assert.deepEqual(panel.positions.at(-1), { x: -1560, y: 50 });
  assert.equal(panel.sizes.length, 2);
  assert.deepEqual(panel.writes, []);
});
