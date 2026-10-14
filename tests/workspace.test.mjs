import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const E = vm.createContext({});
vm.runInContext(readFileSync(new URL('../kwin-effect/contents/code/Workspace.js', import.meta.url), 'utf8').replace('.pragma library', ''), E);
const plain = value => JSON.parse(JSON.stringify(value));
function fixture(count = 4) {
    const state = E.create();
    for (let i = 0; i < count; i++) E.addWindow(state, {
        id: `w${i}`, appId: `app${i}`, role: '', title: `Window ${i}`,
        output: 'DP-1', desktop: 'desk-1', floating: E.rect(.1, .1, .6, .6)
    });
    return [state, E.scopeKey('DP-1', 'desk-1')];
}
function dock(state, key, id, target, edge = 'left', group = false) {
    assert.equal(E.dock(state, id, key, target, edge, group), true);
    assert.equal(E.validate(state), true);
}

test('docking nests spaces while preserving a floating Desktop', () => {
    const [s, k] = fixture();
    dock(s, k, 'w0', s.scopes[k].root.id);
    dock(s, k, 'w1', E.location(s, 'w0').node.id, 'bottom');
    const l = E.layout(s, k);
    assert.equal(l.windows.length, 2);
    assert.deepEqual(plain(l.desktop.rect), { x: .5, y: 0, width: .5, height: 1 });
    assert.deepEqual(plain(l.windows.find(w => w.id === 'w1').rect), { x: 0, y: .5, width: .5, height: .5 });
    assert.deepEqual(plain(l.desktop.windows), ['w2', 'w3']);
});
test('cross-app tabs select, tear off, and move as a group', () => {
    const [s, k] = fixture();
    dock(s, k, 'w0', s.scopes[k].root.id);
    dock(s, k, 'w1', E.location(s, 'w0').node.id, 'center');
    assert.equal(E.layout(s, k).groups[0].active, 'w1');
    dock(s, k, 'w1', E.desktopNode(s.scopes[k]).id, 'right', true);
    assert.equal(E.layout(s, k).groups[0].windows.length, 2);
    E.floatWindow(s, 'w1', k);
    assert.equal(E.layout(s, k).groups.length, 0);
    assert.equal(E.location(s, 'w1').node.kind, 'desktop');
    assert.equal(E.location(s, 'w0').node.kind, 'window');
    E.validate(s);
});
test('minimize reflows without losing the original split or weight', () => {
    const [s, k] = fixture(2);
    dock(s, k, 'w0', s.scopes[k].root.id);
    dock(s, k, 'w1', E.location(s, 'w0').node.id, 'bottom');
    const before = plain(E.layout(s, k).windows);
    s.windows.w0.minimized = true;
    assert.equal(E.layout(s, k).windows[0].rect.height, 1);
    s.windows.w0.minimized = false;
    assert.deepEqual(plain(E.layout(s, k).windows), before);
});
test('removing a split retargets current, saved and history views to surviving spaces', () => {
    const [s, k] = fixture(2);
    dock(s, k, 'w0', s.scopes[k].root.id);
    const split = s.scopes[k].root.id;
    E.navigate(s, k, { node: split });
    E.saveView(s, k, 1);
    E.navigate(s, k, { node: E.location(s, 'w0').node.id });
    E.history(s, k, true);
    E.removeWindow(s, 'w0', true);
    const scope = s.scopes[k];
    for (const camera of [scope.camera, ...scope.back, ...scope.forward, ...Object.values(scope.views)]) {
        assert.ok(E.find(scope, camera.node));
    }
    dock(s, k, 'w1', scope.camera.node);
});
test('minimizing an active tab selects its neighbour and collapses an empty group', () => {
    const [s, k] = fixture(2);
    dock(s, k, 'w0', s.scopes[k].root.id);
    dock(s, k, 'w1', E.location(s, 'w0').node.id, 'center');
    s.windows.w1.minimized = true;
    assert.equal(E.layout(s, k).groups[0].active, 'w0');
    s.windows.w0.minimized = true;
    assert.equal(E.layout(s, k).desktop.rect.width, 1);
});
test('seam resizing changes only adjacent siblings', () => {
    const [s, k] = fixture(3);
    dock(s, k, 'w0', s.scopes[k].root.id);
    dock(s, k, 'w1', E.location(s, 'w0').node.id, 'bottom');
    const before = E.layout(s, k), seam = before.seams.find(x => x.axis === 'y');
    E.resize(s, k, seam.node, seam.before, seam.after, .2);
    const after = E.layout(s, k);
    assert.deepEqual(plain(after.desktop.rect), plain(before.desktop.rect));
    assert.equal(after.windows[0].rect.height, .7);
    assert(Math.abs(after.windows[1].rect.height - .3) < 1e-10);
    E.resize(s, k, seam.node, seam.before, seam.after, -100);
    assert(E.layout(s, k).windows[0].rect.height > 0);
});
test('camera hierarchy, rectangle framing, history and all nine saved views', () => {
    const [s, k] = fixture(2);
    dock(s, k, 'w0', s.scopes[k].root.id);
    const node = E.location(s, 'w0').node.id;
    E.navigate(s, k, { node });
    assert.equal(E.cameraRect(s, k).width, .5);
    for (let i = 1; i <= 9; i++) E.saveView(s, k, i);
    E.stepOut(s, k);
    assert.equal(E.cameraRect(s, k).width, 1);
    assert.equal(E.history(s, k, true), true);
    assert.equal(s.scopes[k].camera.node, node);
    E.navigate(s, k, { rect: E.rect(.1, .2, .6, .4) });
    assert.equal(E.loadView(s, k, 9), true);
    assert.equal(s.scopes[k].camera.node, node);
    assert.throws(() => E.saveView(s, k, 10));
});
test('display and virtual-desktop cameras are independent', () => {
    const [s, k] = fixture(1), other = E.ensureScope(s, 'HDMI-1', 'desk-1');
    const next = E.ensureScope(s, 'DP-1', 'desk-2');
    E.navigate(s, other, { rect: E.rect(.1, 0, .3, 1) });
    assert.equal(E.cameraRect(s, k).width, 1);
    assert.equal(E.cameraRect(s, next).width, 1);
    dock(s, other, 'w0', s.scopes[other].root.id);
    assert.equal(s.windows.w0.output, 'HDMI-1');
    assert.equal(E.location(s, 'w0').scope, other);
});
test('restart reconnects surviving identities and unique reopened windows', () => {
    const [s, k] = fixture(2);
    dock(s, k, 'w0', s.scopes[k].root.id);
    const node = E.location(s, 'w0').node.id;
    const restored = E.restore(E.serializable(s));
    assert.equal(E.layout(restored, k).windows.length, 0);
    E.addWindow(restored, { ...s.windows.w0, id: 'new0' });
    assert.equal(E.location(restored, 'new0').node.id, node);
    assert.equal(restored.windows.w0, undefined);
    E.validate(restored);
});
test('ambiguous reopened application windows get a new floating slot', () => {
    const [s, k] = fixture(2);
    s.windows.w1.appId = s.windows.w0.appId;
    const restored = E.restore(E.serializable(s));
    E.addWindow(restored, { ...s.windows.w0, id: 'new', title: 'Unrelated document' });
    assert.equal(Object.keys(restored.windows).length, 3);
    assert.equal(E.location(restored, 'new').node.kind, 'desktop');
});
test('reopened windows keep their saved display and floating geometry', () => {
    const [s, k] = fixture(1), other = E.ensureScope(s, 'HDMI-1', 'desk-1');
    E.floatWindow(s, 'w0', other, E.rect(.2, .3, .4, .5));
    const restored = E.restore(E.serializable(s));
    E.addWindow(restored, { ...s.windows.w0, id: 'reopened', output: 'DP-1', floating: E.unit() });
    assert.equal(E.location(restored, 'reopened').scope, other);
    assert.equal(restored.windows.reopened.output, 'HDMI-1');
    assert.deepEqual(plain(restored.windows.reopened.floating), { x: .2, y: .3, width: .4, height: .5 });
    E.validate(restored);
});
test('a floating window can be focused, saved, and reconnected without docking', () => {
    const [s, k] = fixture(1);
    E.navigate(s, k, { window: 'w0' });
    E.saveView(s, k, 2);
    assert.deepEqual(plain(E.cameraRect(s, k)), { x: .1, y: .1, width: .6, height: .6 });
    const restored = E.restore(E.serializable(s));
    E.addWindow(restored, { ...s.windows.w0, id: 'reopened' });
    assert.equal(restored.scopes[k].camera.window, 'reopened');
    assert.equal(restored.scopes[k].views['2'].window, 'reopened');
    E.stepOut(restored, k);
    assert.equal(restored.scopes[k].camera.node, E.desktopNode(restored.scopes[k]).id);
    assert.equal(E.location(restored, 'reopened').node.kind, 'desktop');
    E.validate(restored);
});
test('undo/redo does not resurrect closed processes or lose newly opened windows', () => {
    const [s, k] = fixture(1);
    dock(s, k, 'w0', s.scopes[k].root.id);
    E.addWindow(s, { ...s.windows.w0, id: 'new', appId: 'another' });
    E.removeWindow(s, 'w0', false);
    assert.equal(E.travelEdit(s, true), true);
    assert.equal(s.windows.w0.present, false);
    assert.equal(s.windows.new.present, true);
    E.validate(s);
    assert.equal(E.travelEdit(s, false), true);
    E.validate(s);
});
test('float-all retains windows and gives every display a full Desktop', () => {
    const [s, k] = fixture();
    dock(s, k, 'w0', s.scopes[k].root.id);
    dock(s, k, 'w1', E.location(s, 'w0').node.id, 'center');
    E.floatAll(s);
    assert.equal(s.scopes[k].root.kind, 'desktop');
    assert.equal(s.scopes[k].root.windows.length, 4);
    E.validate(s);
});
test('invalid snapshots fail closed', () => {
    const [s] = fixture();
    assert.throws(() => E.restore({ version: 2 }));
    s.scopes[Object.keys(s.scopes)[0]].camera = { rect: { x: 0, y: 0, width: -1, height: 1 } };
    assert.throws(() => E.restore(s));
});
test('random docking, tabs, minimize, detach and undo preserve ownership', () => {
    const [s, k] = fixture(12);
    const other = E.ensureScope(s, 'HDMI-1', 'desk-1');
    let seed = 731;
    const rand = n => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed % n; };
    for (let i = 0; i < 1200; i++) {
        const id = `w${rand(12)}`, key = rand(2) ? k : other;
        const nodes = [];
        E.visit(s.scopes[key].root, n => nodes.push(n.id));
        switch (rand(7)) {
        case 0: E.floatWindow(s, id, key); break;
        case 1: s.windows[id].minimized = !s.windows[id].minimized; break;
        case 2: E.travelEdit(s, true); break;
        default: E.dock(s, id, key, nodes[rand(nodes.length)], ['left', 'right', 'top', 'bottom', 'center'][rand(5)], !!rand(2));
        }
        E.validate(s);
        for (const scope of [k, other]) {
            for (const w of E.layout(s, scope).windows)
                assert(w.rect.width > 0 && w.rect.height > 0);
        }
    }
});
