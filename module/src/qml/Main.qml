import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: root
    objectName: "astra.root"

    readonly property var backend: logos.module("astra_primitives_ui")
    property bool ready: false
    property bool recordingEvidence: false
    property int capturedFrames: 0
    function captureOwnView() {
        if (!root.backend || !root.backend.captureDirectory) return
        root.grabToImage(function(image) {
            const filename = root.backend.captureDirectory + "/astra-view-" + Date.now() + ".png"
            if (image.saveToFile(filename)) root.capturedFrames++
        })
    }
    Timer {
        interval: 1000
        repeat: true
        running: root.recordingEvidence && root.configured
        onTriggered: root.captureOwnView()
    }
    readonly property bool configured: ready && backend && backend.configured
    readonly property bool busy: ready && backend && backend.busy
    property string pendingWitnessTarget: "allowlist"

    function localPath(url) {
        var text = String(url || "")
        if (text.indexOf("file://") === 0) {
            if (text.indexOf("file:///") === 0)
                return decodeURIComponent(text.substring(7))
            return decodeURIComponent(text.substring(5))
        }
        return text
    }

    function callBackend(reply) {
        if (!logos || !logos.watch)
            return
        logos.watch(reply, function() {}, function(error) {
            transientError.text = String(error)
        })
    }

    Connections {
        target: logos
        function onViewModuleReadyChanged(moduleName, isReady) {
            if (moduleName === "astra_primitives_ui")
                root.ready = isReady && root.backend !== null
        }
    }

    Connections {
        target: root.backend
        function onOperationFinished(operation, resultJson) {
            transientError.text = ""
        }
        function onOperationFailed(operation, errorMessage) {
            transientError.text = errorMessage
        }
    }

    Component.onCompleted: {
        root.ready = root.backend !== null && logos.isViewModuleReady("astra_primitives_ui")
    }

    FileDialog {
        id: cliDialog
        title: "Select astra-logos-cli"
        fileMode: FileDialog.OpenFile
        onAccepted: cliPath.text = root.localPath(selectedFile)
    }

    FolderDialog {
        id: walletDialog
        title: "Select testnet wallet directory"
        onAccepted: walletDir.text = root.localPath(selectedFolder)
    }

    FileDialog {
        id: witnessDialog
        title: "Select witness inside the configured testnet wallet folder"
        fileMode: FileDialog.OpenFile
        onAccepted: {
            var selected = root.localPath(selectedFile)
            if (root.pendingWitnessTarget === "threshold")
                root.callBackend(root.backend.selectThresholdWitness(selected))
            else
                root.callBackend(root.backend.selectAllowlistWitness(selected))
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "#101316"
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 12

        RowLayout {
            Layout.fillWidth: true
            spacing: 12

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Label {
                    text: "Astra Primitives - Testnet only"
                    color: "#f6f1e8"
                    font.pixelSize: 22
                    font.weight: Font.DemiBold
                }
                Label {
                    text: !root.ready ? "Offline" : (root.configured ? "Testnet CLI configured" : "Not configured")
                    color: root.configured ? "#58c4b8" : "#d99b32"
                    font.pixelSize: 12
                }
            }

            Button {
                objectName: "evidence.snapshot"
                Accessible.name: "Save screenshot of this module only"
                text: "Snapshot"
                enabled: root.configured && !!root.backend.captureDirectory
                onClicked: root.captureOwnView()
            }
            Button {
                objectName: "evidence.record"
                Accessible.name: "Record only this module view"
                text: root.recordingEvidence ? "Stop recording" : "Record demo"
                enabled: root.configured && !!root.backend.captureDirectory
                onClicked: { root.recordingEvidence = !root.recordingEvidence; root.captureOwnView() }
            }
            Label {
                text: root.capturedFrames ? root.capturedFrames + " frames saved" : ""
                color: "#aab4b8"
            }
            BusyIndicator {
                running: root.busy
                visible: root.busy
                Layout.preferredWidth: 28
                Layout.preferredHeight: 28
            }
        }

        Rectangle {
            Layout.fillWidth: true
            radius: 6
            color: "#181d21"
            border.color: "#2b3338"
            implicitHeight: configGrid.implicitHeight + 24

            GridLayout {
                id: configGrid
                anchors.fill: parent
                anchors.margins: 12
                columns: 7
                columnSpacing: 8
                rowSpacing: 8

                Label { text: "CLI"; color: "#aab4b8"; Layout.alignment: Qt.AlignVCenter }
                TextField {
                    id: cliPath
                    objectName: "config.cliPath"
                            Accessible.name: "Testnet CLI executable"
                    Layout.fillWidth: true
                    Layout.columnSpan: 2
                    placeholderText: "/absolute/path/astra-logos-cli"
                    selectByMouse: true
                    color: "#f6f1e8"
                }
                Button {
                    objectName: "config.cliBrowse"
                            Accessible.name: "Browse CLI executable"
                    text: "Browse"
                    onClicked: cliDialog.open()
                }

                Label { text: "Wallet"; color: "#aab4b8"; Layout.alignment: Qt.AlignVCenter }
                TextField {
                    id: walletDir
                    objectName: "config.walletDir"
                            Accessible.name: "Testnet wallet directory"
                    Layout.fillWidth: true
                    placeholderText: "/absolute/path/testnet-wallet"
                    selectByMouse: true
                    color: "#f6f1e8"
                }
                Button {
                    objectName: "config.walletBrowse"
                            Accessible.name: "Browse testnet wallet directory"
                    text: "Browse"
                    onClicked: walletDialog.open()
                }

                Item { Layout.columnSpan: 5; Layout.fillWidth: true }
                Button {
                    objectName: "config.apply"
                            Accessible.name: "Configure testnet backend"
                    text: "Configure"
                    enabled: root.ready && !root.busy
                    highlighted: true
                    Layout.columnSpan: 2
                    onClicked: root.callBackend(root.backend.configure(cliPath.text, walletDir.text))
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            radius: 6
            color: root.busy ? "#1d2830" : "#15191d"
            border.color: root.backend && root.backend.lastError && root.backend.lastError.length > 0 ? "#9b4747" : "#2b3338"
            implicitHeight: statusText.implicitHeight + 22

            Label {
                id: statusText
                objectName: "status.text"
                            Accessible.name: text
                anchors.fill: parent
                anchors.margins: 11
                text: root.backend ? root.backend.statusText : "Offline. Waiting for backend."
                color: root.backend && root.backend.lastError && root.backend.lastError.length > 0 ? "#ffb4a8" : "#cbd5d7"
                wrapMode: Text.Wrap
                font.pixelSize: 13
            }
        }

        TabBar {
            id: tabs
            Layout.fillWidth: true
            TabButton { objectName: "tabs.allowlist"; text: "Allowlist" }
            TabButton { objectName: "tabs.threshold"; text: "Threshold" }
        }

        StackLayout {
            currentIndex: tabs.currentIndex
            Layout.fillWidth: true
            Layout.fillHeight: true

            ScrollView {
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    width: parent ? parent.width : 720
                    spacing: 12

                    GridLayout {
                        Layout.fillWidth: true
                        columns: 4
                        columnSpacing: 10
                        rowSpacing: 10

                        Label { text: "Merkle root"; color: "#aab4b8" }
                        TextField {
                            id: allowRoot
                            objectName: "allowlist.root"
                            Accessible.name: "Allowlist Merkle root"
                            Layout.fillWidth: true
                            Layout.columnSpan: 3
                            placeholderText: "64-character hex (32 bytes) root"
                            selectByMouse: true
                            color: "#f6f1e8"
                        }

                        Label { text: "Members"; color: "#aab4b8" }
                        SpinBox {
                            id: allowMemberCount
                            objectName: "allowlist.memberCount"
                            Accessible.name: "Eligible member count"
                            from: 1
                            to: 256
                            value: 10
                            editable: true
                        }
                        Button {
                            objectName: "allowlist.create"
                            Accessible.name: "Create distribution"
                            text: "Create distribution"
                            enabled: root.configured && !root.busy
                            highlighted: true
                            Layout.columnSpan: 2
                            onClicked: root.callBackend(root.backend.createDistribution(allowState.text, allowRoot.text, allowMemberCount.value))
                        }

                        Label { text: "State"; color: "#aab4b8" }
                        TextField {
                            id: allowState
                            objectName: "allowlist.stateAccount"
                            Accessible.name: "Allowlist state account"
                            Layout.fillWidth: true
                            Layout.columnSpan: 3
                            text: root.backend ? root.backend.distributionStateAccount : ""
                            placeholderText: "64-character hex (32 bytes) state account"
                            selectByMouse: true
                            color: "#f6f1e8"
                        }

                        Label { text: "Witness"; color: "#aab4b8" }
                        Label {
                            objectName: "allowlist.witnessLabel"
                            Accessible.name: text
                            text: root.backend ? root.backend.allowlistWitnessLabel : "No witness selected"
                            color: "#f6f1e8"
                            elide: Text.ElideMiddle
                            Layout.fillWidth: true
                        }
                        Button {
                            objectName: "allowlist.witnessButton"
                            Accessible.name: "Choose private claim witness"
                            text: "Select"
                            enabled: root.ready && !root.busy
                            onClicked: {
                                root.pendingWitnessTarget = "allowlist"
                                witnessDialog.open()
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Button {
                                objectName: "allowlist.claim"
                            Accessible.name: "Claim allocation privately"
                                text: "Claim"
                                enabled: root.configured && !root.busy
                                onClicked: root.callBackend(root.backend.claimAllowlist(allowState.text))
                            }
                            Button {
                                objectName: "allowlist.inspect"
                            Accessible.name: "Inspect allowlist state"
                                text: "Inspect"
                                enabled: root.configured && !root.busy
                                onClicked: root.callBackend(root.backend.inspectDistribution(allowState.text))
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        radius: 6
                        color: "#15191d"
                        border.color: "#2b3338"
                        implicitHeight: allowSummary.implicitHeight + 22
                        Label {
                            id: allowSummary
                            objectName: "allowlist.summary"
                            Accessible.name: text
                            anchors.fill: parent
                            anchors.margins: 11
                            text: root.backend ? root.backend.distributionSummary : "No live distribution state loaded."
                            color: "#cbd5d7"
                            wrapMode: Text.Wrap
                        }
                    }
                }
            }

            ScrollView {
                clip: true
                contentWidth: availableWidth

                ColumnLayout {
                    width: parent ? parent.width : 720
                    spacing: 12

                    GridLayout {
                        Layout.fillWidth: true
                        columns: 4
                        columnSpacing: 10
                        rowSpacing: 10

                        Label { text: "Merkle root"; color: "#aab4b8" }
                        TextField {
                            id: groupRoot
                            objectName: "threshold.root"
                            Accessible.name: "Threshold membership root"
                            Layout.fillWidth: true
                            Layout.columnSpan: 3
                            placeholderText: "64-character hex (32 bytes) root"
                            selectByMouse: true
                            color: "#f6f1e8"
                        }

                        Label { text: "Members"; color: "#aab4b8" }
                        SpinBox {
                            id: groupMemberCount
                            objectName: "threshold.memberCount"
                            Accessible.name: "Threshold group member count"
                            from: 1
                            to: 256
                            value: 10
                            editable: true
                        }
                        Label { text: "Threshold"; color: "#aab4b8" }
                        SpinBox {
                            id: groupThreshold
                            objectName: "threshold.threshold"
                            Accessible.name: "Required approval count"
                            from: 1
                            to: groupMemberCount.value
                            value: Math.min(3, groupMemberCount.value)
                            editable: true
                        }

                        Label { text: "Initial"; color: "#aab4b8" }
                        TextField {
                            id: initialValue
                            objectName: "threshold.initialValue"
                            Accessible.name: "Initial parameter value"
                            Layout.fillWidth: true
                            placeholderText: "7"
                            text: "7"
                            validator: RegularExpressionValidator { regularExpression: /^-?(0|[1-9][0-9]{0,18})$/ }
                            color: "#f6f1e8"
                        }
                        Button {
                            objectName: "threshold.create"
                            Accessible.name: "Create threshold group"
                            text: "Create group"
                            enabled: root.configured && !root.busy
                            highlighted: true
                            Layout.columnSpan: 2
                            onClicked: root.callBackend(root.backend.createGroup(groupState.text, groupRoot.text, groupMemberCount.value, groupThreshold.value, initialValue.text))
                        }

                        Label { text: "State"; color: "#aab4b8" }
                        TextField {
                            id: groupState
                            objectName: "threshold.stateAccount"
                            Accessible.name: "Threshold state account"
                            Layout.fillWidth: true
                            Layout.columnSpan: 3
                            text: root.backend ? root.backend.groupStateAccount : ""
                            placeholderText: "64-character hex (32 bytes) state account"
                            selectByMouse: true
                            color: "#f6f1e8"
                        }

                        Label { text: "Witness"; color: "#aab4b8" }
                        Label {
                            objectName: "threshold.witnessLabel"
                            Accessible.name: text
                            text: root.backend ? root.backend.thresholdWitnessLabel : "No witness selected"
                            color: "#f6f1e8"
                            elide: Text.ElideMiddle
                            Layout.fillWidth: true
                        }
                        Button {
                            objectName: "threshold.witnessButton"
                            Accessible.name: "Choose private approval witness"
                            text: "Select"
                            enabled: root.ready && !root.busy
                            onClicked: {
                                root.pendingWitnessTarget = "threshold"
                                witnessDialog.open()
                            }
                        }
                        Item { Layout.fillWidth: true }

                        Label { text: "Proposal"; color: "#aab4b8" }
                        TextField {
                            id: nextValue
                            objectName: "threshold.nextValue"
                            Accessible.name: "Proposed parameter value"
                            Layout.fillWidth: true
                            placeholderText: "next integer value"
                            validator: RegularExpressionValidator { regularExpression: /^-?(0|[1-9][0-9]{0,18})$/ }
                            color: "#f6f1e8"
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.columnSpan: 2
                            spacing: 8
                            Button {
                                objectName: "threshold.propose"
                            Accessible.name: "Propose parameter change"
                                text: "Propose"
                                enabled: root.configured && !root.busy
                                onClicked: root.callBackend(root.backend.proposeParameter(groupState.text, nextValue.text))
                            }
                            Button {
                                objectName: "threshold.approve"
                            Accessible.name: "Approve privately"
                                text: "Approve"
                                enabled: root.configured && !root.busy
                                onClicked: root.callBackend(root.backend.approveParameter(groupState.text))
                            }
                            Button {
                                objectName: "threshold.execute"
                            Accessible.name: "Execute approved proposal"
                                text: "Execute"
                                enabled: root.configured && !root.busy
                                onClicked: root.callBackend(root.backend.executeParameter(groupState.text))
                            }
                            Button {
                                objectName: "threshold.inspect"
                            Accessible.name: "Inspect threshold state"
                                text: "Inspect"
                                enabled: root.configured && !root.busy
                                onClicked: root.callBackend(root.backend.inspectGroup(groupState.text))
                            }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        radius: 6
                        color: "#15191d"
                        border.color: "#2b3338"
                        implicitHeight: groupSummary.implicitHeight + 22
                        Label {
                            id: groupSummary
                            objectName: "threshold.summary"
                            Accessible.name: text
                            anchors.fill: parent
                            anchors.margins: 11
                            text: root.backend ? root.backend.groupSummary : "No live group state loaded."
                            color: "#cbd5d7"
                            wrapMode: Text.Wrap
                        }
                    }
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.preferredHeight: 142
            orientation: Qt.Horizontal

            Rectangle {
                SplitView.fillWidth: true
                color: "#15191d"
                border.color: "#2b3338"
                radius: 6
                TextArea {
                    objectName: "result.json"
                            Accessible.name: "result.json"
                    anchors.fill: parent
                    anchors.margins: 8
                    readOnly: true
                    wrapMode: TextEdit.Wrap
                    text: root.backend ? root.backend.lastResultJson : "{}"
                    color: "#cbd5d7"
                    background: null
                    font.family: "Menlo"
                    font.pixelSize: 11
                }
            }

            Rectangle {
                SplitView.preferredWidth: 260
                color: "#15191d"
                border.color: "#2b3338"
                radius: 6
                Label {
                    id: transientError
                    objectName: "result.error"
                            Accessible.name: text
                    anchors.fill: parent
                    anchors.margins: 10
                    text: root.backend ? root.backend.lastError : ""
                    color: "#ffb4a8"
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                }
            }
        }
    }
}
