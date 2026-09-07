import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Logos.Theme
import Logos.Controls

Item {
    id: root
    objectName: "commons.root"
    readonly property var backend: logos.module("commons_primitives_ui")
    property bool ready: false
    readonly property bool configured: ready && backend && backend.configured
    readonly property bool busy: ready && backend && backend.busy
    property bool connectionExpanded: true
    property bool allowCreateExpanded: false
    property bool groupCreateExpanded: false
    property bool technicalExpanded: false
    property string pendingWitnessTarget: "allowlist"
    property double operationStartedAt: 0
    property int elapsedSeconds: 0
    property bool captureInFlight: false
    property bool captureRecording: false
    property int captureFrames: 0
    property string captureStatus: ""
    onConfiguredChanged: if (!configured) captureRecording = false
    Timer {
        interval: 1500
        repeat: true
        running: root.captureRecording && root.configured
        onTriggered: {
            if (root.captureFrames >= 2400) {
                root.captureRecording = false
                root.captureStatus = "Recording stopped at the one-hour limit."
            } else root.captureOwnView()
        }
    }
    Shortcut {
        sequence: "Ctrl+Shift+F10"
        context: Qt.ApplicationShortcut
        enabled: root.configured
        onActivated: root.toggleCaptureRecording()
    }
    function toggleCaptureRecording() {
        if (root.captureRecording) {
            root.captureRecording = false
            root.captureStatus = "Recording stopped. " + root.captureFrames + " frames saved."
        } else {
            root.captureFrames = 0
            root.captureRecording = true
            root.captureStatus = "Recording this module only."
            root.captureOwnView()
        }
    }

    onBusyChanged: if (busy) { operationStartedAt = Date.now(); elapsedSeconds = 0 }
    Timer {
        interval: 1000; repeat: true; running: root.busy
        onTriggered: root.elapsedSeconds = Math.floor((Date.now() - root.operationStartedAt) / 1000)
    }
    Shortcut { sequence: "Alt+1"; onActivated: tabs.currentIndex = 0 }
    Shortcut { sequence: "Alt+2"; onActivated: tabs.currentIndex = 1 }
    // Captures only this module, not other windows or the desktop.
    Shortcut { sequence: "Ctrl+Shift+F9"; context: Qt.ApplicationShortcut; enabled: root.configured; onActivated: root.captureOwnView() }
    function captureOwnView() {
        if (root.captureInFlight || !root.backend || !root.backend.captureDirectory) return
        root.captureInFlight = true
        root.connectionExpanded = false
        Qt.callLater(function() {
            const scale = Math.min(1, 1440 / Math.max(1, root.width))
            const accepted = root.grabToImage(function(image) {
                const saved = image.saveToFile(root.backend.captureDirectory + "/commons-view-" + Date.now() + ".png")
                if (saved) {
                    root.captureFrames += 1
                    root.captureStatus = root.captureRecording
                        ? "Recording this module only. " + root.captureFrames + " frames saved."
                        : "Module view saved."
                } else {
                    root.captureRecording = false
                    root.captureStatus = "Could not save the module view. Check the wallet evidence directory."
                }
                root.captureInFlight = false
            }, Qt.size(Math.max(1, Math.round(root.width * scale)), Math.max(1, Math.round(root.height * scale))))
            if (!accepted) {
                root.captureInFlight = false
                root.captureRecording = false
                root.captureStatus = "The module must be visible before it can be captured."
            }
        })
    }
    function localPath(url) {
        const text = String(url || "")
        if (text.indexOf("file:///") === 0) return decodeURIComponent(text.substring(7))
        if (text.indexOf("file://") === 0) return decodeURIComponent(text.substring(5))
        return text
    }
    function callBackend(reply) {
        if (logos && logos.watch)
            logos.watch(reply, function() {}, function(error) { transientError.text = String(error) })
    }
    Connections {
        target: logos
        function onViewModuleReadyChanged(name, isReady) {
            if (name === "commons_primitives_ui") root.ready = isReady && root.backend !== null
        }
    }
    Connections {
        target: root.backend
        function onOperationFinished(operation, resultJson) {
            transientError.text = ""
            if (operation === "configure") root.connectionExpanded = false
        }
        function onOperationFailed(operation, message) { transientError.text = message }
    }
    Component.onCompleted: root.ready = root.backend !== null && logos.isViewModuleReady("commons_primitives_ui")

    component CopyLabel: LogosText {
        color: Theme.palette.textSecondary
        wrapMode: Text.WordWrap
    }
    component Caption: LogosText {
        color: Theme.palette.textTertiary
        font.pixelSize: Theme.typography.secondaryText
        font.weight: Theme.typography.weightMedium
    }
    // Keep a native TextField so the editable value stays accessible through
    // Basecamp's Qt accessibility tree, with the shared design-system tokens.
    component Field: TextField {
        implicitHeight: 40
        font.family: Theme.typography.publicSans
        font.pixelSize: Theme.typography.primaryText
        color: Theme.palette.text
        placeholderTextColor: Theme.palette.textTertiary
        selectionColor: Theme.palette.overlayOrange
        selectedTextColor: Theme.palette.text
        selectByMouse: true
        leftPadding: Theme.spacing.medium
        rightPadding: Theme.spacing.medium
        background: Rectangle {
            radius: Theme.spacing.radiusSmall
            color: Theme.palette.backgroundSecondary
            border.color: parent.activeFocus ? Theme.palette.overlayOrange : Theme.palette.backgroundElevated
        }
    }
    component Action: LogosButton {
        property bool primary: false
        variant: primary ? LogosButton.Variant.Primary : LogosButton.Variant.Secondary
        radius: Theme.spacing.radiusXlarge
    }
    component Card: Pane {
        padding: 20
        background: Rectangle { color: Theme.palette.surfaceRaised; radius: Theme.spacing.radiusXlarge }
    }
    component CountInput: SpinBox {
        implicitHeight: 38
        from: 1; to: 256; value: 10
        editable: true
        palette.text: Theme.palette.text
        palette.base: Theme.palette.backgroundSecondary
        palette.button: Theme.palette.backgroundSecondary
        palette.buttonText: Theme.palette.text
        palette.highlight: Theme.palette.primary
        background: Rectangle { radius: 7; color: Theme.palette.backgroundSecondary; border.color: Theme.palette.border }
    }
    component SectionHeading: LogosText {
        font.pixelSize: Theme.typography.panelTitleText
        font.weight: Theme.typography.weightMedium
        color: Theme.palette.text
    }

    FileDialog {
        id: cliDialog
        title: "Select the Commons CLI"
        fileMode: FileDialog.OpenFile
        options: FileDialog.DontUseNativeDialog | FileDialog.ReadOnly
        onAccepted: cliPath.text = root.localPath(selectedFile)
    }
    FolderDialog {
        id: walletDialog
        title: "Select a testnet wallet"
        options: FolderDialog.DontUseNativeDialog | FolderDialog.ReadOnly
        onAccepted: walletDir.text = root.localPath(selectedFolder)
    }
    Dialog {
        id: witnessPathDialog
        objectName: "witness.pathDialog"
        title: "Select your membership credential"
        modal: true
        anchors.centerIn: parent
        width: Math.min(parent.width - 40, 650)
        palette.window: Theme.palette.surfaceRaised
        palette.windowText: Theme.palette.text
        palette.button: Theme.palette.border
        palette.buttonText: Theme.palette.text
        standardButtons: Dialog.Ok | Dialog.Cancel
        onOpened: { witnessPathInput.text = ""; witnessPathInput.forceActiveFocus() }
        onAccepted: {
            const path = root.localPath(witnessPathInput.text.trim())
            root.callBackend(root.pendingWitnessTarget === "threshold"
                ? root.backend.selectThresholdWitness(path) : root.backend.selectAllowlistWitness(path))
        }
        contentItem: ColumnLayout {
            spacing: 14
            CopyLabel { text: "Choose the credential supplied for your membership. The file must be inside your configured wallet folder and stays on this device."; Layout.fillWidth: true }
            Field {
                id: witnessPathInput
                objectName: "witness.pathInput"
                Accessible.name: "Local private witness file path"
                placeholderText: "Absolute path to the credential file"
                inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhSensitiveData
                Layout.fillWidth: true
            }
        }
    }

    Rectangle { anchors.fill: parent; color: Theme.palette.background }
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Theme.spacing.xxlarge
        spacing: 18
        RowLayout {
            spacing: 14
            Rectangle {
                implicitWidth: 40; implicitHeight: 40; radius: 10
                color: Theme.palette.surfaceRaised; border.color: Theme.palette.border
                LogosText { anchors.centerIn: parent; text: "C"; font.pixelSize: 26; font.weight: Font.Medium; color: Theme.palette.text }
            }
            ColumnLayout {
                spacing: 3
                LogosText { text: "Commons for Logos"; font.pixelSize: Theme.typography.pageTitleText; font.weight: Theme.typography.weightMedium; color: Theme.palette.text }
                CopyLabel { text: "Private membership and shared approvals"; font.pixelSize: 12 }
            }
            Item { Layout.fillWidth: true }
            Rectangle {
                implicitWidth: 72; implicitHeight: 26; radius: 13
                color: Theme.palette.backgroundMuted; border.color: Theme.palette.borderSubtle
                LogosText { anchors.centerIn: parent; text: "TESTNET"; color: Theme.palette.warning; font.pixelSize: 10; font.weight: Font.DemiBold; font.letterSpacing: 1 }
            }
        }

        Card {
            Layout.fillWidth: true
            padding: 14
            contentItem: ColumnLayout {
                spacing: 14
                RowLayout {
                    Rectangle { implicitWidth: 7; implicitHeight: 7; radius: 4; color: root.configured ? Theme.palette.success : Theme.palette.warning }
                    CopyLabel { text: root.configured ? "Client connected" : "Not configured"; color: Theme.palette.text }
                    Item { Layout.fillWidth: true }
                    Action { text: root.connectionExpanded ? "Hide settings" : "Connection settings"; implicitHeight: 30; onClicked: root.connectionExpanded = !root.connectionExpanded }
                }
                GridLayout {
                    visible: root.connectionExpanded
                    columns: root.width > 1000 ? 2 : 1
                    columnSpacing: 16; rowSpacing: 10
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        Caption { text: "LOCAL CLIENT" }
                        RowLayout {
                            Field { id: cliPath; objectName: "config.cliPath"; Accessible.name: "Testnet CLI executable"; placeholderText: "Select the Commons CLI executable"; Layout.fillWidth: true }
                            Action { objectName: "config.cliBrowse"; text: "Browse"; Accessible.name: "Browse CLI executable"; onClicked: cliDialog.open() }
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        Caption { text: "WALLET PROFILE" }
                        RowLayout {
                            Field { id: walletDir; objectName: "config.walletDir"; Accessible.name: "Testnet wallet directory"; placeholderText: "Select your testnet wallet folder"; Layout.fillWidth: true }
                            Action { objectName: "config.walletBrowse"; text: "Browse"; Accessible.name: "Browse testnet wallet directory"; onClicked: walletDialog.open() }
                        }
                    }
                    Action {
                        objectName: "config.apply"
                        text: "Connect client"; primary: true
                        Accessible.name: "Configure testnet backend"
                        enabled: root.ready && !root.busy
                        onClicked: root.callBackend(root.backend.configure(cliPath.text, walletDir.text))
                    }
                }
            }
        }

        LogosTabBar {
            id: tabs
            Layout.fillWidth: true
            LogosTabButton {
                objectName: "tabs.allowlist"
                text: "Private membership"
                Accessible.name: "Allowlist tab"
                width: implicitWidth + Theme.spacing.xlarge
                Accessible.onPressAction: tabs.currentIndex = 0
            }
            LogosTabButton {
                objectName: "tabs.threshold"
                text: "Shared approvals"
                Accessible.name: "Threshold tab"
                width: implicitWidth + Theme.spacing.xlarge
                Accessible.onPressAction: tabs.currentIndex = 1
            }
        }

        ScrollView {
            id: contentScroll
            Layout.fillWidth: true; Layout.fillHeight: true
            clip: true
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
            contentWidth: availableWidth
            ColumnLayout {
                width: contentScroll.availableWidth
                spacing: 16
                StackLayout {
                    currentIndex: tabs.currentIndex
                    Layout.fillWidth: true
                    Card {
                        Layout.fillWidth: true
                        contentItem: ColumnLayout {
                            spacing: 16
                            RowLayout {
                                ColumnLayout {
                                    SectionHeading { text: "Private allowlist" }
                                    CopyLabel { text: "Prove you belong. Register once without publishing your address."; Layout.fillWidth: true }
                                    Layout.fillWidth: true
                                }
                                Action { text: root.allowCreateExpanded ? "Close setup" : "New allowlist"; onClicked: root.allowCreateExpanded = !root.allowCreateExpanded }
                            }
                            Caption { text: "DISTRIBUTION ACCOUNT" }
                            RowLayout {
                                Field { id: allowState; objectName: "allowlist.stateAccount"; Accessible.name: "Allowlist state account"; placeholderText: "Paste the 64-character state account"; Layout.fillWidth: true }
                                Action { objectName: "allowlist.inspect"; text: "Inspect"; Accessible.name: "Inspect allowlist state"; enabled: root.configured && !root.busy; onClicked: root.callBackend(root.backend.inspectDistribution(allowState.text)) }
                            }
                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: allowSummary.implicitHeight + 28
                                color: Theme.palette.backgroundMuted; radius: 7; border.color: Theme.palette.borderSubtle
                                CopyLabel {
                                    id: allowSummary; objectName: "allowlist.summary"; Accessible.name: text
                                    anchors.fill: parent; anchors.margins: 14; color: Theme.palette.textSecondary
                                    text: !root.backend ? "No distribution loaded."
                                        : root.backend.distributionStateAccount && allowState.text.trim() !== root.backend.distributionStateAccount
                                          ? "Account changed. Inspect to load this distribution."
                                          : root.backend.distributionSummary
                                }
                            }
                            RowLayout {
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Caption { text: "MEMBERSHIP CREDENTIAL" }
                                    CopyLabel { text: root.backend && root.backend.allowlistWitnessLabel !== "No witness selected" ? "Credential selected on this device" : "No credential selected" }
                                }
                                Action { objectName: "allowlist.witnessButton"; text: "Select credential"; Accessible.name: "Choose private claim witness"; enabled: root.configured && !root.busy; onClicked: { root.pendingWitnessTarget = "allowlist"; witnessPathDialog.open() } }
                                Action { objectName: "allowlist.claim"; text: "Register privately"; primary: true; Accessible.name: "Claim allocation privately"; enabled: root.configured && !root.busy && root.backend.allowlistWitnessLabel !== "No witness selected"; onClicked: root.callBackend(root.backend.claimAllowlist(allowState.text)) }
                            }
                            ColumnLayout {
                                visible: root.allowCreateExpanded
                                spacing: 12; Layout.fillWidth: true
                                Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.palette.border }
                                Caption { text: "NEW DISTRIBUTION" }
                                CopyLabel { text: "Use an unused account from your wallet and the commitment root generated for the eligible members."; Layout.fillWidth: true }
                                Field { id: allowRoot; objectName: "allowlist.root"; Accessible.name: "Allowlist Merkle root"; placeholderText: "Membership commitment root (64-character hex)"; Layout.fillWidth: true }
                                RowLayout {
                                    CopyLabel { text: "Eligible members" }
                                    CountInput { id: allowMemberCount; objectName: "allowlist.memberCount"; Accessible.name: "Eligible member count" }
                                    Item { Layout.fillWidth: true }
                                    Action { objectName: "allowlist.create"; text: "Create allowlist"; primary: true; Accessible.name: "Create distribution"; enabled: root.configured && !root.busy; onClicked: root.callBackend(root.backend.createDistribution(allowState.text, allowRoot.text, allowMemberCount.value)) }
                                }
                            }
                        }
                    }
                    Card {
                        Layout.fillWidth: true
                        contentItem: ColumnLayout {
                            spacing: 16
                            RowLayout {
                                ColumnLayout {
                                    SectionHeading { text: "Shared approvals" }
                                    CopyLabel { text: "Set a threshold. Collect private approvals before a change takes effect."; Layout.fillWidth: true }
                                    Layout.fillWidth: true
                                }
                                Action { text: root.groupCreateExpanded ? "Close setup" : "New group"; onClicked: root.groupCreateExpanded = !root.groupCreateExpanded }
                            }
                            Caption { text: "GROUP ACCOUNT" }
                            RowLayout {
                                Field { id: groupState; objectName: "threshold.stateAccount"; Accessible.name: "Threshold state account"; placeholderText: "Paste the 64-character group state account"; Layout.fillWidth: true }
                                Action { objectName: "threshold.inspect"; text: "Inspect"; Accessible.name: "Inspect threshold state"; enabled: root.configured && !root.busy; onClicked: root.callBackend(root.backend.inspectGroup(groupState.text)) }
                            }
                            Rectangle {
                                Layout.fillWidth: true
                                implicitHeight: groupSummary.implicitHeight + 28
                                color: Theme.palette.backgroundMuted; radius: 7; border.color: Theme.palette.borderSubtle
                                CopyLabel {
                                    id: groupSummary; objectName: "threshold.summary"; Accessible.name: text
                                    anchors.fill: parent; anchors.margins: 14; color: Theme.palette.textSecondary
                                    text: !root.backend ? "No group loaded."
                                        : root.backend.groupStateAccount && groupState.text.trim() !== root.backend.groupStateAccount
                                          ? "Account changed. Inspect to load this group."
                                          : root.backend.groupSummary
                                }
                            }
                            RowLayout {
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Caption { text: "MEMBERSHIP CREDENTIAL" }
                                    CopyLabel { text: root.backend && root.backend.thresholdWitnessLabel !== "No witness selected" ? "Credential selected on this device" : "No credential selected" }
                                }
                                Action { objectName: "threshold.witnessButton"; text: "Select credential"; Accessible.name: "Choose private approval witness"; enabled: root.configured && !root.busy; onClicked: { root.pendingWitnessTarget = "threshold"; witnessPathDialog.open() } }
                            }
                            RowLayout {
                                Field { id: nextValue; objectName: "threshold.nextValue"; Accessible.name: "Proposed parameter value"; placeholderText: "New parameter value"; Layout.fillWidth: true }
                                Action { objectName: "threshold.propose"; text: "Propose"; Accessible.name: "Propose parameter change"; enabled: root.configured && !root.busy && root.backend.thresholdWitnessLabel !== "No witness selected"; onClicked: root.callBackend(root.backend.proposeParameter(groupState.text, nextValue.text)) }
                                Action { objectName: "threshold.approve"; text: "Approve privately"; Accessible.name: "Approve privately"; enabled: root.configured && !root.busy && root.backend.thresholdWitnessLabel !== "No witness selected"; onClicked: root.callBackend(root.backend.approveParameter(groupState.text)) }
                                Action { objectName: "threshold.execute"; text: "Execute"; primary: true; Accessible.name: "Execute approved proposal"; enabled: root.configured && !root.busy; onClicked: root.callBackend(root.backend.executeParameter(groupState.text)) }
                            }
                            ColumnLayout {
                                visible: root.groupCreateExpanded
                                spacing: 12; Layout.fillWidth: true
                                Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Theme.palette.border }
                                Caption { text: "NEW GROUP" }
                                Field { id: groupRoot; objectName: "threshold.root"; Accessible.name: "Threshold membership root"; placeholderText: "Membership commitment root (64-character hex)"; Layout.fillWidth: true }
                                RowLayout {
                                    CopyLabel { text: "Members" }
                                    CountInput { id: groupMemberCount; objectName: "threshold.memberCount"; Accessible.name: "Threshold group member count"; value: 3 }
                                    CopyLabel { text: "Required" }
                                    CountInput { id: groupThreshold; objectName: "threshold.threshold"; Accessible.name: "Required approval count"; to: groupMemberCount.value; value: 2 }
                                    Field { id: initialValue; objectName: "threshold.initialValue"; Accessible.name: "Initial parameter value"; placeholderText: "Initial value"; text: "7"; Layout.fillWidth: true }
                                    Action { objectName: "threshold.create"; text: "Create group"; primary: true; Accessible.name: "Create threshold group"; enabled: root.configured && !root.busy; onClicked: root.callBackend(root.backend.createGroup(groupState.text, groupRoot.text, groupMemberCount.value, groupThreshold.value, initialValue.text)) }
                                }
                            }
                        }
                    }
                }
                CopyLabel {
                    Layout.fillWidth: true
                    text: "Membership addresses stay out of public application state. Group settings, activity timing and proof transactions remain observable."
                    font.pixelSize: 11
                    color: Theme.palette.textTertiary
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: statusLabel.implicitHeight + 26
            color: root.backend && root.backend.lastError ? Theme.palette.backgroundMuted : Theme.palette.surfaceRaised
            border.color: root.backend && root.backend.lastError ? Theme.palette.borderSubtle : Theme.palette.borderSubtle
            radius: 8
            CopyLabel {
                id: statusLabel; objectName: "status.text"; Accessible.name: text
                anchors.fill: parent; anchors.margins: 13
                color: root.backend && root.backend.lastError ? Theme.palette.error : Theme.palette.text
                text: (root.backend ? root.backend.statusText : "Waiting for the client...")
                    + (root.busy ? "  " + Math.floor(root.elapsedSeconds / 60) + "m " + (root.elapsedSeconds % 60) + "s elapsed" : "")
            }
        }
        RowLayout {
            CopyLabel { text: "Built for Logos Basecamp"; font.pixelSize: 11; color: Theme.palette.textTertiary }
            Item { Layout.fillWidth: true }
            CopyLabel {
                text: root.captureStatus
                visible: root.technicalExpanded && root.captureStatus.length > 0
                font.pixelSize: 11
                color: Theme.palette.textSecondary
            }
            Action {
                objectName: "evidence.snapshot"
                text: "Save view"
                Accessible.name: "Save only this module view"
                visible: root.technicalExpanded && root.configured
                enabled: !root.captureInFlight && root.backend && root.backend.captureDirectory.length > 0
                implicitHeight: 28
                onClicked: root.captureOwnView()
            }
            Action {
                objectName: "evidence.record"
                text: root.captureRecording ? "Stop recording" : "Record view"
                Accessible.name: "Record only this module view"
                visible: root.technicalExpanded && root.configured
                enabled: root.backend && root.backend.captureDirectory.length > 0
                implicitHeight: 28
                onClicked: root.toggleCaptureRecording()
            }
            Action { text: root.technicalExpanded ? "Hide technical details" : "Technical details"; implicitHeight: 28; onClicked: root.technicalExpanded = !root.technicalExpanded }
        }
        ScrollView {
            visible: root.technicalExpanded
            Layout.fillWidth: true
            Layout.preferredHeight: 140
            TextArea {
                objectName: "result.json"
                readOnly: true
                selectByMouse: true
                text: root.backend ? root.backend.lastResultJson : "{}"
                color: Theme.palette.textSecondary
                font.family: "Menlo"
                font.pixelSize: 11
                wrapMode: TextEdit.Wrap
                background: Rectangle { color: Theme.palette.background; radius: 7 }
            }
        }
        LogosText { id: transientError; objectName: "result.error"; visible: false }
    }
}
