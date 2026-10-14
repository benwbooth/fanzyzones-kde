.pragma library

// The workspace is independent of KWin and pixels. Only the controller holds
// live QObject references; this model can be saved, replayed and tested in Node.
function copy(value) { return JSON.parse(JSON.stringify(value)); }
function create() {
    return { version: 1, nextId: 1, scopes: {}, windows: {}, undo: [], redo: [] };
}
function uid(state) { return "n" + state.nextId++; }
function rect(x, y, width, height) { return { x: x, y: y, width: width, height: height }; }
function unit() { return rect(0, 0, 1, 1); }
function scopeKey(output, desktop) { return JSON.stringify([output, desktop]); }
function ensureScope(state, output, desktop) {
    var key = scopeKey(output, desktop);
    if (!state.scopes[key]) {
        var root = { id: uid(state), kind: "desktop", windows: [] };
        state.scopes[key] = {
            output: output, desktop: desktop, root: root, desktopVisible: true,
            camera: { node: root.id }, back: [], forward: [], views: {}
        };
    }
    return key;
}
function visit(node, callback, parent) {
    if (!node) return;
    callback(node, parent || null);
    var children = node.children || [];
    for (var i = 0; i < children.length; i++) visit(children[i], callback, node);
}
function find(scope, id) {
    var found = null;
    visit(scope.root, function(n) { if (n.id === id) found = n; });
    return found;
}
function parentOf(scope, id) {
    var found = null;
    visit(scope.root, function(n, p) { if (n.id === id) found = p; });
    return found;
}
function desktopNode(scope) {
    var found = null;
    visit(scope.root, function(n) { if (n.kind === "desktop") found = n; });
    return found;
}
function location(state, windowId) {
    var result = null;
    Object.keys(state.scopes).some(function(key) {
        visit(state.scopes[key].root, function(n, p) {
            if (n.kind === "window" && n.window === windowId)
                result = { scope: key, node: n, group: p && p.kind === "tabs" ? p : n };
            if (n.kind === "desktop" && n.windows.indexOf(windowId) >= 0)
                result = { scope: key, node: n, group: n };
        });
        return !!result;
    });
    return result;
}
function scopeOfNode(state, nodeId) {
    return Object.keys(state.scopes).find(function(key) { return !!find(state.scopes[key], nodeId); });
}
function checkpoint(state) {
    state.undo.push(copy({ scopes: state.scopes, windows: state.windows }));
    if (state.undo.length > 30) state.undo.shift();
    state.redo = [];
}
function travelEdit(state, backwards) {
    var from = backwards ? state.undo : state.redo;
    var to = backwards ? state.redo : state.undo;
    if (!from.length) return false;
    to.push(copy({ scopes: state.scopes, windows: state.windows }));
    var previous = from.pop();
    // A layout undo must not resurrect a closed process or forget a new one.
    var live = state.windows;
    state.scopes = previous.scopes;
    state.windows = previous.windows;
    Object.keys(state.windows).forEach(function(id) {
        state.windows[id].present = !!(live[id] && live[id].present);
        if (live[id]) {
            state.windows[id].minimized = live[id].minimized;
            state.windows[id].title = live[id].title;
        }
    });
    Object.keys(live).forEach(function(id) {
        if (!state.windows[id]) addWindow(state, live[id]);
    });
    return true;
}
function serializable(state) {
    var saved = copy(state);
    delete saved.undo;
    delete saved.redo;
    return saved;
}
function restore(value) {
    var state = copy(value);
    state.undo = [];
    state.redo = [];
    validate(state);
    Object.keys(state.windows).forEach(function(id) { state.windows[id].present = false; });
    return state;
}
function addWindow(state, info) {
    if (!info || !info.id) throw new Error("A window needs an identity");
    var key = ensureScope(state, info.output, info.desktop);
    if (state.windows[info.id]) {
        var existing = state.windows[info.id], loc = location(state, info.id);
        Object.assign(existing, info, { present: true, floating: existing.floating, minimized: existing.minimized });
        if (loc) {
            existing.output = state.scopes[loc.scope].output;
            existing.desktop = state.scopes[loc.scope].desktop;
        }
        return info.id;
    }
    // Reuse an absent slot after an application restart. Prefer a matching
    // title/role, then a unique application match; never guess among duplicates.
    var absent = Object.keys(state.windows).filter(function(id) {
        var w = state.windows[id];
        return !w.present && w.appId === info.appId && w.role === info.role;
    });
    var exact = absent.filter(function(id) { return state.windows[id].title === info.title; });
    var oldId = exact.length === 1 ? exact[0] : absent.length === 1 ? absent[0] : null;
    if (oldId) {
        var previous = state.windows[oldId];
        Object.keys(state.scopes).forEach(function(k) {
            visit(state.scopes[k].root, function(n) {
                if (n.kind === "window" && n.window === oldId) n.window = info.id;
                if (n.kind === "desktop") n.windows = n.windows.map(function(id) { return id === oldId ? info.id : id; });
            });
            var scope = state.scopes[k];
            [scope.camera].concat(scope.back, scope.forward, Object.values(scope.views)).forEach(function(camera) {
                if (camera.window === oldId) camera.window = info.id;
            });
        });
        state.windows[info.id] = Object.assign({}, previous, info, {
            present: true, output: previous.output, desktop: previous.desktop,
            floating: previous.floating, minimized: previous.minimized
        });
        delete state.windows[oldId];
    } else {
        state.windows[info.id] = Object.assign({ minimized: false, floating: unit() }, info, { present: true });
        desktopNode(state.scopes[key]).windows.push(info.id);
    }
    return info.id;
}
function removeWindow(state, id, forget) {
    if (!state.windows[id]) return;
    if (!forget) {
        state.windows[id].present = false;
        return;
    }
    takeWindow(state, id);
    delete state.windows[id];
}
function active(state, id) {
    var w = state.windows[id];
    return !!w && w.present && !w.minimized;
}
function tidy(node) {
    if (!node.children) return node;
    var children = [], weights = [];
    node.children.forEach(function(child, i) {
        var next = tidy(child);
        if (next) { children.push(next); weights.push(node.weights ? node.weights[i] : 1); }
    });
    node.children = children;
    if (!children.length) return null;
    if (children.length === 1) return children[0];
    if (node.kind === "split") node.weights = weights;
    if (node.kind === "tabs" && !children.some(function(n) { return n.id === node.active; }))
        node.active = children[0].id;
    return node;
}
function removeNode(scope, id) {
    var p = parentOf(scope, id), node = find(scope, id);
    if (!node || node.kind === "desktop") return null;
    if (!p) return null; // A valid root always contains the Desktop.
    var previous = { root: copy(scope.root) };
    var index = p.children.findIndex(function(n) { return n.id === id; });
    p.children.splice(index, 1);
    if (p.weights) p.weights.splice(index, 1);
    scope.root = tidy(scope.root);
    // A collapsed split may be the current camera or a saved/history view.
    // Retarget it to its surviving space instead of keeping a dead node ID.
    [scope.camera].concat(scope.back, scope.forward, Object.values(scope.views)).forEach(function(camera) {
        if (!camera.node || find(scope, camera.node)) return;
        var old = find(previous, camera.node), survivor = null;
        while (old && !survivor) {
            visit(old, function(n) { if (!survivor) survivor = find(scope, n.id); });
            old = parentOf(previous, old.id);
        }
        camera.node = survivor ? survivor.id : scope.root.id;
    });
    return node;
}
function takeWindow(state, id) {
    var loc = location(state, id);
    if (!loc) return null;
    var scope = state.scopes[loc.scope];
    if (loc.node.kind === "desktop") {
        loc.node.windows = loc.node.windows.filter(function(w) { return w !== id; });
        return { id: uid(state), kind: "window", window: id };
    }
    return removeNode(scope, loc.node.id);
}
function replace(scope, target, replacement) {
    var p = parentOf(scope, target.id);
    if (!p) scope.root = replacement;
    else p.children[p.children.findIndex(function(n) { return n.id === target.id; })] = replacement;
}
function dock(state, id, destination, targetId, edge, wholeGroup) {
    var scope = state.scopes[destination], loc = location(state, id);
    if (!scope || !loc) return false;
    var target = find(scope, targetId);
    if (!target) return false;
    var source = wholeGroup && loc.group.kind === "tabs" ? loc.group : loc.node;
    if (source.kind !== "desktop") {
        var containsTarget = false;
        visit(source, function(n) { if (n.id === targetId) containsTarget = true; });
        if (containsTarget) return false;
    }
    if (edge === "center" && target.kind === "desktop") return floatWindow(state, id, destination);
    if (edge === "center" && target.kind === "split") return false;
    // A leaf in a tab group represents the group for all edge docking.
    var targetParent = parentOf(scope, target.id);
    if (targetParent && targetParent.kind === "tabs") target = targetParent;
    if (source.id === target.id && source.kind !== "desktop") return false;
    checkpoint(state);
    var targetPath = [];
    var ancestor = target;
    while (ancestor) { targetPath.push(ancestor.id); ancestor = parentOf(scope, ancestor.id); }
    var moving = wholeGroup && loc.group.kind === "tabs"
        ? removeNode(state.scopes[loc.scope], loc.group.id) : takeWindow(state, id);
    if (!moving) { state.undo.pop(); return false; }
    // Removing the source can collapse a target's parent or a tab group. Find
    // the surviving target, falling back to its surviving descendants/root.
    target = find(scope, target.id) || (target.children && find(scope, target.children[0].id)) || scope.root;
    if (edge === "center") {
        var tabs = target.kind === "tabs" ? target : { id: uid(state), kind: "tabs", children: [target], active: target.id };
        if (target.kind !== "tabs") replace(scope, target, tabs);
        var arrivals = moving.kind === "tabs" ? moving.children : [moving];
        tabs.children = tabs.children.concat(arrivals);
        tabs.active = arrivals[0].id;
    } else {
        var before = edge === "left" || edge === "top";
        var split = {
            id: uid(state), kind: "split", axis: edge === "left" || edge === "right" ? "x" : "y",
            children: before ? [moving, target] : [target, moving], weights: [1, 1]
        };
        replace(scope, target, split);
    }
    visit(moving, function(n) {
        if (n.kind === "window") {
            state.windows[n.window].output = scope.output;
            state.windows[n.window].desktop = scope.desktop;
            state.windows[n.window].minimized = false;
        }
    });
    // Docking beside the space being viewed must reveal the new split.
    if (scope.camera.node && targetPath.indexOf(scope.camera.node) >= 0)
        scope.camera = { node: scope.root.id };
    return true;
}
function floatWindow(state, id, destination, floatingRect) {
    var loc = location(state, id);
    if (!loc || !state.scopes[destination]) return false;
    checkpoint(state);
    takeWindow(state, id);
    var scope = state.scopes[destination];
    desktopNode(scope).windows.push(id);
    var w = state.windows[id];
    w.output = scope.output;
    w.desktop = scope.desktop;
    w.minimized = false;
    if (floatingRect) w.floating = copy(floatingRect);
    scope.desktopVisible = true;
    return true;
}
function floatAll(state, onlyScope) {
    checkpoint(state);
    Object.keys(state.scopes).forEach(function(key) {
        if (onlyScope && key !== onlyScope) return;
        var scope = state.scopes[key], desktop = desktopNode(scope), ids = [];
        visit(scope.root, function(n) {
            if (n.kind === "window") ids.push(n.window);
            if (n.kind === "desktop") ids = ids.concat(n.windows);
        });
        desktop.windows = ids;
        scope.root = desktop;
        scope.desktopVisible = true;
        scope.camera = { node: desktop.id };
    });
}
function selectedTab(state, node) {
    if (node.kind !== "tabs") return null;
    return node.children.find(function(n) { return n.id === node.active && active(state, n.window); })
        || node.children.find(function(n) { return active(state, n.window); }) || null;
}
function participates(state, scope, node) {
    if (node.kind === "desktop") return scope.desktopVisible;
    if (node.kind === "window") return active(state, node.window);
    return node.children.some(function(n) { return participates(state, scope, n); });
}
function layout(state, key) {
    var scope = state.scopes[key], out = { nodes: {}, windows: [], groups: [], seams: [], desktop: null };
    if (!scope) return out;
    function solve(node, box) {
        out.nodes[node.id] = copy(box);
        if (node.kind === "desktop") {
            out.desktop = { node: node.id, rect: copy(box), windows: node.windows.filter(function(id) { return active(state, id); }) };
        } else if (node.kind === "window") {
            if (active(state, node.window)) out.windows.push({ id: node.window, node: node.id, group: node.id, rect: copy(box) });
        } else if (node.kind === "tabs") {
            var selected = selectedTab(state, node);
            node.children.forEach(function(n) { out.nodes[n.id] = copy(box); });
            if (selected) {
                out.windows.push({ id: selected.window, node: selected.id, group: node.id, rect: copy(box) });
                out.groups.push({ node: node.id, rect: copy(box), active: selected.window,
                    windows: node.children.filter(function(n) { return active(state, n.window); }).map(function(n) { return n.window; }) });
            }
        } else {
            var indices = [];
            node.children.forEach(function(n, i) { if (participates(state, scope, n)) indices.push(i); });
            var total = indices.reduce(function(sum, i) { return sum + node.weights[i]; }, 0), offset = 0;
            indices.forEach(function(index, order) {
                var share = node.weights[index] / total, b = copy(box);
                if (node.axis === "x") { b.x += box.width * offset; b.width *= share; }
                else { b.y += box.height * offset; b.height *= share; }
                solve(node.children[index], b);
                offset += share;
                if (order < indices.length - 1) {
                    out.seams.push({ node: node.id, axis: node.axis, before: index, after: indices[order + 1],
                        rect: node.axis === "x" ? rect(box.x + box.width * offset, box.y, 0, box.height)
                            : rect(box.x, box.y + box.height * offset, box.width, 0), parent: copy(box) });
                }
            });
        }
    }
    solve(scope.root, unit());
    return out;
}
function resize(state, key, nodeId, before, after, delta) {
    var node = find(state.scopes[key], nodeId);
    if (!node || node.kind !== "split" || before === after) return false;
    var sum = node.weights[before] + node.weights[after];
    var total = node.weights.reduce(function(a, b) { return a + b; }, 0);
    var next = Math.max(sum * 0.02, Math.min(sum * 0.98, node.weights[before] + delta * total));
    node.weights[before] = next;
    node.weights[after] = sum - next;
    return true;
}
function cameraRect(state, key, computed, camera) {
    var scope = state.scopes[key];
    camera = camera || scope.camera;
    computed = computed || layout(state, key);
    if (camera.window) {
        var info = state.windows[camera.window], loc = location(state, camera.window);
        var box = loc && computed.nodes[loc.node.id];
        if (info && box && loc.node.kind === "desktop") {
            var f = info.floating || unit();
            return rect(box.x + f.x * box.width, box.y + f.y * box.height, f.width * box.width, f.height * box.height);
        }
    }
    return copy(camera.rect || computed.nodes[camera.node] || unit());
}
function navigate(state, key, camera, remember) {
    var scope = state.scopes[key];
    if (JSON.stringify(scope.camera) === JSON.stringify(camera)) return false;
    if (remember !== false) {
        scope.back.push(copy(scope.camera));
        if (scope.back.length > 100) scope.back.shift();
        scope.forward = [];
    }
    scope.camera = copy(camera);
    return true;
}
function history(state, key, backwards) {
    var scope = state.scopes[key], from = backwards ? scope.back : scope.forward, to = backwards ? scope.forward : scope.back;
    if (!from.length) return false;
    to.push(copy(scope.camera));
    scope.camera = from.pop();
    return true;
}
function stepOut(state, key) {
    var scope = state.scopes[key];
    if (scope.camera.window) return navigate(state, key, { node: desktopNode(scope).id });
    var p = scope.camera.node ? parentOf(scope, scope.camera.node) : null;
    return navigate(state, key, { node: p ? p.id : scope.root.id });
}
function saveView(state, key, slot) {
    if (!Number.isInteger(slot) || slot < 1 || slot > 9) throw new Error("View must be 1 through 9");
    state.scopes[key].views[String(slot)] = copy(state.scopes[key].camera);
}
function loadView(state, key, slot) {
    var camera = state.scopes[key].views[String(slot)];
    return !!camera && navigate(state, key, camera);
}
function project(box, camera, area) {
    return rect(area.x + (box.x - camera.x) / camera.width * area.width,
        area.y + (box.y - camera.y) / camera.height * area.height,
        box.width / camera.width * area.width, box.height / camera.height * area.height);
}
function intersect(a, b) {
    var x = Math.max(a.x, b.x), y = Math.max(a.y, b.y);
    return rect(x, y, Math.max(0, Math.min(a.x + a.width, b.x + b.width) - x),
        Math.max(0, Math.min(a.y + a.height, b.y + b.height) - y));
}
function contains(box, x, y) { return x >= box.x && y >= box.y && x <= box.x + box.width && y <= box.y + box.height; }
function hitTest(state, key, x, y, computed) {
    computed = computed || layout(state, key);
    var result = null;
    visit(state.scopes[key].root, function(n) {
        var box = computed.nodes[n.id];
        if (box && contains(box, x, y)) result = n;
    });
    return result;
}
function neighbour(state, key, nodeId, direction) {
    var computed = layout(state, key), from = computed.nodes[nodeId];
    if (!from) return null;
    var cx = from.x + from.width / 2, cy = from.y + from.height / 2, best = null, score = Infinity;
    var nodes = computed.windows.map(function(w) { return w.group; });
    if (computed.desktop) nodes.push(computed.desktop.node);
    nodes.forEach(function(id) {
        if (id === nodeId) return;
        var b = computed.nodes[id], dx = b.x + b.width / 2 - cx, dy = b.y + b.height / 2 - cy;
        var along = direction === "left" ? -dx : direction === "right" ? dx : direction === "up" ? -dy : dy;
        var across = direction === "left" || direction === "right" ? dy : dx;
        var value = along + Math.abs(across) * 2;
        if (along > 0.00001 && value < score) { score = value; best = id; }
    });
    return best;
}
function validate(state) {
    if (!state || state.version !== 1 || !state.scopes || !state.windows || !Number.isSafeInteger(state.nextId) || state.nextId < 1)
        throw new Error("Unsupported workspace state");
    var nodes = {}, seenWindows = {}, largestId = 0;
    Object.keys(state.scopes).forEach(function(key) {
        var scope = state.scopes[key], desktops = 0;
        if (!scope.camera || !scope.views || !Array.isArray(scope.back) || !Array.isArray(scope.forward))
            throw new Error("Invalid camera state");
        function checkCamera(camera) {
            if (camera.rect) {
                var b = camera.rect;
                if (![b.x, b.y, b.width, b.height].every(Number.isFinite) || b.width <= 0 || b.height <= 0)
                    throw new Error("Invalid camera rectangle");
            } else if (typeof camera.node !== "string" && typeof camera.window !== "string") throw new Error("Invalid camera target");
        }
        checkCamera(scope.camera);
        scope.back.concat(scope.forward).concat(Object.values(scope.views)).forEach(checkCamera);
        visit(scope.root, function(n) {
            if (!n.id || nodes[n.id]) throw new Error("Duplicate node identity");
            nodes[n.id] = true;
            if (/^n\d+$/.test(n.id)) largestId = Math.max(largestId, Number(n.id.slice(1)));
            var ids = [];
            if (n.kind === "desktop") { desktops++; ids = n.windows; }
            else if (n.kind === "window") ids = [n.window];
            else if (n.kind === "split") {
                if (!Array.isArray(n.children) || n.children.length < 2 || !Array.isArray(n.weights)
                    || n.weights.length !== n.children.length || ["x", "y"].indexOf(n.axis) < 0
                    || !n.weights.every(function(w) { return Number.isFinite(w) && w > 0; }))
                    throw new Error("Invalid split");
            } else if (n.kind === "tabs") {
                if (!Array.isArray(n.children) || n.children.length < 2 || !n.children.every(function(c) { return c.kind === "window"; }))
                    throw new Error("Invalid tab group");
            } else throw new Error("Unknown workspace node");
            if (!Array.isArray(ids)) throw new Error("Invalid Desktop members");
            ids.forEach(function(id) {
                if (!state.windows[id] || seenWindows[id]) throw new Error("Duplicate or missing window");
                seenWindows[id] = true;
            });
        });
        if (desktops !== 1) throw new Error("Each workspace must have exactly one Desktop");
    });
    Object.keys(state.windows).forEach(function(id) {
        if (!seenWindows[id]) throw new Error("Window has no home");
    });
    if (state.nextId <= largestId) throw new Error("Invalid next node identity");
    return true;
}
