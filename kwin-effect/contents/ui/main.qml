import QtQuick
import QtQml.Models
import org.kde.kwin

SceneEffect {
    id: sceneEffect
    property bool framing: false

    WorkspaceController {
        id: workspace
        effect: sceneEffect
    }

    delegate: WorkspaceScene {
        controller: workspace
        effect: sceneEffect
        output: SceneView.screen
    }

    Instantiator {
        model: workspace.chrome.length
        delegate: WorkspaceChrome {
            required property int index
            controller: workspace
            entry: workspace.chrome[index] || ({})
        }
    }

    DropPreview { controller: workspace }

    Instantiator {
        model: [
            { action: "show-all", text: "Show everything", key: "Ctrl+Alt+Space" },
            { action: "zoom-out", text: "Step out a level", key: "Ctrl+Alt+Up" },
            { action: "zoom-in", text: "Step in toward the pointer", key: "Ctrl+Alt+Down" },
            { action: "focus", text: "Focus window", key: "Ctrl+Alt+Return" },
            { action: "up", text: "Space above", key: "Ctrl+Alt+Shift+Up" },
            { action: "down", text: "Space below", key: "Ctrl+Alt+Shift+Down" },
            { action: "back", text: "Previous view", key: "Ctrl+Alt+[" },
            { action: "forward", text: "Next view", key: "Ctrl+Alt+]" },
            { action: "toggle-desktop", text: "Toggle Desktop", key: "Ctrl+Alt+D" },
            { action: "new-window", text: "Split with a file manager", key: "Ctrl+Alt+N" },
            { action: "toggle-float", text: "Toggle floating", key: "Ctrl+Alt+F" },
            { action: "frame", text: "Frame a rectangle", key: "Meta+Shift+R" },
            { action: "next-tab", text: "Next workspace tab", key: "Ctrl+Alt+Tab" },
            { action: "previous-tab", text: "Previous workspace tab", key: "Ctrl+Alt+Shift+Tab" },
            { action: "undo", text: "Undo workspace change", key: "Ctrl+Alt+Z" },
            { action: "redo", text: "Redo workspace change", key: "Ctrl+Alt+Shift+Z" },
            { action: "disable", text: "Restore windows and disable nested workspaces", key: "Meta+Ctrl+Shift+F12" }
        ]
        delegate: ShortcutHandler {
            required property var modelData
            name: "FanzyZones: Workspace " + modelData.text
            text: "FanzyZones: Workspace " + modelData.text
            sequence: modelData.key
            onActivated: {
                if (modelData.action === "disable") {
                    workspace.request("InvokeAction", [JSON.stringify({ action: "workspace", command: { action: "disable" } })]);
                } else workspace.execute({ action: modelData.action });
            }
        }
    }
    Instantiator {
        model: 18
        delegate: ShortcutHandler {
            required property int index
            readonly property int slot: index % 9 + 1
            readonly property bool saving: index >= 9
            name: "FanzyZones: Workspace " + (saving ? "Save view " : "Go to view ") + slot
            text: name
            sequence: (saving ? "Ctrl+Alt+Shift+" : "Ctrl+Alt+") + slot
            onActivated: workspace.execute({ action: saving ? "save-view" : "load-view", slot: slot })
        }
    }
    Instantiator {
        model: workspace.enabled ? [true, false] : []
        delegate: PinchGestureHandler {
            required property bool modelData
            fingerCount: 3
            direction: modelData ? PinchGestureHandler.Direction.Contracting : PinchGestureHandler.Direction.Expanding
            onProgressChanged: workspace.gesture(modelData ? "out" : "in", progress)
            onActivated: workspace.endGesture(true)
            onCancelled: workspace.endGesture(false)
        }
    }
}
