import QtQuick
import QtQuick.Controls.Basic as Basic

// Render in the host scene instead of delegating to the macOS native menu.
// The caller supplies the Logos palette and font; no global style is changed.
Basic.ComboBox {
    id: control
    property Item viewportItem: parent
    property int maximumPopupHeight: 328
    readonly property real anchorY: viewportItem ? mapToItem(viewportItem, 0, 0).y : y
    readonly property real roomAbove: Math.max(0, anchorY - 12)
    readonly property real roomBelow: Math.max(0, (viewportItem ? viewportItem.height : 600) - anchorY - height - 12)
    readonly property real desiredPopupHeight: Math.min(maximumPopupHeight, count * 44 + 8)
    readonly property bool opensBelow: roomBelow >= desiredPopupHeight || roomBelow >= roomAbove

    implicitHeight: 42
    leftPadding: 12
    rightPadding: 34
    hoverEnabled: true
    contentItem: Text {
        text: control.displayText
        textFormat: Text.PlainText
        font: control.font
        color: control.enabled ? control.palette.text : control.palette.placeholderText
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    indicator: Text {
        x: control.width - width - 12
        y: (control.height - height) / 2
        text: "\u2304"
        font: control.font
        color: control.palette.placeholderText
    }
    background: Rectangle {
        radius: 6
        color: control.palette.base
        border.color: control.activeFocus || control.popup.visible ? control.palette.accent : control.palette.mid
    }
    delegate: Basic.ItemDelegate {
        id: option
        required property int index
        width: optionList.width
        height: 44
        text: control.textAt(index)
        Accessible.name: text
        highlighted: control.highlightedIndex === index
        hoverEnabled: true
        contentItem: Text {
            text: option.text
            textFormat: Text.PlainText
            font: control.font
            color: control.palette.text
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
        background: Rectangle {
            radius: 4
            color: option.highlighted || option.hovered ? control.palette.highlight : control.palette.window
        }
    }
    popup: Basic.Popup {
        objectName: "workspace.popup"
        popupType: Basic.Popup.Item
        width: control.width
        height: Math.max(8, Math.min(control.desiredPopupHeight, control.opensBelow ? control.roomBelow : control.roomAbove))
        y: control.opensBelow ? control.height + 4 : -height - 4
        padding: 4
        closePolicy: Basic.Popup.CloseOnEscape | Basic.Popup.CloseOnPressOutsideParent
        contentItem: ListView {
            id: optionList
            objectName: "workspace.options"
            clip: true
            model: control.popup.visible ? control.delegateModel : null
            currentIndex: control.highlightedIndex
            highlightMoveDuration: 0
            boundsBehavior: Flickable.StopAtBounds
            Basic.ScrollBar.vertical: Basic.ScrollBar {
                policy: Basic.ScrollBar.AsNeeded
                contentItem: Rectangle { implicitWidth: 4; radius: 2; color: control.palette.placeholderText }
                background: Rectangle { color: "transparent" }
            }
        }
        background: Rectangle {
            radius: 6
            color: control.palette.window
            border.color: control.palette.mid
        }
        onOpened: {
            optionList.positionViewAtIndex(Math.max(0, control.currentIndex), ListView.Contain)
        }
    }
}
