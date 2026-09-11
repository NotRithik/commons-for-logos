import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Logos.Theme
import Logos.Controls
import "WorkspaceState.js" as WorkspaceState

ColumnLayout {
    id: page
    property var host
    readonly property var backend: host ? host.backend : null
    readonly property bool working: host ? host.busy : true
    readonly property var catalog: {
        try { return JSON.parse(backend ? backend.governanceStateJson : "{}") }
        catch (_) { return {} }
    }
    readonly property var activePolicy: {
        try { return JSON.parse(backend ? backend.activeGovernancePolicyJson : "{}") }
        catch (_) { return {} }
    }
    readonly property var identities: catalog.profiles || []
    readonly property var policies: catalog.policies || []
    readonly property bool hasIdentity: !!catalog.selected_profile
    property string mode: "library"
    property bool creatingMembership: false
    readonly property var selectedWorkspace: policies[selectedPolicy] || ({})
    readonly property bool selectedIsMembership: selectedWorkspace.kind === "allowlist"
    readonly property bool selectedIsLive: !!(backend && selectedWorkspace.state_account && (selectedIsMembership
        ? backend.distributionStateAccount === selectedWorkspace.state_account
        : backend.groupStateAccount === selectedWorkspace.state_account))
    property var members: []
    readonly property var localEnrollments: identities.filter(function(row) {
        return row.enrollment && !page.members.some(function(member) {return member.account_id===row.enrollment.account_id})
    })
    property string localError: ""
    property string pendingAction: ""
    property string pendingState: ""
    property string expectedTitle: ""
    property int selectedPolicy: 0
    property int selectionRevision: 0
    property var sharingPolicy: ({})
    signal openDecisionRequested(string stateAccount)
    signal openMembershipRequested(string stateAccount)
    signal reviewCreateRequested(var policy)
    spacing: 16
    onActivePolicyChanged: {
        if (!pendingAction || activePolicy.state_account !== pendingState) return
        const action = pendingAction
        pendingAction = ""
        if (action === "publish") reviewCreateRequested(activePolicy)
        else if (activePolicy.kind === "allowlist") openMembershipRequested(activePolicy.state_account)
        else openDecisionRequested(activePolicy.state_account)
    }
    onCatalogChanged: {
        // Wait for dependent QML bindings to observe the new catalogue.
        Qt.callLater(function() {
            const rows = page.catalog.policies || []
            // Select the exact successful setup result, not the first workspace
            // with a matching label. Two organizers may use the same title.
            const target = page.catalog.selected_workspace || ""
            if (target) {
                for (let i=0; i<rows.length; ++i) {
                    if (rows[i].state_account.toLowerCase() === target.toLowerCase()) {
                        page.selectedPolicy=i
                        page.expectedTitle=""
                        page.mode="library"
                        return
                    }
                }
            }
            if (page.expectedTitle) {
                for (let i=0; i<rows.length; ++i) {
                    if (rows[i].title === page.expectedTitle) {
                        page.selectedPolicy=i
                        page.expectedTitle=""
                        page.mode="library"
                        break
                    }
                }
            }
            if (page.selectedPolicy >= rows.length)
                page.selectedPolicy=Math.max(0,rows.length-1)
        })
    }
    function run(reply) { if (host) host.callBackend(reply) }
    function addMember(value) {
        if (!value || value.schema_version !== 1 || !value.label || !/^[0-9a-f]{64}$/.test(value.account_id || "") || !/^[0-9a-f]{64}$/.test(value.salt || "")) {
            localError="Copy the complete enrollment code from the member. Do not paste a wallet, mnemonic or secret key."; return
        }
        for (let member of members) if (member.account_id === value.account_id) {
            localError="This member is already included."; return
        }
        if (members.length>=256) {localError="A policy supports at most 256 members."; return}
        const updated=members.slice(); updated.push(value); members=updated; localError=""
    }
    function ownInvitation(policy) {
        if (!policy || !page.catalog.enrollment) return null
        for (const invitation of (policy.invitations || []))
            if (invitation.member.account_id === page.catalog.enrollment.account_id) return invitation
        return null
    }
    function choosePolicy(action) {
        if (selectedPolicy<0 || selectedPolicy>=policies.length) return
        pendingAction=action; pendingState=policies[selectedPolicy].state_account
        // Opening is a local selection only. Publication always has a separate
        // exact-intent review in the parent screen.
        run(backend.governanceOpenPolicy(selectedPolicy))
    }
    function preset(which) {
        creatingMembership = which === 3
        members=[]
        enrollmentInput.text=""
        policyTitle.text = which===3 ? "Community membership list" : which===0 ? "Community grant limit" : which===1 ? "Shared storage quota" : "New governance policy"
        policyField.text = which===0 ? "grant_limit_units" : which===1 ? "storage_limit_gb" : "setting"
        policyInitial.text = which===0 ? "100" : which===1 ? "20" : "0"
        mode="create"; localError=""
    }
    component Copy: LogosText {
        textFormat: Text.PlainText
        color: Theme.palette.textSecondary
        wrapMode: Text.WordWrap
        Accessible.role: Accessible.StaticText
        Accessible.name: text
        Layout.fillWidth: true
    }
    component Heading: Copy {
        color: Theme.palette.text
        font.pixelSize: Theme.typography.panelTitleText
        font.weight: Theme.typography.weightMedium
    }
    component Box: Pane {
        padding: 20
        Layout.fillWidth: true
        background: Rectangle {color: Theme.palette.surfaceRaised; radius: Theme.spacing.radiusXlarge}
    }
    component Button: LogosButton {
        property bool primary: false
        variant: primary ? LogosButton.Variant.Primary : LogosButton.Variant.Secondary
        radius: Theme.spacing.radiusXlarge
        enabled: !page.working
        Accessible.name: text
    }
    component Input: TextField {
        implicitHeight: 40
        Layout.fillWidth: true
        color: Theme.palette.text
        font.family: Theme.typography.publicSans
        selectByMouse: true
        placeholderTextColor: Theme.palette.textTertiary
        background: Rectangle {color: Theme.palette.backgroundSecondary; radius: 7; border.color: parent.activeFocus ? Theme.palette.overlayOrange : Theme.palette.border}
    }
    component Area: ScrollView {
        id: area
        property alias text: editor.text
        property alias readOnly: editor.readOnly
        property alias placeholderText: editor.placeholderText
        function selectAll() { editor.selectAll() }
        function copy() { editor.copy() }
        function deselect() { editor.deselect() }
        Layout.fillWidth: true
        implicitHeight: 110
        Layout.preferredHeight: implicitHeight
        Layout.minimumHeight: implicitHeight
        Layout.maximumHeight: implicitHeight
        clip: true
        contentWidth: availableWidth
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AlwaysOn
        ScrollBar.vertical.interactive: true
        background: Rectangle {color: Theme.palette.backgroundSecondary; radius: 7; border.color: Theme.palette.border}
        TextArea {
            id: editor
            objectName: area.objectName + ".text"
            Accessible.name: area.Accessible.name
            selectByMouse: true
            wrapMode: TextEdit.WrapAnywhere
            color: Theme.palette.text
            placeholderTextColor: Theme.palette.textTertiary
            font.pixelSize: 12
            leftPadding: 10; rightPadding: 18; topPadding: 10; bottomPadding: 10
            background: null
        }
    }
    component Picker: WorkspacePicker {
        viewportItem: page.host
        implicitHeight: 40
        Layout.fillWidth: true
        palette.text: Theme.palette.text
        palette.buttonText: Theme.palette.text
        palette.base: Theme.palette.backgroundSecondary
        palette.button: Theme.palette.backgroundSecondary
        palette.window: Theme.palette.surfaceRaised
        palette.mid: Theme.palette.border
        palette.highlight: Theme.palette.backgroundSecondary
        palette.accent: Theme.palette.overlayOrange
        palette.placeholderText: Theme.palette.textTertiary
        enabled: !page.working
    }

    component DialogBox: Dialog {
        id: box
        property string confirmLabel: "Continue"
        property bool confirmEnabled: true
        padding: 20
        background: Rectangle { color: Theme.palette.surfaceRaised; radius: Theme.spacing.radiusLarge; border.color: Theme.palette.border }
        header: LogosText {
            text: box.title; textFormat: Text.PlainText; color: Theme.palette.text
            wrapMode: Text.WordWrap; font.pixelSize: Theme.typography.panelTitleText
            font.weight: Theme.typography.weightMedium; padding: 20
        }
        footer: Pane {
            padding: 20; background: Item {}
            contentItem: RowLayout {
                Button { text: (box.standardButtons & Dialog.Ok) !== 0 ? "Cancel" : "Close"; onClicked: box.reject() }
                Item { Layout.fillWidth: true }
                Button {
                    visible: (box.standardButtons & Dialog.Ok) !== 0
                    text: box.confirmLabel; primary: true
                    enabled: !page.working && box.confirmEnabled
                    onClicked: box.accept()
                }
            }
        }
    }

    Box {
        ColumnLayout {
            anchors.fill: parent; spacing: 12
            Heading {text: "Your membership and decisions"}
            Copy {text: "Create a private membership list or a group decision policy. Invite members without collecting their private keys."}
            Flow {
                Layout.fillWidth: true; spacing: 8
                Button {text: "My workspaces"; onClicked: {page.mode="library";page.localError=""}}
                Button {text: "Create a policy"; enabled: page.hasIdentity && !page.working; onClicked: page.preset(2)}
                Button {text: "Create membership list"; objectName: "membership.newList"; enabled: page.hasIdentity && !page.working; onClicked: page.preset(3)}
                Button {text: "Join with invitation"; enabled: page.hasIdentity && !page.working; onClicked: {page.mode="join";page.localError=""}}
            }
            RowLayout {
                Layout.fillWidth: true
                Picker {
                    id: identityPicker
                    objectName: "governance.identity"
                    Accessible.name: "Your member identity"
                    model: page.identities
                    textRole: "label"
                    displayText: page.catalog.identity_label || "Load or create your identity"
                    onActivated: index => page.run(page.backend.governanceSelectIdentity(page.identities[index].id))
                }
                Button {text: "Load identities"; objectName: "governance.refresh"; onClicked: page.run(page.backend.governanceRefresh())}
                Button {text: "New identity"; objectName: "governance.newIdentity"; onClicked: {identityName.text="";identityDialog.open()}}
            }
            RowLayout {
                visible: page.hasIdentity
                Layout.fillWidth: true
                Copy {text: "Acting as " + (page.catalog.identity_label || "") + ". Switching identities never signs or votes."}
                Button {text: "Share enrollment"; objectName: "governance.shareEnrollment"; onClicked: enrollmentDialog.open()}
            }
        }
    }
    Copy {visible: page.localError.length>0; text: page.localError; color: Theme.palette.error}

    Box {
        visible: page.mode==="library"
        ColumnLayout {
            anchors.fill: parent; spacing: 12
            Heading {text: "Saved workspaces"}
            Copy {visible: !page.hasIdentity; text: "Start by creating a local testnet identity. On separate computers, each participant creates their own identity and shares only their enrollment with the organizer."}
            Copy {visible: page.hasIdentity && page.policies.length===0; text: "No workspaces saved for this identity yet. Create a membership list or decision policy, or import your invitation."}
            Picker {
                id: policyPicker; objectName: "governance.policy"
                Accessible.name: "Saved governance policy"
                visible: page.policies.length>0
                model: page.policies; textRole: "title"
                currentIndex: page.selectedPolicy
                onActivated: index => page.selectedPolicy=index
            }
            Copy {
                visible: page.policies.length>0
                text: {
                    const p=page.policies[page.selectedPolicy]
                    return !p ? "" : p.kind === "allowlist" ? "Private membership list for " + p.member_count + " eligible members. Each member can register once; this does not transfer tokens."
                        : p.threshold+" of "+p.member_count+" approvals control “"+p.field_name.replace(/_/g," ")+"”. Whole-number setting. Starting value: "+p.initial_value+"."
                }
            }
            Flow {
                visible: page.policies.length>0
                Layout.fillWidth: true; spacing: 8
                Button {text: page.selectedIsMembership ? "Open membership list" : "Open decision"; primary: true; objectName: "governance.openDecision"; onClicked: page.choosePolicy("open")}
                Button {
                    text: "Enable my membership"; objectName: "governance.joinOwn"
                    visible: page.policies.length > 0 && !page.policies[page.selectedPolicy].witness_file && !!page.ownInvitation(page.policies[page.selectedPolicy])
                    onClicked: { page.expectedTitle=page.policies[page.selectedPolicy].title; page.run(page.backend.governanceJoinOwnPolicy(page.selectedPolicy)) }
                }
                Button {
                    text: page.selectedIsLive ? "Published on testnet" : "Review first publication"; objectName: "governance.publish"
                    enabled: !page.working && !page.selectedIsLive && !!(page.policies[page.selectedPolicy] && page.policies[page.selectedPolicy].creator)
                    onClicked: page.choosePolicy("publish")
                }
                Button {
                    text: "Member invitations"; objectName: "governance.invitations"
                    onClicked: {page.sharingPolicy=page.policies[page.selectedPolicy]; invitationPicker.currentIndex=0;shareDialog.open()}
                }
            }
            Copy {visible: page.policies.length>0; text: page.policies[page.selectedPolicy] && !page.policies[page.selectedPolicy].witness_file && page.ownInvitation(page.policies[page.selectedPolicy])
                ? "You are included in this workspace. Enable your membership to prepare your private credential on this device. This does not register or vote."
                : "Open this workspace to check its current blockchain status. Saving a draft alone does not publish it; the organizer must review the first publication."}
            Rectangle {Layout.fillWidth: true; implicitHeight: 1; color: Theme.palette.border}
            Heading {text: "Start with an example"}
            Flow {
                Layout.fillWidth: true; spacing: 8
                Button {text: "Membership gate"; enabled: page.hasIdentity && !page.working; onClicked: page.preset(3)}
                Button {text: "Grant limit"; enabled: page.hasIdentity && !page.working; onClicked: page.preset(0)}
                Button {text: "Storage quota"; enabled: page.hasIdentity && !page.working; onClicked: page.preset(1)}
                Button {text: "Custom integer setting"; enabled: page.hasIdentity && !page.working; onClicked: page.preset(2)}
            }
            Copy {text: "Membership lists and shared decisions use separate verified programs. Each saved workspace is its own instance. These examples do not transfer money or provision storage."}
        }
    }
    Box {
        visible: page.mode==="create"
        ColumnLayout {
            anchors.fill: parent; spacing: 12
            Heading {text: page.creatingMembership ? "Create a private membership list" : "Create an approval policy"}
            Input {id: policyTitle; objectName: "governance.title"; Accessible.name: page.creatingMembership ? "Membership list name" : "Policy name"; placeholderText: "Give this workspace a name"; maximumLength: 80}
            Input {id: policyField; visible: !page.creatingMembership; objectName: "governance.field"; Accessible.name: "Controlled setting name"; placeholderText: "Setting name, for example grant_limit_units"; maximumLength: 80}
            RowLayout {
                visible: !page.creatingMembership
                Copy {text: "Whole-number value"; Layout.fillWidth: false}
                Input {id: policyInitial; objectName: "governance.initial"; Accessible.name: "Initial policy value"; placeholderText: "Initial value"; text: "0"; maximumLength: 20}
            }
            Copy {visible: !page.creatingMembership; text: "Policy and setting names are local display labels. The on-chain rule enforces the integer value and the approval threshold; it does not enforce a currency unit, Boolean type or application-specific meaning."}
            Heading {text: "Members ("+page.members.length+")"}
            Button {text: "Add my selected identity"; objectName: "governance.addSelf"; enabled: page.hasIdentity && !page.working; onClicked: page.addMember(page.catalog.enrollment)}
            Repeater {
                model: page.members
                delegate: RowLayout {
                    required property int index
                    required property var modelData
                    Layout.fillWidth: true
                    Copy {text: (index+1)+". "+modelData.label}
                    Button {text: "Remove"; Accessible.name: "Remove member "+modelData.label; onClicked: {const rows=page.members.slice();rows.splice(index,1);page.members=rows}}
                }
            }
            RowLayout {
                visible: page.identities.length > 1
                Layout.fillWidth: true
                Picker { id: localMemberPicker; objectName: "governance.localMember"; Accessible.name: "Member on this device"; model: page.localEnrollments; textRole: "label" }
                Button {
                    text: "Add selected member"; objectName: "governance.addLocalMember"
                    enabled: !page.working && localMemberPicker.currentIndex>=0 && localMemberPicker.currentIndex<page.localEnrollments.length
                    onClicked: page.addMember(page.localEnrollments[localMemberPicker.currentIndex].enrollment)
                }
            }
            Copy {visible: page.identities.length>1; text: "For a one-computer demo, choose another local member by name. This adds only their enrollment, not their signing key. For members on other devices, paste their enrollment below."}
            Area {id: enrollmentInput; objectName: "governance.memberEnrollment"; Accessible.name: "Member enrollment JSON"; placeholderText: "Paste one member's enrollment code"}
            Button {
                text: "Add enrollment"; objectName: "governance.addEnrollment"
                onClicked: {try {page.addMember(JSON.parse(enrollmentInput.text));if (!page.localError) enrollmentInput.text=""} catch (_) {page.localError="This enrollment could not be read. Copy the complete code from the member."}}
            }
            RowLayout {
                visible: !page.creatingMembership
                Copy {text: "Required approvals"; Layout.fillWidth: false}
                SpinBox {
                    id: approvalCount; objectName: "governance.threshold"; Accessible.name: "Required governance approvals"
                    from: 1; to: Math.max(1,page.members.length); value: Math.min(2,to); editable: true
                    palette.text: Theme.palette.text; palette.buttonText: Theme.palette.text; palette.base: Theme.palette.backgroundSecondary
                    enabled: !page.working
                }
                Copy {text: "of "+page.members.length+" distinct members"}
            }
            Copy {text: "Only enrollment data is combined here. Every participant uses their own invitation and private account to register or approve. The organizer does not receive their signing keys."}
            Button {
                text: page.creatingMembership ? "Prepare membership list" : "Prepare policy draft"; primary: true; objectName: "governance.prepare"
                enabled: !page.working && page.hasIdentity && page.members.length>0 && policyTitle.text.trim().length>0 && (page.creatingMembership || (policyField.text.trim().length>0 && WorkspaceState.signedValue(policyInitial.text)))
                onClicked: {
                    page.localError=""; page.expectedTitle=policyTitle.text.trim()
                    page.run(page.creatingMembership
                        ? page.backend.membershipPrepareList(policyTitle.text, JSON.stringify(page.members))
                        : page.backend.governancePreparePolicy(policyTitle.text,policyField.text,policyInitial.text,approvalCount.value,JSON.stringify(page.members)))
                }
            }
            Copy {visible: !page.creatingMembership && policyInitial.text.length>0 && !WorkspaceState.signedValue(policyInitial.text); color: Theme.palette.error; text: "Enter a whole number between -9223372036854775808 and 9223372036854775807. Do not use decimals or exponent notation."}
            Copy {text: "Preparing a draft saves the workspace and individual invitations on this computer. Nothing is published, registered or voted on until you review a separate blockchain action."}
        }
    }
    Box {
        visible: page.mode==="join"
        ColumnLayout {
            anchors.fill: parent; spacing: 12
            Heading {text: "Use your member invitation"}
            Copy {text: "Select the identity named in your enrollment, then paste the individual invitation from the organizer. The app verifies the membership path and derives the private credential locally."}
            Area {id: joinInput; objectName: "governance.invitationInput"; Accessible.name: "Policy invitation JSON"; placeholderText: "Paste the complete invitation code from the organizer"; implicitHeight: 180}
            Button {text: "Verify and save invitation"; primary: true; objectName: "governance.join"; enabled: !page.working && page.hasIdentity && joinInput.text.trim().length>0; onClicked: { try { const i=JSON.parse(joinInput.text); page.expectedTitle=i.title || "" } catch (_) {} page.run(page.backend.governanceJoinPolicy(joinInput.text)) }}
            Copy {text: "Importing an invitation does not register or vote. Open the saved workspace to review the available action. Only your own member identity can use this invitation."}
        }
    }

    DialogBox {
        id: identityDialog
        confirmLabel: "Create identity"
        confirmEnabled: identityName.text.trim().length > 0
        title: "Create a local testnet identity"
        modal: true; anchors.centerIn: Overlay.overlay; width: Math.min(560,page.host.width-40)
        standardButtons: Dialog.Ok | Dialog.Cancel
        contentItem: ColumnLayout {
            spacing: 12
            Copy {text: "A new member wallet will be saved on this computer, protected by your operating-system account permissions. Existing wallets are never replaced. This demo wallet is not encrypted or intended for real money."}
            Input {id: identityName; objectName: "governance.identityName"; Accessible.name: "New identity label"; placeholderText: "A name for this identity"; maximumLength: 80}
            Copy {text: "No faucet request, funding, proof or blockchain transaction will be submitted."}
        }
        onAccepted: page.run(page.backend.governanceCreateIdentity(identityName.text))
    }
    DialogBox {
        id: enrollmentDialog
        title: "Share your enrollment with the organizer"
        modal: true; anchors.centerIn: Overlay.overlay; width: Math.min(650,page.host.width-40)
        standardButtons: Dialog.Close
        contentItem: ColumnLayout {
            spacing: 12
            Copy {text: "This contains your member address and membership salt, not your secret key. Share it privately with the organizer. Never send your wallet folder, storage.json or private credential."}
            Area {id: enrollmentOutput; objectName: "governance.enrollmentOutput"; Accessible.name: "Public member enrollment"; readOnly: true; text: JSON.stringify(page.catalog.enrollment || {},null,2); implicitHeight: 180}
            Button {text: "Copy enrollment"; onClicked: {enrollmentOutput.selectAll();enrollmentOutput.copy();enrollmentOutput.deselect()}}
        }
    }
    DialogBox {
        id: shareDialog
        title: "Individual member invitations"
        modal: true; anchors.centerIn: Overlay.overlay; width: Math.min(700,page.host.width-40)
        standardButtons: Dialog.Close
        contentItem: ColumnLayout {
            spacing: 12
            Copy {text: "Give each member only their own invitation. It proves membership in this exact policy and contains no private signing key. On-chain activity does not identify which member approved."}
            Picker {id: invitationPicker; objectName: "governance.invitationMember"; Accessible.name: "Invitation recipient"; model: (page.sharingPolicy.invitations || []).map(x=>x.member.label)}
            Area {id: invitationOutput; objectName: "governance.invitationOutput"; Accessible.name: "Individual policy invitation"; readOnly: true; text: JSON.stringify((page.sharingPolicy.invitations || [])[invitationPicker.currentIndex] || {},null,2); implicitHeight: 230}
            Button {text: "Copy invitation"; onClicked: {invitationOutput.selectAll();invitationOutput.copy();invitationOutput.deselect()}}
        }
    }
}
