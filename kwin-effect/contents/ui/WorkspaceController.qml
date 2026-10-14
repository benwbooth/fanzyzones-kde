import QtQuick
import QtQml.Models
import org.kde.kwin
import "../code/Workspace.js" as Engine

Item {
    id: controller
    required property var effect
    property var settings: ({})
    property var model: Engine.create()
    property var clients: ({})
    property var dialogs: ({})
    property var connected: ({})
    property var watchedPanels: ({})
    property var originals: ({})
    property var expectedMinimized: ({})
    property var expectedGeometry: ({})
    property var appColors: ({})
    property var views: []
    property var chrome: []
    property bool ready: false
    enabled: false
    property bool applying: false
    property bool shuttingDown: false
    property bool animating: false
    property int revision: 0
    property string lastAction: ""
    property string errorText: ""
    property string lastFocused: ""
    property string focusAfterLayout: ""
    property var drag: null
    property var dropTarget: null
    property var seamDrag: null
    property var pendingLaunch: null
    property var cameraBeforeGesture: null
    property string gestureScope: ""
    property var gestureTo: null
    property var oldViews: []
    property real transition: 1
    property bool gestureActive: false
    property var nativeBridge: null
    property bool nativeFraming: false
    property point nativeFrameStart
    property rect nativeFrameRect
    property string nativeFrameScope: ""
    property int settlingTicks: 0
    readonly property int duration: settings.nested_animation_ms === undefined ? 460 : settings.nested_animation_ms
    readonly property int headerHeight: 28
    readonly property real gap: settings.nested_gap === undefined ? 8 : settings.nested_gap
    readonly property bool chromeVisible: enabled && ready && !animating && !effect.visible

    Component {
        id: requestFactory
        DBusCall {
            property var callback: null
            property var failure: null
            service: "com.benwbooth.FanzyZones"
            path: "/com/benwbooth/FanzyZones"
            dbusInterface: "com.benwbooth.FanzyZones"
            onFinished: function(reply) {
                if (callback) callback(reply.length ? reply[0] : null);
                destroy();
            }
            onFailed: {
                if (failure) failure();
                else controller.fail("Workspace service could not complete " + method);
                destroy();
            }
        }
    }
    function request(method, args, callback, failure) {
        const rpc = requestFactory.createObject(controller, {
            method: method, arguments: args || [], callback: callback || null, failure: failure || null
        });
        rpc.call();
    }
    function fail(message) {
        errorText = String(message);
        console.warn("FanzyZones workspace: " + errorText);
        report();
    }
    function report() {
        if (shuttingDown) return;
        const displays = views.map(function(v) {
            const scope = model.scopes[v.key];
            return { name: v.name, key: v.key, camera: scope.camera,
                views: Object.keys(scope.views).map(Number), back: scope.back.length, forward: scope.forward.length,
                activeViews: Object.keys(scope.views).filter(function(slot) {
                    return JSON.stringify(scope.views[slot]) === JSON.stringify(scope.camera);
                }).map(Number), root: scope.root.id, desktop: Engine.desktopNode(scope).id,
                windows: v.tiles.filter(function(t) { return t.kind === "window"; }).length };
        });
        request("ReportStatus", [JSON.stringify({ ready: ready, enabled: enabled,
            animating: animating, lastAction: lastAction, error: errorText,
            windows: Object.keys(clients).length, displays: displays, revision: revision,
            presentation: { scene: effect.visible, framing: effect.framing, chrome: chromeVisible,
                chromeCount: chrome.length, seamDragging: !!seamDrag },
            activeWindow: lastFocused, nativeInput: !!(nativeBridge && nativeBridge.available), model: Engine.serializable(model),
            dialogs: Object.keys(dialogs).map(function(id) { return { id: id, parent: dialogs[id].parent }; }),
            geometry: expectedGeometry, hidden: expectedMinimized })], null, function() {});
    }
    function persist() {
        if (!ready || shuttingDown) return;
        saveTimer.restart();
    }
    Timer {
        id: saveTimer
        interval: 250
        onTriggered: controller.saveNow()
    }
    function saveNow() {
        if (!ready) return;
        try {
            Engine.validate(model);
            request("SaveWorkspace", [JSON.stringify({ version: 1, model: Engine.serializable(model), originals: originals, lastAction: lastAction })]);
        } catch (error) { fail("Refusing to save an invalid arrangement: " + error); }
    }
    function readSettings() {
        try { settings = JSON.parse(effect.configuration.settingsJson || "{}"); }
        catch (error) { fail("Invalid workspace settings: " + error); return; }
        const requested = settings.nested_enabled === true;
        if (enabled && !requested) stop();
        else if (ready && requested && !enabled) start();
        else if (ready && requested) rebuild(false);
    }
    function initialize() {
        try {
            nativeBridge = Qt.createQmlObject('import QtQml; import "native" 1.0; InputBridge {}', controller, "native-input");
            nativeBridge.controller = controller;
        } catch (error) { console.info("FanzyZones native input unavailable: " + error); }
        readSettings();
        request("LoadWorkspace", [], function(text) {
            try {
                const saved = JSON.parse(text || "{}");
                if (saved.model) {
                    model = Engine.restore(saved.model);
                    originals = saved.originals || {};
                    lastAction = saved.lastAction || "";
                }
                ready = true;
                if (settings.nested_enabled) start();
                commandTimer.restart();
                report();
            } catch (error) { fail("Workspace could not be restored: " + error); }
        }, function() { fail("Workspace storage is unavailable; windows have not been rearranged"); });
    }
    function start() {
        if (!ready) return;
        enabled = true;
        const all = Workspace.stackingOrder || [];
        for (let i = 0; i < all.length; ++i) adopt(all[i]);
        for (let i = 0; i < all.length; ++i) trackDialog(all[i]);
        rebuild(false);
    }
    function stop() {
        motion.stop();
        settleTimer.stop();
        effect.visible = false;
        animating = false;
        enabled = false;
        nativeFraming = false;
        effect.framing = false;
        drag = null;
        dropTarget = null;
        restoreWindows();
        chrome = [];
        saveNow();
        report();
    }
    function windowId(w) { return w && w.internalId ? w.internalId.toString() : ""; }
    function appColor(id) {
        const info = model.windows[id], w = clients[id];
        if (!info) return "#263447";
        if (!appColors[info.appId]) {
            let hash = 0;
            for (let i = 0; i < info.appId.length; ++i) hash = (hash * 31 + info.appId.charCodeAt(i)) >>> 0;
            const sampled = nativeBridge && w ? nativeBridge.iconColor(w.icon) : "";
            appColors[info.appId] = sampled || Qt.hsla((hash % 360) / 360, .35, .23, 1);
        }
        return appColors[info.appId];
    }
    function outputOf(w) { return w ? (w.output || w.screen || Workspace.activeScreen) : Workspace.activeScreen; }
    function outputId(screen) {
        if (!screen) return "unknown";
        const serial = screen.serialNumber || "";
        return serial ? [screen.manufacturer || "", screen.model || "", serial].join(":") : screen.name;
    }
    function currentDesktop(screen) {
        return Workspace.currentDesktopForScreen ? Workspace.currentDesktopForScreen(screen) : Workspace.currentDesktop;
    }
    function desktopId(desktop) { return desktop && desktop.id ? desktop.id.toString() : "default"; }
    function windowDesktop(w) {
        return w.desktops && w.desktops.length ? w.desktops[0] : currentDesktop(outputOf(w));
    }
    function areaFor(screen) {
        const area = Workspace.clientArea(Workspace.MaximizeArea, screen, currentDesktop(screen));
        return Engine.rect(area.x, area.y, area.width, area.height);
    }
    function rectOf(w) {
        const g = w.frameGeometry;
        return Engine.rect(g.x, g.y, g.width, g.height);
    }
    function acceptable(w) {
        if (!w || !w.normalWindow || w.transient || w.dock || w.desktopWindow || w.popupWindow || w.skipTaskbar) return false;
        const app = String(w.resourceClass || "").toLowerCase();
        if (!app || app === "kwin" || app === "kwin_wayland" || app === "plasmashell" || app === "org.kde.plasmashell") return false;
        const skipped = (settings.skipped_window_classes || []).concat(settings.nested_excluded_apps || []);
        return skipped.indexOf(app) < 0 && String(w.caption || "").indexOf("FanzyZones Workspace") !== 0;
    }
    function snapshotOriginal(w) {
        const screen = outputOf(w);
        return { geometry: rectOf(w), output: outputId(screen),
            desktops: (w.desktops || []).map(function(d) { return desktopId(d); }),
            minimized: w.minimized, noBorder: w.noBorder, keepAbove: w.keepAbove,
            fullScreen: w.fullScreen, maximized: w.maximizeMode || 0 };
    }
    function adopt(w) {
        if (w && w.dock) { watchPanel(w); return; }
        if (enabled && w && w.transient) { trackDialog(w); return; }
        if (!enabled || !acceptable(w)) return;
        const id = windowId(w);
        if (!id) return;
        clients[id] = w;
        if (!originals[id]) originals[id] = snapshotOriginal(w);
        const previous = model.windows[id];
        const g = rectOf(w), area = areaFor(outputOf(w));
        Engine.addWindow(model, { id: id, appId: String(w.resourceClass).toLowerCase(),
            title: String(w.caption || ""), role: String(w.windowRole || ""),
            output: outputId(outputOf(w)), desktop: desktopId(windowDesktop(w)),
            minimized: previous ? previous.minimized : !!w.minimized,
            floating: previous ? previous.floating : Engine.rect((g.x - area.x) / area.width,
                (g.y - area.y) / area.height, g.width / area.width, g.height / area.height) });
        if (!connected[id]) connectWindow(w, id);
        if (pendingLaunch && String(w.resourceClass).toLowerCase().indexOf("dolphin") >= 0) {
            Engine.dock(model, id, pendingLaunch.scope, pendingLaunch.target, pendingLaunch.edge, false);
            const scope = model.scopes[pendingLaunch.scope], loc = Engine.location(model, id);
            const parent = loc ? Engine.parentOf(scope, loc.group.id) : null;
            Engine.navigate(model, pendingLaunch.scope, { node: parent ? parent.id : scope.root.id });
            focusAfterLayout = id;
            pendingLaunch = null;
            launchTimer.stop();
        }
    }
    function watchPanel(w) {
        const id = windowId(w);
        if (watchedPanels[id]) return;
        watchedPanels[id] = true;
        w.frameGeometryChanged.connect(function() { if (controller.enabled) rebuildTimer.restart(); });
        w.closed.connect(function() { delete watchedPanels[id]; if (controller.enabled) rebuildTimer.restart(); });
    }
    function trackDialog(w) {
        if (!enabled || !w || !w.transient || !w.transientFor || w.popupWindow) return;
        const id = windowId(w);
        if (dialogs[id]) return;
        let parent = w.transientFor;
        while (parent && parent.transientFor) parent = parent.transientFor;
        const parentId = windowId(parent);
        if (!clients[parentId]) return;
        if (!originals[id]) originals[id] = snapshotOriginal(w);
        dialogs[id] = { window: w, parent: parentId };
        w.closed.connect(function() { delete dialogs[id]; delete expectedMinimized[id]; });
    }
    function connectWindow(w, id) {
        connected[id] = true;
        w.closed.connect(function() {
            if (!controller) return;
            delete clients[id];
            delete connected[id];
            delete expectedMinimized[id];
            delete expectedGeometry[id];
            Engine.removeWindow(model, id, false);
            if (enabled) rebuild(true);
        });
        w.minimizedChanged.connect(function() {
            if (!controller || !enabled || applying || !model.windows[id]) return;
            if (expectedMinimized[id] === w.minimized) return;
            const wasUserMinimized = model.windows[id].minimized;
            model.windows[id].minimized = !!w.minimized;
            if (!w.minimized) revealWindow(id, wasUserMinimized);
            rebuild(true);
        });
        w.captionChanged.connect(function() {
            if (!controller || !model.windows[id]) return;
            model.windows[id].title = String(w.caption || "");
            if (enabled) rebuild(false);
        });
        w.fullScreenChanged.connect(function() {
            if (!controller || applying || !enabled) return;
            rebuild(false);
        });
        w.interactiveMoveResizeStarted.connect(function() {
            if (!controller || !enabled || applying) return;
            if (w.move) beginDrag(id, false);
        });
        w.interactiveMoveResizeStepped.connect(function() {
            if (!controller || !enabled) return;
            if (drag && drag.id === id) updateDrag();
        });
        w.interactiveMoveResizeFinished.connect(function() {
            if (!controller || !enabled) return;
            if (drag && drag.id === id) finishDrag();
            else updateFloating(id);
        });
        w.desktopsChanged.connect(function() {
            if (!controller || !enabled || applying || !model.windows[id]) return;
            moveScopeIfNeeded(id);
        });
        w.outputChanged.connect(function() {
            if (!controller || !enabled || applying || drag || !model.windows[id]) return;
            moveScopeIfNeeded(id);
        });
    }
    function moveScopeIfNeeded(id) {
        const w = clients[id], loc = Engine.location(model, id);
        if (!w || !loc) return;
        const key = Engine.ensureScope(model, outputId(outputOf(w)), desktopId(windowDesktop(w)));
        if (key === loc.scope) return;
        if (!(Workspace.screens || []).some(function(s) { return outputId(s) === model.scopes[loc.scope].output; })) {
            rebuild(false);
            return;
        }
        if (loc.node.kind === "desktop") Engine.floatWindow(model, id, key);
        else Engine.dock(model, id, key, model.scopes[key].root.id, "right", true);
        rebuild(false);
    }
    function updateFloating(id) {
        const loc = Engine.location(model, id), w = clients[id];
        if (!loc || !w) return;
        const view = viewFor(loc.scope);
        if (loc.node.kind === "desktop" && view && view.desktopArea) {
            const a = view.desktopArea, g = rectOf(w);
            model.windows[id].floating = Engine.rect((g.x - a.x) / a.width, (g.y - a.y) / a.height, g.width / a.width, g.height / a.height);
        }
        rebuild(false);
    }
    function viewFor(key) { return views.find(function(v) { return v.key === key; }) || null; }
    function keyForScreen(screen) {
        return Engine.ensureScope(model, outputId(screen), desktopId(currentDesktop(screen)));
    }
    function activeKey(display) {
        const screens = Workspace.screens || [];
        let screen = null;
        if (display) screen = screens.find(function(s) { return s.name === display || outputId(s) === display; });
        if (display && !screen) throw new Error("Display is not connected: " + display);
        if (!screen) screen = Workspace.screenAt(Workspace.cursorPos) || Workspace.activeScreen || screens[0];
        return screen ? keyForScreen(screen) : null;
    }
    function targetWindow(explicitId) {
        if (explicitId) return clients[explicitId] ? explicitId : null;
        const id = windowId(Workspace.activeWindow);
        if (clients[id]) return id;
        return clients[lastFocused] ? lastFocused : null;
    }
    function revealWindow(id, preserveCamera) {
        const loc = Engine.location(model, id);
        if (!loc) return;
        model.windows[id].minimized = false;
        if (loc.group.kind === "tabs") loc.group.active = loc.node.id;
        if (expectedMinimized[id] && !preserveCamera) Engine.navigate(model, loc.scope, { node: loc.group.id });
    }
    function activate(id) {
        const w = clients[id];
        if (!w) return;
        revealWindow(id);
        rebuild(false);
        Workspace.activeWindow = w;
        lastFocused = id;
    }
    function geometry(box) { return Qt.rect(Math.round(box.x), Math.round(box.y), Math.max(1, Math.round(box.width)), Math.max(1, Math.round(box.height))); }
    function minimumSize(id) {
        const w = clients[id], size = w ? w.minSize : null;
        // Wayland configure replies are asynchronous. A delayed fullscreen or
        // animation resize is not evidence of a minimum size constraint.
        return { width: Math.max(160, size ? size.width : 0), height: Math.max(90, size ? size.height : 0) };
    }
    function clampBox(box, area) {
        const width = Math.min(Math.max(1, box.width), area.width), height = Math.min(Math.max(1, box.height), area.height);
        return Engine.rect(Math.max(area.x, Math.min(area.x + area.width - width, box.x)),
            Math.max(area.y, Math.min(area.y + area.height - height, box.y)), width, height);
    }
    function restoreOne(id) {
        const w = clients[id] || (dialogs[id] && dialogs[id].window), saved = originals[id];
        if (!w || !saved) return;
        try {
            w.tile = null;
            w.noBorder = saved.noBorder;
            w.keepAbove = saved.keepAbove;
            const screens = Workspace.screens || [];
            const screen = screens.find(function(s) { return outputId(s) === saved.output; }) || outputOf(w) || screens[0];
            if (screen) w.frameGeometry = geometry(clampBox(saved.geometry, areaFor(screen)));
            if (w.setMaximize) w.setMaximize((saved.maximized & 2) !== 0, (saved.maximized & 1) !== 0);
            expectedMinimized[id] = !!saved.minimized;
            w.minimized = !!saved.minimized;
        } catch (error) { console.warn("FanzyZones workspace recovery: " + error); }
    }
    function restoreWindows() {
        applying = true;
        Object.keys(clients).forEach(restoreOne);
        Object.keys(dialogs).forEach(restoreOne);
        applying = false;
        expectedGeometry = {};
    }

    function viewItems(screen) {
        const key = keyForScreen(screen), scope = model.scopes[key];
        const computed = Engine.layout(model, key), camera = Engine.cameraRect(model, key, computed);
        const area = areaFor(screen), tiles = [], decorations = [], placements = {}, suppressedSeams = {};
        const view = { key: key, name: screen.name, output: screen, area: area, camera: camera,
            computed: computed, tiles: tiles, chrome: decorations, placements: placements, desktopArea: null };
        const active = Workspace.activeWindow;
        view.blocked = !!(active && active.fullScreen && outputOf(active) === screen);
        function projected(node) {
            const b = computed.nodes[node.id];
            return b ? Engine.project(b, camera, area) : null;
        }
        function members(node) {
            const ids = [];
            Engine.visit(node, function(n) {
                if (n.kind === "window" && Engine.active(model, n.window)) ids.push(n.window);
                if (n.kind === "desktop") n.windows.forEach(function(id) { if (Engine.active(model, id)) ids.push(id); });
            });
            return ids;
        }
        function addTile(node, id, kind, box, ids) {
            const visible = Engine.intersect(box, area);
            if (visible.width < 2 || visible.height < 2) return;
            const tile = { scope: key, node: node.id, id: id || "", kind: kind, rect: box, clipped: visible,
                windows: ids || [], blocked: view.blocked, title: id && model.windows[id] ? model.windows[id].title
                    : kind === "desktop" ? "Desktop" : (ids || []).length + " windows" };
            tiles.push(tile);
            decorations.push(Object.assign({}, tile, { type: kind === "placeholder" ? "placeholder" : "header",
                rect: kind === "placeholder" ? visible : Engine.rect(visible.x + gap / 2, visible.y + gap / 2,
                    Math.max(1, visible.width - gap), Math.min(headerHeight, visible.height)) }));
            return tile;
        }
        function place(id, box, floating) {
            placements[id] = { rect: box, floating: floating, scope: key };
        }
        function solve(node) {
            const box = projected(node);
            if (!box) return;
            const visible = Engine.intersect(box, area);
            if (visible.width < 2 || visible.height < 2) return;
            if (node.kind === "split") {
                if (visible.width < 150 || visible.height < 90) {
                    Engine.visit(node, function(n) { suppressedSeams[n.id] = true; });
                    const ids = members(node);
                    if (ids.length) {
                        const tile = addTile(node, ids[0], "placeholder", box, ids);
                        if (tile) tile.splits = node.children.map(function(n) {
                            const b = computed.nodes[n.id];
                            return b ? Engine.rect((b.x - computed.nodes[node.id].x) / computed.nodes[node.id].width,
                                (b.y - computed.nodes[node.id].y) / computed.nodes[node.id].height,
                                b.width / computed.nodes[node.id].width, b.height / computed.nodes[node.id].height) : Engine.unit();
                        });
                        if (tile) decorations[decorations.length - 1].splits = tile.splits;
                    }
                    return;
                }
                node.children.forEach(solve);
                return;
            }
            if (node.kind === "desktop") {
                const ids = node.windows.filter(function(id) { return Engine.active(model, id); });
                if (visible.width < 180 || visible.height < 120) { addTile(node, "", "placeholder", box, ids); return; }
                addTile(node, "", "desktop", box, ids);
                const a = Engine.rect(visible.x + gap, visible.y + headerHeight + gap,
                    Math.max(1, visible.width - gap * 2), Math.max(1, visible.height - headerHeight - gap * 2));
                view.desktopArea = a;
                ids.forEach(function(id) {
                    const f = model.windows[id].floating || Engine.rect(.1, .1, .6, .6);
                    const b = clampBox(Engine.project(f, Engine.unit(), a), a);
                    const minimum = minimumSize(id);
                    if (b.width >= minimum.width - 2 && b.height >= minimum.height - 2) place(id, b, true);
                    else addTile({ id: node.id + ":" + id }, id, "placeholder", b, [id]);
                    tiles.push({ scope: key, node: node.id, id: id, kind: "floating", rect: b, clipped: b,
                        windows: [id], title: model.windows[id].title });
                });
                return;
            }
            const selected = node.kind === "tabs" ? Engine.selectedTab(model, node) : node;
            if (!selected || !Engine.active(model, selected.window)) return;
            const id = selected.window;
            const ids = node.kind === "tabs" ? node.children.filter(function(n) { return Engine.active(model, n.window); }).map(function(n) { return n.window; }) : [id];
            const minimum = minimumSize(id);
            const content = Engine.rect(visible.x + gap / 2, visible.y + gap / 2 + headerHeight,
                Math.max(1, visible.width - gap), Math.max(1, visible.height - gap - headerHeight));
            const small = content.width < minimum.width - 2 || content.height < minimum.height - 2;
            addTile(node, id, small ? "placeholder" : "window", box, ids);
            if (!small) place(id, content, false);
        }
        if (scope.camera.window && Engine.active(model, scope.camera.window) && clients[scope.camera.window]) {
            const id = scope.camera.window, loc = Engine.location(model, id);
            addTile(loc.group, id, "window", area, [id]);
            place(id, Engine.rect(area.x + gap / 2, area.y + headerHeight + gap / 2,
                area.width - gap, area.height - headerHeight - gap), false);
            return view;
        }
        solve(scope.root);
        computed.seams.forEach(function(seam) {
            if (suppressedSeams[seam.node]) return;
            const b = Engine.project(seam.rect, camera, area);
            const region = seam.axis === "x" ? Engine.rect(b.x - Math.max(6, gap) / 2, b.y, Math.max(6, gap), b.height)
                : Engine.rect(b.x, b.y - Math.max(6, gap) / 2, b.width, Math.max(6, gap));
            const clipped = Engine.intersect(region, area);
            if (clipped.width >= 2 && clipped.height >= 2) decorations.push({ type: "seam", scope: key,
                node: seam.node, axis: seam.axis, before: seam.before, after: seam.after,
                parent: seam.parent, rect: clipped, blocked: view.blocked });
        });
        return view;
    }
    function calculateViews() {
        const screens = Workspace.screens || [], result = [], decorations = [];
        for (let i = 0; i < screens.length; ++i) result.push(viewItems(screens[i]));
        // An unplugged output retains its arrangement. Its live windows remain
        // reachable on a connected display until that output comes back.
        const presentOutputs = screens.map(outputId), desktops = (Workspace.desktops || []).map(desktopId);
        if (result.length) {
            Object.keys(model.windows).forEach(function(id) {
                if (!Engine.active(model, id) || !clients[id]) return;
                const loc = Engine.location(model, id);
                if (!loc) return;
                const source = model.scopes[loc.scope];
                if (presentOutputs.indexOf(source.output) >= 0 && desktops.indexOf(source.desktop) >= 0) return;
                const v = result[0], saved = originals[id];
                const box = clampBox(saved ? saved.geometry : rectOf(clients[id]), v.area);
                v.placements[id] = { rect: box, floating: true, scope: v.key };
                v.tiles.push({ scope: v.key, node: loc.node.id, id: id, kind: "floating", rect: box,
                    clipped: box, windows: [id], title: model.windows[id].title });
            });
        }
        result.forEach(function(v) { decorations.push.apply(decorations, v.chrome); });
        views = result;
        chrome = decorations;
        revision++;
    }
    function rebuild(animate) {
        if (!ready || !enabled) return;
        if (gestureActive) return;
        if (animate && !seamDrag && duration > 0 && views.length) {
            motion.stop(); settleTimer.stop();
            oldViews = views;
            calculateViews();
            transition = 0;
            animating = true;
            effect.visible = true;
            Qt.callLater(function() { if (controller.enabled) motion.restart(); });
        } else {
            calculateViews();
            if (!animating) applyLayout();
        }
        persist();
        report();
    }
    function applyLayout() {
        if (!enabled) return;
        applying = true;
        const placements = {}, activeScopes = {};
        views.forEach(function(view) {
            activeScopes[view.key] = true;
            Object.keys(view.placements).forEach(function(id) { placements[id] = view.placements[id]; });
        });
        const newGeometry = {};
        Object.keys(clients).forEach(function(id) {
            const w = clients[id], info = model.windows[id], loc = Engine.location(model, id);
            if (!info || !loc || !w || w.fullScreen) return;
            const placement = placements[id];
            if (!activeScopes[loc.scope] && !placement) return;
            const hidden = !placement || info.minimized;
            expectedMinimized[id] = hidden;
            try {
                if (!hidden) {
                    if (w.tile) w.tile = null;
                    if (w.setMaximize && w.maximizeMode) w.setMaximize(false, false);
                    w.noBorder = placement.floating ? (originals[id] ? originals[id].noBorder : false) : true;
                    newGeometry[id] = placement.rect;
                    w.frameGeometry = geometry(placement.rect);
                }
                if (w.minimized !== hidden) w.minimized = hidden;
            } catch (error) { console.warn("FanzyZones window placement: " + error); }
        });
        Object.keys(dialogs).forEach(function(id) {
            const dialog = dialogs[id], w = dialog.window, parent = clients[dialog.parent];
            if (!parent) { restoreOne(id); delete dialogs[id]; return; }
            const loc = Engine.location(model, dialog.parent);
            if (!loc || !activeScopes[loc.scope]) return;
            const placement = placements[dialog.parent];
            const hidden = !parent.fullScreen && (!placement || !!expectedMinimized[dialog.parent]);
            expectedMinimized[id] = hidden;
            w.minimized = hidden;
            if (!hidden) {
                w.desktops = parent.desktops;
                const box = parent.fullScreen ? rectOf(parent) : placement.rect, current = rectOf(w);
                const targetView = placement ? viewFor(placement.scope) : null;
                const area = targetView ? targetView.area : areaFor(outputOf(parent));
                w.frameGeometry = geometry(clampBox(Engine.rect(box.x + (box.width - current.width) / 2,
                    box.y + (box.height - current.height) / 2, current.width, current.height), area));
            }
        });
        expectedGeometry = newGeometry;
        if (focusAfterLayout && clients[focusAfterLayout] && placements[focusAfterLayout]) {
            Workspace.activeWindow = clients[focusAfterLayout];
            lastFocused = focusAfterLayout;
        }
        focusAfterLayout = "";
        applying = false;
    }
    NumberAnimation {
        id: motion
        target: controller
        property: "transition"
        from: 0
        to: 1
        duration: controller.duration
        easing.type: Easing.InOutCubic
        onFinished: {
            controller.applyLayout();
            controller.settlingTicks = 0;
            settleTimer.restart();
        }
    }
    Timer {
        id: settleTimer
        interval: 16
        repeat: true
        onTriggered: {
            controller.settlingTicks++;
            const ids = Object.keys(controller.expectedGeometry);
            const settled = ids.every(function(id) {
                const w = controller.clients[id], r = controller.expectedGeometry[id];
                if (!w) return true;
                const g = w.frameGeometry;
                return Math.abs(g.width - r.width) < 2 && Math.abs(g.height - r.height) < 2;
            });
            if (settled || controller.settlingTicks >= 25) {
                stop();
                controller.animating = false;
                controller.effect.visible = false;
                controller.oldViews = [];
                controller.report();
                rebuildTimer.restart();
            }
        }
    }
    function tileStartRect(tile, key) {
        const view = oldViews.find(function(v) { return v.key === key; });
        if (!view) return tile.rect;
        const old = view.tiles.find(function(t) { return t.node === tile.node && t.id === tile.id && t.kind === tile.kind; })
            || view.tiles.find(function(t) { return tile.id && t.id === tile.id; });
        return old ? old.rect : (clients[tile.id] ? rectOf(clients[tile.id]) : tile.rect);
    }
    function focusNode(key, node) {
        const scope = model.scopes[key];
        if (scope.camera.node === node && scope.back.length) Engine.history(model, key, true);
        else Engine.navigate(model, key, { node: node });
        chooseFocus(key);
        rebuild(true);
    }
    function focusWindow(id) {
        const loc = Engine.location(model, id);
        if (!loc) return;
        if (loc.node.kind !== "desktop") { focusNode(loc.scope, loc.group.id); return; }
        const scope = model.scopes[loc.scope];
        if (scope.camera.window === id && scope.back.length) Engine.history(model, loc.scope, true);
        else Engine.navigate(model, loc.scope, { window: id });
        focusAfterLayout = id;
        rebuild(true);
    }
    function chooseFocus(key) {
        const scope = model.scopes[key];
        if (!scope) return;
        if (scope.camera.window) { focusAfterLayout = scope.camera.window; return; }
        if (!scope.camera.node) return;
        const node = Engine.find(scope, scope.camera.node);
        if (!node) return;
        const choices = [];
        function add(n) {
            if (n.kind === "window" && Engine.active(model, n.window)) choices.push(n.window);
            else if (n.kind === "tabs") {
                const selected = Engine.selectedTab(model, n);
                if (selected) add(selected);
            } else if (n.kind === "desktop") {
                n.windows.forEach(function(id) { if (Engine.active(model, id)) choices.push(id); });
            } else (n.children || []).forEach(add);
        }
        add(node);
        const active = windowId(Workspace.activeWindow);
        focusAfterLayout = choices.indexOf(active) >= 0 ? active : choices[0] || "";
    }
    function selectTab(key, nodeId, id) {
        const node = Engine.find(model.scopes[key], nodeId);
        if (node && node.kind === "tabs") {
            const leaf = node.children.find(function(n) { return n.window === id; });
            if (leaf) node.active = leaf.id;
        }
        if (model.windows[id]) model.windows[id].minimized = false;
        rebuild(false);
        if (clients[id]) Workspace.activeWindow = clients[id];
    }
    function beginSeam(entry) {
        const node = Engine.find(model.scopes[entry.scope], entry.node);
        if (!node) return;
        Engine.checkpoint(model);
        seamDrag = { entry: entry, start: Qt.point(Workspace.cursorPos.x, Workspace.cursorPos.y), weights: node.weights.slice() };
    }
    function updateSeam() {
        if (!seamDrag) return;
        const e = seamDrag.entry, view = viewFor(e.scope);
        if (!view) return;
        const n = Engine.find(model.scopes[e.scope], e.node);
        n.weights = seamDrag.weights.slice();
        const pixels = e.axis === "x" ? Workspace.cursorPos.x - seamDrag.start.x : Workspace.cursorPos.y - seamDrag.start.y;
        const size = e.axis === "x" ? e.parent.width / view.camera.width * view.area.width : e.parent.height / view.camera.height * view.area.height;
        Engine.resize(model, e.scope, e.node, e.before, e.after, pixels / size);
        rebuild(false);
    }
    function endSeam() { seamDrag = null; persist(); }
    function beginDrag(id, manual, wholeGroup) {
        if (!clients[id]) return;
        motion.stop(); settleTimer.stop();
        animating = false;
        effect.visible = false;
        drag = { id: id, manual: !!manual, wholeGroup: !!wholeGroup,
            start: Qt.point(Workspace.cursorPos.x, Workspace.cursorPos.y),
            geometry: rectOf(clients[id]), candidate: "", since: Date.now() };
        dropTarget = null;
    }
    function updateDrag() {
        if (!drag) return;
        const point = Workspace.cursorPos;
        const screen = Workspace.screenAt(point) || Workspace.activeScreen;
        if (!screen) return;
        const key = keyForScreen(screen), view = viewFor(key);
        if (!view) return;
        const a = view.area, camera = view.camera;
        let target = null, edge = "", region = null;
        const margin = 30;
        if (point.x - a.x < margin) edge = "left";
        else if (a.x + a.width - point.x < margin) edge = "right";
        else if (point.y - a.y < margin) edge = "top";
        else if (a.y + a.height - point.y < margin) edge = "bottom";
        if (edge) {
            target = Engine.find(model.scopes[key], model.scopes[key].camera.node) || model.scopes[key].root;
            region = a;
        } else {
            const x = camera.x + (point.x - a.x) / a.width * camera.width;
            const y = camera.y + (point.y - a.y) / a.height * camera.height;
            target = Engine.hitTest(model, key, x, y, view.computed);
            if (target) {
                const parent = Engine.parentOf(model.scopes[key], target.id);
                if (parent && parent.kind === "tabs") target = parent;
                region = Engine.intersect(Engine.project(view.computed.nodes[target.id], camera, a), a);
                const rx = (point.x - region.x) / region.width, ry = (point.y - region.y) / region.height;
                const distances = [rx, 1 - rx, ry, 1 - ry], smallest = Math.min.apply(Math, distances);
                edge = smallest < .22 ? ["left", "right", "top", "bottom"][distances.indexOf(smallest)] : "center";
            }
        }
        if (!target || !region) { dropTarget = null; return; }
        const loc = Engine.location(model, drag.id);
        if (loc && target.id === (drag.wholeGroup ? loc.group.id : loc.node.id) && target.kind !== "desktop") {
            dropTarget = null;
            return;
        }
        const candidate = key + ":" + target.id + ":" + edge;
        if (candidate !== drag.candidate) { drag.candidate = candidate; drag.since = Date.now(); }
        const tab = edge === "center" && target.kind !== "desktop";
        const armed = !tab || Date.now() - drag.since >= 300;
        let box = Engine.copy(region);
        if (edge === "left") box.width /= 2;
        if (edge === "right") { box.x += box.width / 2; box.width /= 2; }
        if (edge === "top") box.height /= 2;
        if (edge === "bottom") { box.y += box.height / 2; box.height /= 2; }
        dropTarget = { scope: key, node: target.id, edge: edge, rect: box, armed: armed,
            label: tab ? (armed ? "Group as tabs" : "Hold to group as tabs") : edge === "center" ? "Float on Desktop" : "Dock " + edge };
    }
    Timer {
        interval: 40
        repeat: true
        running: controller.drag !== null
        onTriggered: controller.updateDrag()
    }
    function finishDrag(cancel) {
        if (!drag) return;
        const moving = drag, target = dropTarget;
        drag = null; dropTarget = null;
        if (cancel) { rebuild(false); return; }
        const id = moving.id, w = clients[id];
        if (!w) return;
        let key = activeKey();
        if (target && target.armed) {
            key = target.scope;
            Engine.dock(model, id, key, target.node, target.edge, moving.wholeGroup);
        } else {
            const a = viewFor(key).desktopArea || viewFor(key).area;
            let box = rectOf(w);
            if (moving.manual) box = Engine.rect(Workspace.cursorPos.x - Math.min(box.width / 2, 180),
                Workspace.cursorPos.y - 15, Math.min(box.width, a.width * .8), Math.min(box.height, a.height * .8));
            Engine.floatWindow(model, id, key, Engine.rect((box.x - a.x) / a.width, (box.y - a.y) / a.height,
                box.width / a.width, box.height / a.height));
        }
        const scope = model.scopes[key];
        const desktop = (Workspace.desktops || []).find(function(d) { return desktopId(d) === scope.desktop; });
        if (desktop) { applying = true; w.desktops = [desktop]; applying = false; }
        rebuild(true);
    }
    function frameRectangle(key, pixelRect) {
        const view = viewFor(key);
        if (!view || pixelRect.width < 12 || pixelRect.height < 12) return;
        const r = Engine.intersect(pixelRect, view.area), a = view.area, c = view.camera;
        Engine.navigate(model, key, { rect: Engine.rect(c.x + (r.x - a.x) / a.width * c.width,
            c.y + (r.y - a.y) / a.height * c.height, r.width / a.width * c.width, r.height / a.height * c.height) });
        rebuild(true);
    }
    function nativeDesktopAt() {
        if (!enabled || animating || effect.visible) return false;
        const point = Workspace.cursorPos, view = viewFor(activeKey());
        if (!view || !view.desktopArea || !Engine.contains(view.desktopArea, point.x, point.y)) return false;
        return !(Workspace.stackingOrder || []).some(function(w) {
            return !w.minimized && !w.desktopWindow && !w.dock && !w.deleted
                && (!w.desktops.length || w.desktops.indexOf(currentDesktop(outputOf(w))) >= 0)
                && Engine.contains(rectOf(w), point.x, point.y);
        });
    }
    function desktopDoubleClick() {
        const key = activeKey();
        focusNode(key, Engine.desktopNode(model.scopes[key]).id);
    }
    function beginNativeFrame() {
        if (!enabled) return;
        nativeFrameScope = activeKey();
        nativeFrameStart = Workspace.cursorPos;
        nativeFrameRect = Qt.rect(nativeFrameStart.x, nativeFrameStart.y, 0, 0);
        nativeFraming = true;
        effect.framing = true;
        effect.visible = true;
    }
    function updateNativeFrame() {
        if (!nativeFraming) return;
        const p = Workspace.cursorPos, s = nativeFrameStart;
        nativeFrameRect = Qt.rect(Math.min(s.x, p.x), Math.min(s.y, p.y), Math.abs(p.x - s.x), Math.abs(p.y - s.y));
    }
    function endNativeFrame(commit) {
        if (!nativeFraming) return;
        updateNativeFrame();
        nativeFraming = false;
        effect.framing = false;
        if (commit) frameRectangle(nativeFrameScope, nativeFrameRect);
        if (!animating) effect.visible = false;
    }
    function cancelGesture() {
        if (!gestureActive) return;
        transition = 0;
        endGesture(false);
    }
    function gesture(direction, progress) {
        if (!enabled || !ready) return;
        if (!gestureActive) {
            const key = activeKey(), scope = model.scopes[key];
            if (!scope) return;
            gestureScope = key;
            cameraBeforeGesture = Engine.copy(scope.camera);
            oldViews = views;
            if (direction === "out") {
                const parent = scope.camera.node ? Engine.parentOf(scope, scope.camera.node) : null;
                gestureTo = { node: parent ? parent.id : scope.root.id };
            } else {
                const view = viewFor(key), point = Workspace.cursorPos;
                const node = Engine.hitTest(model, key, view.camera.x + (point.x - view.area.x) / view.area.width * view.camera.width,
                    view.camera.y + (point.y - view.area.y) / view.area.height * view.camera.height, view.computed);
                gestureTo = { node: node ? node.id : scope.root.id };
            }
            if (JSON.stringify(gestureTo) === JSON.stringify(cameraBeforeGesture)) return;
            motion.stop(); settleTimer.stop();
            scope.camera = Engine.copy(gestureTo);
            calculateViews();
            animating = true;
            gestureActive = true;
            effect.visible = true;
        }
        transition = Math.max(0, Math.min(1, progress));
    }
    function endGesture(triggered) {
        if (!gestureActive) return;
        const scope = model.scopes[gestureScope];
        const commit = triggered || transition >= .5;
        scope.camera = Engine.copy(cameraBeforeGesture);
        if (commit) Engine.navigate(model, gestureScope, gestureTo);
        gestureActive = false;
        cameraBeforeGesture = null;
        // Start the settling animation at the actual in-flight camera position.
        const start = oldViews;
        oldViews = views.map(function(v) {
            const before = start.find(function(s) { return s.key === v.key; });
            if (!before) return v;
            const interpolated = Object.assign({}, v);
            interpolated.tiles = v.tiles.map(function(t) {
                const b = tileStartRect(t, v.key), end = t.rect, p = transition;
                return Object.assign({}, t, { rect: Engine.rect(b.x + (end.x - b.x) * p, b.y + (end.y - b.y) * p,
                    b.width + (end.width - b.width) * p, b.height + (end.height - b.height) * p) });
            });
            return interpolated;
        });
        calculateViews();
        transition = 0;
        motion.restart();
        persist();
    }
    function processCommand() {
        if (!ready) return;
        let payload;
        try { payload = JSON.parse(effect.configuration.pendingActionJson || "{}"); }
        catch (error) { fail("Invalid workspace command: " + error); return; }
        if (!payload.action || !payload.nonce || payload.nonce === lastAction) return;
        lastAction = payload.nonce;
        execute(payload);
    }
    function execute(payload) {
        errorText = "";
        try {
            const action = payload.action;
            if (action === "enable") { start(); report(); return; }
            if (action === "disable") { stop(); report(); return; }
            if (!enabled) throw new Error("Nested workspaces are disabled");
            const key = activeKey(payload.display), scope = model.scopes[key];
            if (!scope) throw new Error("No connected workspace");
            const id = targetWindow(payload.window), loc = id ? Engine.location(model, id) : null;
            let animate = true;
            if (action === "arrange") {
                const ids = Engine.desktopNode(scope).windows.filter(function(id) { return Engine.active(model, id); });
                ids.forEach(function(w, i) {
                    if (Engine.active(model, w)) {
                        const target = i && Engine.location(model, ids[i - 1]) ? Engine.location(model, ids[i - 1]).group.id : scope.root.id;
                        Engine.dock(model, w, key, target, i % 2 ? "bottom" : "left", false);
                    }
                });
                Engine.navigate(model, key, { node: scope.root.id });
            } else if (action === "float-all") {
                Engine.floatAll(model, payload.display ? key : null);
            } else if (action === "show-all") Engine.navigate(model, key, { node: scope.root.id });
            else if (action === "desktop") {
                scope.desktopVisible = true;
                Engine.navigate(model, key, { node: Engine.desktopNode(scope).id });
            } else if (action === "toggle-desktop") {
                Engine.checkpoint(model);
                scope.desktopVisible = !scope.desktopVisible;
                Engine.navigate(model, key, { node: scope.root.id });
            } else if (action === "back" || action === "forward") Engine.history(model, key, action === "back");
            else if (action === "zoom-out") Engine.stepOut(model, key);
            else if (action === "zoom-in") {
                const view = viewFor(key), p = Workspace.cursorPos;
                const node = Engine.hitTest(model, key, view.camera.x + (p.x - view.area.x) / view.area.width * view.camera.width,
                    view.camera.y + (p.y - view.area.y) / view.area.height * view.camera.height, view.computed);
                if (node) Engine.navigate(model, key, { node: node.id });
            } else if (action === "frame") {
                if (payload.rect) {
                    const r = payload.rect;
                    if (![r.x, r.y, r.width, r.height].every(Number.isFinite) || r.width <= 0 || r.height <= 0)
                        throw new Error("Invalid frame rectangle");
                    Engine.navigate(model, key, { rect: r });
                } else {
                    effect.framing = true;
                    effect.visible = true;
                    return;
                }
            } else if (action === "save-view" || action === "load-view") {
                if (!Number.isInteger(payload.slot) || payload.slot < 1 || payload.slot > 9) throw new Error("View must be 1 through 9");
                if (action === "save-view") { Engine.saveView(model, key, payload.slot); animate = false; }
                else if (!Engine.loadView(model, key, payload.slot)) throw new Error("That view has not been saved");
            } else if (action === "undo" || action === "redo") Engine.travelEdit(model, action === "undo");
            else if (["left", "right", "up", "down"].indexOf(action) >= 0) {
                const from = scope.camera.node && scope.camera.node !== scope.root.id ? scope.camera.node : loc ? loc.group.id : Engine.desktopNode(scope).id;
                const target = Engine.neighbour(model, key, from, action);
                if (target) Engine.navigate(model, key, { node: target });
            } else if (action === "new-window") {
                pendingLaunch = { scope: key, target: loc && loc.scope === key ? loc.group.id : scope.root.id, edge: "right" };
                launchTimer.restart();
                request("OpenFileManager", []);
                return;
            } else if (action === "exclude-app" || action === "include-app") {
                const app = String(payload.app || (id && model.windows[id] ? model.windows[id].appId : "")).trim().toLowerCase();
                if (!app) throw new Error("Choose an application first");
                let excluded = (settings.nested_excluded_apps || []).filter(function(a) { return a !== app; });
                if (action === "exclude-app") excluded.push(app);
                settings.nested_excluded_apps = excluded;
                Object.keys(clients).forEach(function(w) {
                    if (model.windows[w] && model.windows[w].appId === app && action === "exclude-app") {
                        applying = true; restoreOne(w); applying = false;
                        Engine.removeWindow(model, w, true);
                        delete clients[w];
                    }
                });
                if (action === "include-app") (Workspace.stackingOrder || []).forEach(adopt);
                request("InvokeAction", [JSON.stringify({ action: "updateSettings", patch: { nested_excluded_apps: excluded } })]);
            } else {
                if (!id || !loc) throw new Error("No managed window is focused");
                if (action === "focus") {
                    focusWindow(id);
                    return;
                } else if (action === "float" || (action === "toggle-float" && loc.node.kind !== "desktop")) {
                    Engine.floatWindow(model, id, key);
                    Engine.navigate(model, key, { node: Engine.desktopNode(scope).id });
                } else if (["dock", "toggle-float", "split-horizontal", "split-vertical"].indexOf(action) >= 0) {
                    let destination = key;
                    let target = payload.target || scope.camera.node || scope.root.id;
                    const targetWindow = payload.target ? Engine.location(model, payload.target) : null;
                    if (targetWindow) { destination = targetWindow.scope; target = targetWindow.group.id; }
                    if (target === loc.group.id) target = scope.root.id;
                    const edge = payload.edge || (action === "split-vertical" ? "bottom" : "right");
                    if (["left", "right", "top", "bottom", "center"].indexOf(edge) < 0) throw new Error("Invalid docking edge");
                    if (!Engine.dock(model, id, destination, target, edge, !!payload.group)) throw new Error("Cannot dock into that space");
                } else if (action === "next-tab" || action === "previous-tab") {
                    if (loc.group.kind === "tabs") {
                        const tabs = loc.group.children.filter(function(n) { return Engine.active(model, n.window); });
                        const index = tabs.findIndex(function(n) { return n.id === loc.group.active; });
                        const next = tabs[(index + (action === "next-tab" ? 1 : tabs.length - 1)) % tabs.length];
                        if (next) { selectTab(loc.scope, loc.group.id, next.window); return; }
                    }
                    animate = false;
                } else if (action === "close-tab") { clients[id].closeWindow(); return; }
                else throw new Error("Unknown workspace action: " + action);
            }
            Engine.validate(model);
            if (["left", "right", "up", "down", "back", "forward", "load-view", "zoom-in", "zoom-out", "desktop"].indexOf(action) >= 0)
                chooseFocus(key);
            rebuild(animate);
        } catch (error) { fail(error); }
        finally { report(); }
    }
    Timer {
        id: launchTimer
        interval: 10000
        onTriggered: controller.pendingLaunch = null
    }
    Timer {
        id: rebuildTimer
        interval: 100
        onTriggered: if (controller.enabled) controller.rebuild(false)
    }
    Timer {
        id: commandTimer
        interval: 0
        onTriggered: controller.processCommand()
    }
    Connections {
        target: controller.effect
        function onConfigurationChanged() { controller.readSettings(); commandTimer.restart(); }
    }
    Connections {
        target: Workspace
        function onWindowAdded(w) {
            if (!controller.ready || !controller.enabled) return;
            controller.adopt(w);
            rebuildTimer.restart();
        }
        function onWindowActivated(w) {
            const id = controller.windowId(w);
            if (controller.clients[id]) controller.lastFocused = id;
        }
        function onCurrentDesktopChanged() { if (controller.enabled) rebuildTimer.restart(); }
        function onScreensChanged() { if (controller.enabled) rebuildTimer.restart(); }
        function onVirtualScreenSizeChanged() { if (controller.enabled) rebuildTimer.restart(); }
    }
    Instantiator {
        model: Workspace.screens
        delegate: Connections {
            required property var modelData
            target: modelData
            function onGeometryChanged() { if (controller.enabled) rebuildTimer.restart(); }
            function onScaleChanged() { if (controller.enabled) rebuildTimer.restart(); }
        }
    }
    Component.onCompleted: initialize()
    Component.onDestruction: {
        shuttingDown = true;
        restoreWindows();
    }
}
