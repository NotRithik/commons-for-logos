.pragma library

// These hints prevent invalid UI actions. The native CLI and chain still decide
// authorization and validity; no hint is permission to send a transaction.
function signedValue(value) {
    // Match cli::validation::integer without converting through a JS double.
    // Preserve all 64 bits and reject whitespace, exponent notation and + signs.
    if (typeof value !== "string" || !/^-?(0|[1-9][0-9]{0,18})$/.test(value)) return false
    const negative = value.charAt(0) === "-"
    const magnitude = negative ? value.slice(1) : value
    return magnitude.length < 19 || magnitude <= (negative ? "9223372036854775808" : "9223372036854775807")
}
function readState(raw, account, loadedAccount) {
    if (typeof account !== "string" || !/^[a-fA-F0-9]{64}$/.test(account.trim())
        || account.trim().toLowerCase() !== String(loadedAccount).toLowerCase()) return null
    try {
        const state = JSON.parse(raw)
        return state && typeof state === "object" && !Array.isArray(state) ? state : null
    } catch (_) { return null }
}
function count(value) { return typeof value === "number" && value >= 0 && value <= 256 && value % 1 === 0 }
function group(raw, account, loadedAccount) {
    const s = readState(raw, account, loadedAccount)
    const blocked = {known: false, propose: false, approve: false, execute: false,
        message: "Refresh status to load this group's current decision."}
    if (!s || !count(s.member_count) || !count(s.threshold) || s.threshold < 1
        || s.threshold > s.member_count || !signedValue(s.value)) return blocked
    if (s.proposal === null) return {known: true, propose: true, approve: false, execute: false,
        message: "No proposal is waiting. Current value: " + s.value + ". Choose a credential and enter a value to propose a change."}
    const p = s.proposal
    if (!p || typeof p.executed !== "boolean" || !count(p.approvals_count)
        || p.approvals_count > s.member_count || !signedValue(p.next_value)) return blocked
    if (p.executed) return {known: true, propose: true, approve: false, execute: false,
        message: "Decision applied. Current value: " + s.value + ". This proposal cannot be approved or executed again."}
    const enough = p.approvals_count >= s.threshold
    return {known: true, propose: false, approve: !enough, execute: enough,
        message: "Proposed value: " + p.next_value + ". Approvals: " + p.approvals_count + " of " + s.threshold
            + (enough ? ". Ready to apply the decision." : ". More distinct member approvals are needed.")}
}
function membership(raw, account, loadedAccount) {
    const s = readState(raw, account, loadedAccount)
    if (!s || !count(s.member_count) || s.member_count < 1 || !count(s.claims_count)
        || s.claims_count > s.member_count) return {known: false, register: false,
            message: "Refresh status to load the current membership list."}
    return {known: true, register: s.claims_count < s.member_count,
        message: s.claims_count + " of " + s.member_count + " members registered."
            + (s.claims_count === s.member_count ? " This list is full; no registrations remain." : " Each eligible member may register once.")}
}
