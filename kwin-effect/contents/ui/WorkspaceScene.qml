import QtQuick
import org.kde.kwin
import org.kde.kirigami as Kirigami
import "../code/Workspace.js" as Engine

Rectangle {
    id: scene
    required property var controller
    required property var effect
    required property var output
    readonly property var view: controller.views.find(function(v) { return v.output === output; }) || null
    readonly property real originX: output ? output.geometry.x : 0
    readonly property real originY: output ? output.geometry.y : 0
    color: "#151a22"
    focus: true
    clip: true

    Repeater {
        model: scene.view ? scene.view.tiles : []
        delegate: Rectangle {
            id: tile
            required property var modelData
            readonly property var start: controller.tileStartRect(modelData, scene.view.key)
            readonly property real progress: controller.animating ? controller.transition : 1
            x: start.x + (modelData.rect.x - start.x) * progress - scene.originX
            y: start.y + (modelData.rect.y - start.y) * progress - scene.originY
            width: Math.max(1, start.width + (modelData.rect.width - start.width) * progress)
            height: Math.max(1, start.height + (modelData.rect.height - start.height) * progress)
            radius: 5
            color: modelData.kind === "desktop" ? "#202b3c" : controller.appColor(modelData.id)
            border.width: 1
            border.color: "#537095"
            clip: true

            DesktopBackground {
                anchors.fill: parent
                output: scene.output
                // KWin's fallback dereferences its activity manager, which is
                // absent in sessions launched with --no-kactivities.
                activity: Workspace.currentActivity || "00000000-0000-0000-0000-000000000000"
                visible: modelData.kind === "desktop"
            }

            WindowThumbnail {
                anchors.fill: parent
                anchors.topMargin: modelData.kind === "floating" ? 0 : controller.headerHeight
                anchors.margins: 4
                client: controller.clients[modelData.id] || null
                visible: !!client && modelData.kind !== "placeholder" && modelData.kind !== "desktop"
            }
            Kirigami.Icon {
                anchors.centerIn: parent
                width: Math.min(48, parent.width * .35)
                height: width
                source: controller.clients[modelData.id] ? controller.clients[modelData.id].icon : "user-desktop"
                visible: modelData.kind === "placeholder" || modelData.kind === "desktop"
            }
            Text {
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: 8 }
                text: modelData.title
                color: "#f1f5fb"
                font.pixelSize: 12
                elide: Text.ElideRight
                visible: modelData.kind !== "floating"
            }
            MouseArea {
                anchors.fill: parent
                enabled: !scene.effect.framing && !controller.animating
                onDoubleClicked: controller.focusNode(modelData.scope, modelData.node)
                onClicked: if (modelData.id) controller.activate(modelData.id)
            }
        }
    }

    Rectangle {
        anchors { top: parent.top; horizontalCenter: parent.horizontalCenter; topMargin: 20 }
        width: instruction.implicitWidth + 32
        height: 42
        radius: 8
        color: "#e625303f"
        visible: scene.effect.framing
        Text {
            id: instruction
            anchors.centerIn: parent
            text: "Drag to frame a view · Esc to cancel"
            color: "white"
        }
    }
    MouseArea {
        id: selection
        anchors.fill: parent
        enabled: scene.effect.framing && !controller.nativeFraming
        cursorShape: Qt.CrossCursor
        property point start
        property point end
        onPressed: function(mouse) { start = Qt.point(mouse.x, mouse.y); end = start; }
        onPositionChanged: function(mouse) { if (pressed) end = Qt.point(mouse.x, mouse.y); }
        onReleased: {
            scene.effect.framing = false;
            if (scene.view) controller.frameRectangle(scene.view.key, Engine.rect(
                Math.min(start.x, end.x) + scene.originX, Math.min(start.y, end.y) + scene.originY,
                Math.abs(end.x - start.x), Math.abs(end.y - start.y)));
            if (!controller.animating) scene.effect.visible = false;
        }
    }
    Rectangle {
        x: controller.nativeFrameRect.x - scene.originX
        y: controller.nativeFrameRect.y - scene.originY
        width: controller.nativeFrameRect.width
        height: controller.nativeFrameRect.height
        visible: controller.nativeFraming
        color: "#335996e0"
        border { width: 2; color: "#88baff" }
    }
    Rectangle {
        x: Math.min(selection.start.x, selection.end.x)
        y: Math.min(selection.start.y, selection.end.y)
        width: Math.abs(selection.end.x - selection.start.x)
        height: Math.abs(selection.end.y - selection.start.y)
        visible: selection.pressed
        color: "#335996e0"
        border { width: 2; color: "#88baff" }
    }
    Keys.onEscapePressed: {
        if (controller.gestureActive) controller.cancelGesture();
        scene.effect.framing = false;
        scene.effect.visible = false;
    }
}
