// FanzyZones KDE KWin script.
import QtQuick
import org.kde.kwin
import org.kde.plasma.core as PlasmaCore
import org.kde.plasma.components as PlasmaComponents

Item {
    id: root

    property var settings: ({})
    property int currentLayout: 0
    property var scopedLayouts: ({})
    property var connectedWindows: ({})
    property var savedWindows: ({})
    property var managedDesktops: []
    property bool moving: false
    property bool moved: false
    property bool overlayForced: false
    property var movingWindow: null
    property int highlightedZone: -1
    property var activeArea: Qt.rect(0, 0, 1, 1)
    property var activeScreen: null
    property var lastDesktopBeforeSwitch: null

    function log(message) {
        if (settings.debug)
            print("FanzyZones KDE: " + message);
    }

    function defaultLayouts() {
        return [
            {
                "id": "builtin.two-panes",
                "name": "Two Panes",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Left", "x": 0, "y": 0, "width": 0.5, "height": 1},
                    {"id": 1, "name": "Right", "x": 0.5, "y": 0, "width": 0.5, "height": 1}
                ]
            },
            {
                "id": "builtin.two-panes-wide",
                "name": "Two Panes (Wide + Side)",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Main", "x": 0, "y": 0, "width": 0.7, "height": 1},
                    {"id": 1, "name": "Side", "x": 0.7, "y": 0, "width": 0.3, "height": 1}
                ]
            },
            {
                "id": "builtin.three-panes",
                "name": "Three Panes",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Left", "x": 0, "y": 0, "width": 0.3333333333, "height": 1},
                    {"id": 1, "name": "Center", "x": 0.3333333333, "y": 0, "width": 0.3333333333, "height": 1},
                    {"id": 2, "name": "Right", "x": 0.6666666667, "y": 0, "width": 0.3333333333, "height": 1}
                ]
            },
            {
                "id": "builtin.three-panes-ultrawide",
                "name": "Three Panes (Ultrawide)",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Left", "x": 0, "y": 0, "width": 0.25, "height": 1},
                    {"id": 1, "name": "Center", "x": 0.25, "y": 0, "width": 0.5, "height": 1},
                    {"id": 2, "name": "Right", "x": 0.75, "y": 0, "width": 0.25, "height": 1}
                ]
            },
            {
                "id": "builtin.quarters",
                "name": "Quarters",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Top-Left", "x": 0, "y": 0, "width": 0.5, "height": 0.5},
                    {"id": 1, "name": "Top-Right", "x": 0.5, "y": 0, "width": 0.5, "height": 0.5},
                    {"id": 2, "name": "Bottom-Left", "x": 0, "y": 0.5, "width": 0.5, "height": 0.5},
                    {"id": 3, "name": "Bottom-Right", "x": 0.5, "y": 0.5, "width": 0.5, "height": 0.5}
                ]
            },
            {
                "id": "builtin.priority-left",
                "name": "Priority (Left Focus)",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Focus", "x": 0, "y": 0, "width": 0.6, "height": 1},
                    {"id": 1, "name": "Top-Right", "x": 0.6, "y": 0, "width": 0.4, "height": 0.5},
                    {"id": 2, "name": "Bottom-Right", "x": 0.6, "y": 0.5, "width": 0.4, "height": 0.5}
                ]
            },
            {
                "id": "builtin.grid-3x3",
                "name": "Grid 3x3",
                "is_built_in": true,
                "padding": 0,
                "zones": [
                    {"id": 0, "name": "Zone 1", "x": 0, "y": 0, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 1, "name": "Zone 2", "x": 0.3333333333, "y": 0, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 2, "name": "Zone 3", "x": 0.6666666667, "y": 0, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 3, "name": "Zone 4", "x": 0, "y": 0.3333333333, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 4, "name": "Zone 5", "x": 0.3333333333, "y": 0.3333333333, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 5, "name": "Zone 6", "x": 0.6666666667, "y": 0.3333333333, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 6, "name": "Zone 7", "x": 0, "y": 0.6666666667, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 7, "name": "Zone 8", "x": 0.3333333333, "y": 0.6666666667, "width": 0.3333333333, "height": 0.3333333333},
                    {"id": 8, "name": "Zone 9", "x": 0.6666666667, "y": 0.6666666667, "width": 0.3333333333, "height": 0.3333333333}
                ]
            }
        ];
    }

    function loadSettings() {
        const defaults = {
            "version": 1,
            "snap_mode": "modifier",
            "modifiers": ["shift"],
            "active_layout": 0,
            "gap": 0,
            "outer_padding": 0,
            "enable_zone_overlay": true,
            "enable_zone_selector": true,
            "enable_edge_snapping": false,
            "remember_window_geometries": true,
            "keyboard_shortcuts_enabled": true,
            "highlight_color": {"red": 0.18, "green": 0.48, "blue": 0.96},
            "overlay_opacity": 0.35,
            "show_zone_numbers": true,
            "track_layout_per_screen": false,
            "track_layout_per_desktop": false,
            "auto_snap_new_windows": false,
            "dynamic_workspaces": true,
            "keep_empty_middle_desktops": false,
            "macsimize_fullscreen": true,
            "macsimize_maximized": true,
            "macsimize_move_to_last_desktop": false,
            "macsimize_exclusive_desktops": true,
            "skipped_window_classes": [
                "krunner",
                "ksmserver",
                "ksmserver-logout-greeter",
                "ksplashqml",
                "kwin",
                "kwin_wayland",
                "org.kde.plasmashell",
                "org.kde.spectacle",
                "org.kde.yakuake",
                "plasmashell"
            ],
            "debug": false,
            "layouts": defaultLayouts()
        };

        let parsed = {};
        try {
            parsed = JSON.parse(KWin.readConfig("settingsJson", "{}"));
        } catch (error) {
            print("FanzyZones KDE: could not parse settingsJson: " + error);
        }

        settings = Object.assign({}, defaults, parsed);
        if (!settings.layouts || settings.layouts.length === 0)
            settings.layouts = defaultLayouts();
        if (settings.active_layout < 0 || settings.active_layout >= settings.layouts.length)
            settings.active_layout = 0;
        currentLayout = settings.active_layout;
    }

    function windows() {
        if (Workspace.windowList)
            return Workspace.windowList();
        return Workspace.stackingOrder || [];
    }

    function windowId(client) {
        if (!client)
            return "";
        if (client.internalId)
            return client.internalId.toString();
        if (client.windowId)
            return client.windowId.toString();
        return String(client.caption) + ":" + String(client.resourceClass);
    }

    function windowClass(client) {
        if (!client || !client.resourceClass)
            return "";
        return client.resourceClass.toString().toLowerCase();
    }

    function windowCaption(client) {
        if (!client || !client.caption)
            return "";
        return client.caption.toString().toLowerCase();
    }

    function isFanzyZonesWindow(client) {
        const caption = root.windowCaption(client);
        return caption === "fanzyzones" || caption.indexOf("fanzyzones ") === 0;
    }

    function isSkippedWindow(client) {
        if (!client)
            return true;
        if (root.isFanzyZonesWindow(client))
            return true;
        if (!client.normalWindow || client.skipTaskbar || client.popupWindow || client.desktopWindow || client.dock)
            return true;
        const klass = windowClass(client);
        return !klass || settings.skipped_window_classes.indexOf(klass) !== -1;
    }

    function windowsInStackingOrder() {
        if (Workspace.stackingOrder)
            return Workspace.stackingOrder;
        if (Workspace.windowList)
            return Workspace.windowList();
        return [];
    }

    function isCandidateWindow(client) {
        if (root.isSkippedWindow(client))
            return false;
        if (client.minimized || client.hidden || client.hiddenByShowDesktop)
            return false;
        if (client.onAllDesktops)
            return true;
        if (!client.desktops || client.desktops.length === 0)
            return true;
        return client.desktops.indexOf(Workspace.currentDesktop) !== -1;
    }

    function targetWindow() {
        const active = Workspace.activeWindow;
        if (root.isCandidateWindow(active))
            return active;

        const all = root.windowsInStackingOrder();
        for (let i = all.length - 1; i >= 0; i--) {
            const client = all[i];
            if (client === active)
                continue;
            if (root.isCandidateWindow(client))
                return client;
        }
        return null;
    }

    function refreshArea(client) {
        activeScreen = client && client.screen ? client.screen : Workspace.activeScreen;
        try {
            activeArea = Workspace.clientArea(KWin.MaximizeArea, activeScreen, Workspace.currentDesktop);
        } catch (error) {
            activeArea = Workspace.clientArea(KWin.FullScreenArea, activeScreen, Workspace.currentDesktop);
        }
    }

    function layoutKey() {
        const parts = [];
        if (settings.track_layout_per_screen && Workspace.activeScreen)
            parts.push(Workspace.activeScreen.name);
        if (settings.track_layout_per_desktop && Workspace.currentDesktop)
            parts.push(Workspace.currentDesktop.id || Workspace.currentDesktop.name);
        return parts.join(":");
    }

    function activeLayoutIndex() {
        const key = layoutKey();
        if (key.length > 0) {
            if (scopedLayouts[key] === undefined)
                scopedLayouts[key] = currentLayout;
            return scopedLayouts[key];
        }
        return currentLayout;
    }

    function setActiveLayout(index) {
        if (index < 0 || index >= settings.layouts.length)
            return;
        const key = layoutKey();
        if (key.length > 0)
            scopedLayouts[key] = index;
        currentLayout = index;
        highlightedZone = -1;
    }

    function activeLayout() {
        return settings.layouts[root.activeLayoutIndex()];
    }

    function zoneRect(zone, area) {
        return root.zoneRectForLayout(root.activeLayout(), zone, area);
    }

    function zoneRectForLayout(layout, zone, area) {
        const padding = Math.max(0, settings.outer_padding || 0) + Math.max(0, layout.padding || 0);
        const gap = Math.max(0, settings.gap || 0);
        const usableX = area.x + padding;
        const usableY = area.y + padding;
        const usableWidth = Math.max(1, area.width - padding * 2);
        const usableHeight = Math.max(1, area.height - padding * 2);
        return Qt.rect(
            Math.round(usableX + usableWidth * zone.x + gap / 2),
            Math.round(usableY + usableHeight * zone.y + gap / 2),
            Math.max(1, Math.round(usableWidth * zone.width - gap)),
            Math.max(1, Math.round(usableHeight * zone.height - gap))
        );
    }

    function contains(rect, point) {
        return point.x >= rect.x && point.x <= rect.x + rect.width && point.y >= rect.y && point.y <= rect.y + rect.height;
    }

    function modifierMask(name) {
        if (name === "shift")
            return Qt.ShiftModifier;
        if (name === "control")
            return Qt.ControlModifier;
        if (name === "alt")
            return Qt.AltModifier;
        if (name === "meta")
            return Qt.MetaModifier;
        return 0;
    }

    function modifiersSatisfied() {
        if (settings.snap_mode === "auto")
            return true;
        if (!settings.modifiers || settings.modifiers.length === 0)
            return false;
        for (let i = 0; i < settings.modifiers.length; i++) {
            const mask = modifierMask(settings.modifiers[i]);
            if (mask !== 0 && (Qt.keyboardModifiers & mask) !== mask)
                return false;
        }
        return true;
    }

    function snapActiveWindowToZone(index) {
        root.moveClientToZone(root.targetWindow(), index);
    }

    function moveClientToZone(client, index) {
        return root.moveClientToLayoutZone(client, root.activeLayoutIndex(), index);
    }

    function moveClientToLayoutZone(client, layoutIndex, index) {
        if (root.isSkippedWindow(client))
            return false;
        const layout = settings.layouts[layoutIndex];
        if (!layout || index < 0 || index >= layout.zones.length)
            return false;
        root.refreshArea(client);
        const rect = root.zoneRectForLayout(layout, layout.zones[index], activeArea);
        if (client.setMaximize)
            client.setMaximize(false, false);
        client.frameGeometry = rect;
        client.fanzyZone = index;
        client.fanzyLayout = layoutIndex;
        client.fanzyDesktop = Workspace.currentDesktop;
        return true;
    }

    function nearestZoneIndex(client) {
        if (root.isSkippedWindow(client))
            return -1;
        root.refreshArea(client);
        const layout = root.activeLayout();
        const center = Qt.point(
            client.frameGeometry.x + client.frameGeometry.width / 2,
            client.frameGeometry.y + client.frameGeometry.height / 2
        );
        let best = -1;
        let bestDistance = Number.POSITIVE_INFINITY;
        for (let i = 0; i < layout.zones.length; i++) {
            const rect = root.zoneRect(layout.zones[i], activeArea);
            const dx = rect.x + rect.width / 2 - center.x;
            const dy = rect.y + rect.height / 2 - center.y;
            const distance = Math.sqrt(dx * dx + dy * dy);
            if (distance < bestDistance) {
                best = i;
                bestDistance = distance;
            }
        }
        return best;
    }

    function snapClientToClosestZone(client) {
        const index = root.nearestZoneIndex(client);
        if (index >= 0)
            return root.moveClientToZone(client, index);
        return false;
    }

    function cycleActiveWindow(delta) {
        const client = root.targetWindow();
        if (root.isSkippedWindow(client))
            return;
        const layout = root.activeLayout();
        let current = client.fanzyLayout === root.activeLayoutIndex() ? client.fanzyZone : root.nearestZoneIndex(client);
        if (current < 0)
            current = 0;
        const target = (current + delta + layout.zones.length) % layout.zones.length;
        root.moveClientToZone(client, target);
    }

    function snapAllWindows() {
        const all = root.windows();
        for (let i = 0; i < all.length; i++)
            root.snapClientToClosestZone(all[i]);
    }

    function clearPendingAction() {
        try {
            KWin.writeConfig("pendingActionJson", "{}");
        } catch (error) {
            log("could not clear pending action: " + error);
        }
    }

    function processPendingAction() {
        let action = {};
        try {
            action = JSON.parse(KWin.readConfig("pendingActionJson", "{}"));
        } catch (error) {
            print("FanzyZones KDE: could not parse pendingActionJson: " + error);
            root.clearPendingAction();
            return;
        }

        if (!action || !action.action)
            return;

        if (action.action === "reloadSettings") {
            root.loadSettings();
        } else if (action.action === "setLayout") {
            root.setActiveLayout(action.layout || 0);
        } else if (action.action === "snap") {
            root.moveClientToLayoutZone(root.targetWindow(), action.layout || 0, action.zone || 0);
        }

        root.clearPendingAction();
    }

    function refreshOverlayVisibility() {
        root.updateHighlightedZone();
        if (settings.snap_mode === "modifier")
            overlayContent.visible = root.modifiersSatisfied() || overlayForced;
    }

    function updateHighlightedZone() {
        if (!movingWindow)
            return;
        root.refreshArea(movingWindow);
        highlightedZone = -1;
        const cursor = Workspace.cursorPos;
        const layout = root.activeLayout();
        const selectorHeight = 86;

        if (settings.enable_zone_selector && cursor.y >= activeArea.y && cursor.y <= activeArea.y + selectorHeight) {
            const layoutWidth = activeArea.width / Math.max(1, settings.layouts.length);
            const layoutIndex = Math.min(settings.layouts.length - 1, Math.max(0, Math.floor((cursor.x - activeArea.x) / layoutWidth)));
            root.setActiveLayout(layoutIndex);
            const selectedLayout = root.activeLayout();
            const localX = cursor.x - activeArea.x - layoutWidth * layoutIndex;
            for (let i = 0; i < selectedLayout.zones.length; i++) {
                const z = selectedLayout.zones[i];
                const mini = Qt.rect(
                    activeArea.x + layoutWidth * layoutIndex + z.x * layoutWidth,
                    activeArea.y + 24 + z.y * 56,
                    Math.max(1, z.width * layoutWidth),
                    Math.max(1, z.height * 56)
                );
                if (root.contains(mini, cursor)) {
                    highlightedZone = i;
                    return;
                }
            }
            if (localX >= 0)
                highlightedZone = root.nearestZoneIndex(movingWindow);
            return;
        }

        for (let i = 0; i < layout.zones.length; i++) {
            const rect = root.zoneRect(layout.zones[i], activeArea);
            if (root.contains(rect, cursor)) {
                highlightedZone = i;
                return;
            }
        }

        if (settings.enable_edge_snapping) {
            const d = 24;
            if (cursor.x <= activeArea.x + d || cursor.x >= activeArea.x + activeArea.width - d ||
                cursor.y <= activeArea.y + d || cursor.y >= activeArea.y + activeArea.height - d) {
                highlightedZone = root.nearestZoneIndex(movingWindow);
            }
        }
    }

    function saveOriginalGeometry(client) {
        if (!settings.remember_window_geometries || !client)
            return;
        const id = root.windowId(client);
        if (!savedWindows[id])
            savedWindows[id] = {};
        if (client.fanzyZone === undefined || client.fanzyZone < 0) {
            savedWindows[id].oldGeometry = {
                "x": client.frameGeometry.x,
                "y": client.frameGeometry.y,
                "width": client.frameGeometry.width,
                "height": client.frameGeometry.height
            };
        }
    }

    function connectWindow(client) {
        if (!client || connectedWindows[root.windowId(client)])
            return;
        connectedWindows[root.windowId(client)] = true;

        if (client.onInteractiveMoveResizeStarted) {
            client.onInteractiveMoveResizeStarted.connect(function() {
                if (!root)
                    return;
                if (root.isSkippedWindow(client) || !client.move)
                    return;
                root.moving = true;
                root.moved = false;
                root.movingWindow = client;
                root.highlightedZone = -1;
                root.saveOriginalGeometry(client);
                root.refreshArea(client);
                overlay.showOverlay();
            });
            client.onInteractiveMoveResizeStepped.connect(function() {
                if (!root)
                    return;
                if (root.moving && root.movingWindow === client) {
                    root.moved = true;
                    root.refreshOverlayVisibility();
                }
            });
            client.onInteractiveMoveResizeFinished.connect(function() {
                if (!root)
                    return;
                if (root.moving && root.movingWindow === client && root.moved && (root.modifiersSatisfied() || root.overlayForced) && root.highlightedZone >= 0)
                    root.moveClientToZone(client, root.highlightedZone);
                root.moving = false;
                root.moved = false;
                root.overlayForced = false;
                root.movingWindow = null;
                root.highlightedZone = -1;
                overlay.hideOverlay();
            });
        }

        if (client.onFullScreenChanged) {
            client.onFullScreenChanged.connect(function() {
                if (!root)
                    return;
                root.handleFullscreenChanged(client);
            });
        }
        if (client.maximizedAboutToChange) {
            client.maximizedAboutToChange.connect(function(mode) {
                if (!root)
                    return;
                root.handleMaximizedChanged(client, mode);
            });
        }
        if (client.minimizedChanged) {
            client.minimizedChanged.connect(function() {
                if (!root)
                    return;
                root.handleMinimizedChanged(client);
            });
        }
        if (client.captionChanged) {
            client.captionChanged.connect(function() {
                if (!root)
                    return;
                root.renameManagedDesktop(client);
            });
        }
        if (client.desktopsChanged) {
            client.desktopsChanged.connect(function() {
                if (!root)
                    return;
                root.ensureTrailingEmptyDesktop(client);
            });
        }
        if (client.closed) {
            client.closed.connect(function() {
                if (!root)
                    return;
                root.restoreMacsimizedWindow(client);
                delete root.connectedWindows[root.windowId(client)];
                delete root.savedWindows[root.windowId(client)];
            });
        }
    }

    function desktopIndex(desktop) {
        if (!desktop)
            return -1;
        return Workspace.desktops.indexOf(desktop);
    }

    function clientOnDesktop(client, desktop) {
        return client && client.desktops && client.desktops.indexOf(desktop) !== -1;
    }

    function desktopHasWindows(desktop) {
        const all = root.windows();
        for (let i = 0; i < all.length; i++) {
            const client = all[i];
            if (root.clientOnDesktop(client, desktop) && !client.skipPager && !client.onAllDesktops)
                return true;
        }
        return false;
    }

    function ensureTrailingEmptyDesktop(client) {
        if (!settings.dynamic_workspaces || !Workspace.desktops || Workspace.desktops.length === 0)
            return;
        const last = Workspace.desktops[Workspace.desktops.length - 1];
        if (client && root.clientOnDesktop(client, last) && !client.skipPager)
            Workspace.createDesktop(Workspace.desktops.length, "Desktop " + (Workspace.desktops.length + 1));
        else if (root.desktopHasWindows(last))
            Workspace.createDesktop(Workspace.desktops.length, "Desktop " + (Workspace.desktops.length + 1));
    }

    function pruneEmptyDesktops(oldDesktop) {
        if (!settings.dynamic_workspaces || !Workspace.desktops || Workspace.desktops.length <= 2)
            return;
        const currentIndex = root.desktopIndex(Workspace.currentDesktop);
        for (let i = Workspace.desktops.length - 2; i > currentIndex && i > 0; i--) {
            const desktop = Workspace.desktops[i];
            if (!root.desktopHasWindows(desktop)) {
                Workspace.removeDesktop(desktop);
            } else if (settings.keep_empty_middle_desktops) {
                break;
            }
        }
        root.ensureTrailingEmptyDesktop(null);
    }

    function managedDesktopIndex(desktop) {
        for (let i = 0; i < managedDesktops.length; i++) {
            if (managedDesktops[i] === desktop)
                return i;
        }
        return -1;
    }

    function createDesktopForWindow(client) {
        if (root.isSkippedWindow(client))
            return;
        const id = root.windowId(client);
        if (savedWindows[id] && savedWindows[id].macsimized)
            return;
        if (!settings.macsimize_fullscreen && client.fullScreen)
            return;
        if (!settings.macsimize_maximized && !client.fullScreen)
            return;

        const insertAt = settings.macsimize_move_to_last_desktop
            ? Workspace.desktops.length
            : Math.max(1, root.desktopIndex(Workspace.currentDesktop) + 1);
        savedWindows[id] = Object.assign({}, savedWindows[id] || {}, {
            "macsimized": true,
            "desktops": client.desktops ? client.desktops.slice(0) : [Workspace.currentDesktop],
            "resourceClass": windowClass(client)
        });
        Workspace.createDesktop(insertAt, client.caption || "Full Screen");
        const desktop = Workspace.desktops[insertAt];
        managedDesktops.push(desktop);
        client.desktops = [desktop];
        Workspace.currentDesktop = desktop;
    }

    function restoreMacsimizedWindow(client) {
        const id = root.windowId(client);
        const saved = savedWindows[id];
        if (!saved || !saved.macsimized || !client.desktops || client.desktops.length === 0)
            return;
        const desktop = client.desktops[0];
        client.desktops = saved.desktops && saved.desktops.length > 0 ? saved.desktops : [Workspace.desktops[0]];
        const idx = root.managedDesktopIndex(desktop);
        if (idx >= 0)
            managedDesktops.splice(idx, 1);
        if (!root.desktopHasWindows(desktop))
            Workspace.removeDesktop(desktop);
        saved.macsimized = false;
    }

    function handleFullscreenChanged(client) {
        if (settings.macsimize_fullscreen && client.fullScreen)
            root.createDesktopForWindow(client);
        else
            root.restoreMacsimizedWindow(client);
    }

    function handleMaximizedChanged(client, mode) {
        const id = root.windowId(client);
        if (!savedWindows[id])
            savedWindows[id] = {};
        savedWindows[id].windowMode = mode;
        if (settings.macsimize_maximized && mode === 3)
            root.createDesktopForWindow(client);
        else if (!client.fullScreen)
            root.restoreMacsimizedWindow(client);
    }

    function handleMinimizedChanged(client) {
        const saved = savedWindows[root.windowId(client)];
        if (!saved || !saved.macsimized)
            return;
        if (client.minimized)
            root.restoreMacsimizedWindow(client);
        else if (saved.windowMode === 3 || client.fullScreen)
            root.createDesktopForWindow(client);
    }

    function renameManagedDesktop(client) {
        const saved = savedWindows[root.windowId(client)];
        if (saved && saved.macsimized && client.desktops && client.desktops.length > 0)
            client.desktops[0].name = client.caption || client.desktops[0].name;
    }

    function handleNewWindow(client) {
        if (!client)
            return;
        root.connectWindow(client);
        if (root.isSkippedWindow(client))
            return;

        if (client.transient && client.transientFor) {
            const parentSaved = savedWindows[root.windowId(client.transientFor)];
            if (parentSaved && parentSaved.macsimized) {
                client.desktops = client.transientFor.desktops;
                return;
            }
        }

        if (settings.macsimize_exclusive_desktops && root.managedDesktopIndex(Workspace.currentDesktop) >= 0 &&
            !client.fullScreen && !(client.maximizeMode === 3)) {
            client.desktops = [Workspace.desktops[0]];
            Workspace.currentDesktop = Workspace.desktops[0];
        }

        if (settings.auto_snap_new_windows)
            root.snapClientToClosestZone(client);
        root.ensureTrailingEmptyDesktop(client);
    }

    Component.onCompleted: {
        root.loadSettings();
        const all = root.windows();
        for (let i = 0; i < all.length; i++)
            root.handleNewWindow(all[i]);
        if (Workspace.windowAdded)
            Workspace.windowAdded.connect(function(client) {
                if (!root)
                    return;
                root.handleNewWindow(client);
            });
        if (Workspace.currentDesktopChanged)
            Workspace.currentDesktopChanged.connect(function(oldDesktop) {
                if (!root)
                    return;
                root.pruneEmptyDesktops(oldDesktop);
            });
        root.ensureTrailingEmptyDesktop(null);
        log("loaded with " + settings.layouts.length + " layouts");
    }

    PlasmaCore.Dialog {
        id: overlay

        function showOverlay() {
            root.refreshArea(movingWindow);
            visible = true;
            setWidth(Workspace.virtualScreenSize.width);
            setHeight(Workspace.virtualScreenSize.height);
        }

        function hideOverlay() {
            visible = false;
        }

        title: "FanzyZones KDE Overlay"
        location: PlasmaCore.Types.Desktop
        type: PlasmaCore.Dialog.OnScreenDisplay
        backgroundHints: PlasmaCore.Types.NoBackground
        flags: Qt.BypassWindowManagerHint | Qt.FramelessWindowHint | Qt.Popup
        hideOnWindowDeactivate: false
        outputOnly: true
        visible: false
        opacity: 1
        width: Workspace.virtualScreenSize.width
        height: Workspace.virtualScreenSize.height

        Item {
            id: overlayContent
            anchors.fill: parent
            visible: settings.snap_mode === "auto" || root.modifiersSatisfied() || overlayForced

            Repeater {
                model: root.activeLayout() ? root.activeLayout().zones : []
                delegate: Rectangle {
                    property var rect: root.zoneRect(modelData, activeArea)
                    x: rect.x
                    y: rect.y
                    width: rect.width
                    height: rect.height
                    color: Qt.rgba(settings.highlight_color.red, settings.highlight_color.green, settings.highlight_color.blue, index === highlightedZone ? settings.overlay_opacity : settings.overlay_opacity * 0.32)
                    border.color: Qt.rgba(1, 1, 1, index === highlightedZone ? 0.95 : 0.45)
                    border.width: index === highlightedZone ? 3 : 1

                    PlasmaComponents.Label {
                        anchors.centerIn: parent
                        visible: settings.show_zone_numbers
                        text: index + 1
                        color: "white"
                        font.pixelSize: 28
                        font.bold: true
                    }
                }
            }

            Rectangle {
                id: selector
                visible: settings.enable_zone_selector && Workspace.cursorPos.y >= activeArea.y && Workspace.cursorPos.y <= activeArea.y + height + 18
                x: activeArea.x
                y: activeArea.y
                width: activeArea.width
                height: 86
                color: Qt.rgba(0.04, 0.05, 0.06, 0.78)
                border.color: Qt.rgba(1, 1, 1, 0.22)

                Repeater {
                    model: settings.layouts
                    delegate: Item {
                        id: layoutDelegate
                        property int layoutIndex: index

                        x: index * selector.width / Math.max(1, settings.layouts.length)
                        y: 0
                        width: selector.width / Math.max(1, settings.layouts.length)
                        height: selector.height

                        PlasmaComponents.Label {
                            x: 6
                            y: 4
                            width: parent.width - 12
                            height: 20
                            elide: Text.ElideRight
                            text: modelData.name
                            color: index === root.activeLayoutIndex() ? "white" : "#d6d6d6"
                            font.pixelSize: 11
                        }

                        Repeater {
                            model: modelData.zones
                            delegate: Rectangle {
                                x: modelData.x * parent.width + 3
                                y: 26 + modelData.y * 54
                                width: Math.max(3, modelData.width * parent.width - 6)
                                height: Math.max(3, modelData.height * 54 - 3)
                                radius: 2
                                color: Qt.rgba(settings.highlight_color.red, settings.highlight_color.green, settings.highlight_color.blue, index === highlightedZone && layoutDelegate.layoutIndex === root.activeLayoutIndex() ? 0.70 : 0.28)
                                border.color: Qt.rgba(1, 1, 1, 0.38)
                            }
                        }
                    }
                }
            }
        }
    }

    ShortcutHandler {
        name: "FanzyZones: Process pending action"
        text: "FanzyZones: Process pending action"
        sequence: "Meta+Ctrl+Alt+Shift+F12"
        onActivated: root.processPendingAction()
    }

    ShortcutHandler {
        name: "FanzyZones: Move active window to next zone"
        text: "FanzyZones: Move active window to next zone"
        sequence: settings.keyboard_shortcuts_enabled ? "Ctrl+Alt+Right" : ""
        onActivated: root.cycleActiveWindow(1)
    }

    ShortcutHandler {
        name: "FanzyZones: Move active window to previous zone"
        text: "FanzyZones: Move active window to previous zone"
        sequence: settings.keyboard_shortcuts_enabled ? "Ctrl+Alt+Left" : ""
        onActivated: root.cycleActiveWindow(-1)
    }

    ShortcutHandler {
        name: "FanzyZones: Toggle zone overlay"
        text: "FanzyZones: Toggle zone overlay"
        sequence: settings.keyboard_shortcuts_enabled ? "Ctrl+Alt+C" : ""
        onActivated: {
            if (moving)
                overlayForced = !overlayForced;
        }
    }

    ShortcutHandler {
        name: "FanzyZones: Snap active window"
        text: "FanzyZones: Snap active window"
        sequence: settings.keyboard_shortcuts_enabled ? "Meta+Shift+Space" : ""
        onActivated: root.snapClientToClosestZone(root.targetWindow())
    }

    ShortcutHandler {
        name: "FanzyZones: Snap all windows"
        text: "FanzyZones: Snap all windows"
        sequence: settings.keyboard_shortcuts_enabled ? "Meta+Space" : ""
        onActivated: root.snapAllWindows()
    }

    Repeater {
        model: [1, 2, 3, 4, 5, 6, 7, 8, 9]
        delegate: Item {
            ShortcutHandler {
                name: "FanzyZones: Move active window to zone " + modelData
                text: "FanzyZones: Move active window to zone " + modelData
                sequence: settings.keyboard_shortcuts_enabled ? "Ctrl+Alt+Num+" + modelData : ""
                onActivated: root.snapActiveWindowToZone(modelData - 1)
            }
        }
    }

    Repeater {
        model: [1, 2, 3, 4, 5, 6, 7, 8, 9]
        delegate: Item {
            ShortcutHandler {
                name: "FanzyZones: Activate layout " + modelData
                text: "FanzyZones: Activate layout " + modelData
                sequence: settings.keyboard_shortcuts_enabled ? "Meta+Num+" + modelData : ""
                onActivated: root.setActiveLayout(modelData - 1)
            }
        }
    }

}
