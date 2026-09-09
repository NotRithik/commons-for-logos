import QtQuick
import QtTest
import "../../src/qml/WorkspaceState.js" as Rules

TestCase {
    name: "WorkspaceState"
    property string account: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    function state(proposal) {
        return JSON.stringify({member_count: 3, threshold: 2, value: "42", proposal: proposal})
    }
    function test_i64_validation_keeps_exact_integer_precision() {
        for (const value of ["0", "-0", "42", "-1", "9007199254740993", "-9007199254740993", "9223372036854775807", "-9223372036854775808"])
            verify(Rules.signedValue(value), value)
        for (const value of ["", "abc", "+1", "1.5", "1e3", "01", "-01", " 42", "42 ", "9223372036854775808", "-9223372036854775809", "18446744073709551615", 42])
            verify(!Rules.signedValue(value), String(value))
    }
    function test_negative_group_values_remain_usable() {
        const raw = JSON.stringify({member_count:3, threshold:2, value:"-7", proposal:null})
        const result = Rules.group(raw, account, account)
        verify(result.known); verify(result.propose)
        const pending = JSON.stringify({member_count:3, threshold:2, value:"-7", proposal:{executed:false, approvals_count:1, next_value:"-9223372036854775808"}})
        verify(Rules.group(pending, account, account).approve)
    }
    function test_values_beyond_program_range_do_not_enable_actions() {
        for (const value of ["9223372036854775808", "-9223372036854775809"]) {
            const raw = JSON.stringify({member_count:3, threshold:2, value:value, proposal:null})
            verify(!Rules.group(raw, account, account).known)
        }
    }
    function test_executed_decision_cannot_be_approved_or_executed_again() {
        const result = Rules.group(state({executed: true, approvals_count: 2, next_value: "42"}), account, account)
        verify(result.known); verify(result.propose)
        verify(!result.execute); verify(!result.approve)
        verify(result.message.indexOf("Decision applied") >= 0)
    }
    function test_incomplete_threshold_can_only_be_approved() {
        const result = Rules.group(state({executed: false, approvals_count: 1, next_value: "43"}), account, account)
        verify(!result.propose); verify(!result.execute); verify(result.approve)
    }
    function test_satisfied_threshold_can_only_be_executed() {
        const result = Rules.group(state({executed: false, approvals_count: 2, next_value: "43"}), account, account)
        verify(!result.propose); verify(result.execute); verify(!result.approve)
    }
    function test_new_group_allows_only_proposal() {
        const result = Rules.group(state(null), account, account)
        verify(result.propose); verify(!result.execute); verify(!result.approve)
    }
    function test_changed_account_or_missing_data_disarms_actions() {
        const raw = state({executed: false, approvals_count: 2, next_value: "43"})
        for (const args of [[raw, account, ""], ["{}", account, account], ["malformed", account, account]]) {
            const result = Rules.group(args[0], args[1], args[2])
            verify(!result.known); verify(!result.propose); verify(!result.execute); verify(!result.approve)
        }
    }
    function test_malformed_receipt_cannot_enable_actions() {
        const raw = JSON.stringify({member_count: 3, threshold: 7, value: "42", proposal: null})
        verify(!Rules.group(raw, account, account).known)
        verify(!Rules.group(state({executed: "false", approvals_count: 2, next_value: "43"}), account, account).known)
    }
    function test_full_membership_list_has_no_remaining_registrations() {
        const result = Rules.membership('{"member_count":10,"claims_count":10}', account, account)
        verify(result.known); verify(!result.register); verify(result.message.indexOf("list is full") >= 0)
    }
    function test_membership_requires_valid_matching_state() {
        verify(Rules.membership('{"member_count":10,"claims_count":4}', account, account).register)
        verify(!Rules.membership('{"member_count":10,"claims_count":11}', account, account).register)
        verify(!Rules.membership('{"member_count":10,"claims_count":4}', account, "").register)
    }
}
