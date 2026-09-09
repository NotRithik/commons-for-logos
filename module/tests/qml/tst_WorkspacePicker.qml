import QtQuick
import QtQuick.Controls.Basic as Basic
import QtTest
import "../../src/qml" as Ui

// Actual shipping control, with test-only data. This is not network evidence.
Item {
    id: viewport
    width: 800
    height: 600
    Ui.WorkspacePicker {
        id: picker
        x: 30; y: 120; width: 500
        viewportItem: viewport
        textRole: "label"
        palette.base: "#222222"
        palette.window: "#282828"
        palette.text: "#eeeeee"
        palette.placeholderText: "#999999"
        palette.mid: "#444444"
        palette.accent: "#f07850"
        palette.highlight: "#383838"
    }
    SignalSpy { id: activations; target: picker; signalName: "activated" }
    TestCase {
        name: "WorkspacePicker"
        when: windowShown
        function init() {
            picker.popup.close()
            picker.y = 120
            picker.enabled = true
            picker.model = [{label: "Live membership"}, {label: "Current shared approvals"}, {label: "Archive"}]
            picker.currentIndex = 0
            activations.clear()
        }
        function cleanup() { picker.popup.close() }
        function openMenu() {
            mouseClick(picker, picker.width / 2, picker.height / 2)
            tryCompare(picker.popup, "visible", true)
            waitForRendering(picker)
        }
        function test_popup_has_explicit_palette_and_no_blank_native_surface() {
            openMenu()
            compare(picker.popup.popupType, Basic.Popup.Item)
            compare(picker.popup.background.color, picker.palette.window)
            compare(picker.popup.contentItem.count, 3)
            verify(picker.popup.height <= 3 * 44 + 8)
            compare(picker.popup.contentItem.itemAtIndex(0).text, "Live membership")
            compare(activations.count, 0)
        }
        function test_select_by_mouse_activates_exactly_once() {
            openMenu()
            let row = picker.popup.contentItem.itemAtIndex(1)
            verify(row !== null)
            mouseClick(row, row.width / 2, row.height / 2)
            tryCompare(picker, "currentIndex", 1)
            tryCompare(picker.popup, "visible", false)
            compare(activations.count, 1)
            compare(picker.currentText, "Current shared approvals")
        }
        function test_escape_does_not_activate_or_change_selection() {
            picker.currentIndex = 1
            openMenu()
            keyClick(Qt.Key_Escape)
            tryCompare(picker.popup, "visible", false)
            compare(picker.currentIndex, 1)
            compare(activations.count, 0)
        }
        function test_keyboard_selection() {
            picker.forceActiveFocus()
            keyClick(Qt.Key_Space)
            tryCompare(picker.popup, "visible", true)
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Return)
            tryCompare(picker.popup, "visible", false)
            compare(picker.currentIndex, 1)
            compare(activations.count, 1)
        }
        function test_long_catalog_is_bounded_and_scrollable() {
            let rows = []
            for (let i = 0; i < 32; i++) rows.push({label: "Workspace " + i})
            picker.model = rows
            picker.currentIndex = 31
            openMenu()
            verify(picker.popup.height <= 328)
            verify(picker.popup.contentItem.contentHeight > picker.popup.height)
            verify(picker.popup.contentItem.contentY > 0)
            compare(activations.count, 0)
        }
        function test_bottom_position_opens_above() {
            picker.y = 550
            openMenu()
            verify(picker.popup.y < 0)
            verify(picker.y + picker.popup.y >= 0)
        }
        function test_disabled_control_cannot_open() {
            picker.enabled = false
            mouseClick(picker, 20, 20)
            compare(picker.popup.visible, false)
            compare(activations.count, 0)
        }
    }
}
