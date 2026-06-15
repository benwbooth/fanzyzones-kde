import QtQuick
import FanzyZones 1.0

Item {
    id: root

    visible: false
    property var menuWindow: null

    FanzyBackend {
        id: backendObject
    }

    Component.onCompleted: {
        menuWindow = menuComponent.createObject(null, {
            "backend": backendObject,
            "embeddedBackendRequested": true
        });
        if (menuWindow === null)
            console.error("FanzyZones tray menu failed to create");
    }

    Component {
        id: menuComponent

        LayoutMenu {}
    }
}
