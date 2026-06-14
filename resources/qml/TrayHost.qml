import QtQuick
import FanzyZones

Item {
    FanzyBackend {
        id: fanzyBackend
    }

    LayoutMenu {
        backend: fanzyBackend
    }
}
