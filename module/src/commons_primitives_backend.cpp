#include "commons_primitives_backend.h"

#include <QFileInfo>
#include <QFile>
#include <QRegularExpression>
#include <QTimer>
#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>

namespace {

QString compactJson(const QJsonObject& object)
{
    return QString::fromUtf8(QJsonDocument(object).toJson(QJsonDocument::Compact));
}

QString prettyJson(const QJsonObject& object)
{
    return QString::fromUtf8(QJsonDocument(object).toJson(QJsonDocument::Indented));
}

QString stringValue(const QJsonObject& object, const QString& key)
{
    const QJsonValue value = object.value(key);
    if (value.isString())
        return value.toString();
    if (value.isDouble())
        return QString::number(value.toDouble(), 'f', 0);
    return {};
}

int intValue(const QJsonObject& object, const QString& key, const int fallback = -1)
{
    const QJsonValue value = object.value(key);
    if (value.isDouble())
        return value.toInt();
    if (value.isArray())
        return value.toArray().size();
    return fallback;
}

QJsonObject stateObject(const QJsonObject& result)
{
    const QJsonObject nested = result.value(QStringLiteral("state")).toObject();
    return nested.isEmpty() ? result : nested;
}

void insertIfPresent(QStringList& parts, const QString& label, const QString& value)
{
    if (!value.isEmpty())
        parts.append(QStringLiteral("%1 %2").arg(label, value));
}

} // namespace

CommonsPrimitivesBackend::CommonsPrimitivesBackend()
{
    reloadSavedProfiles();
    // An optional operator-provided profile avoids re-entering paths on every
    // Basecamp restart. Normal validation still runs before accepting it.
    const auto defaultCli = qEnvironmentVariable("COMMONS_DEFAULT_CLI");
    const auto defaultWallet = qEnvironmentVariable("COMMONS_DEFAULT_WALLET");
    if (!defaultCli.isEmpty() && !defaultWallet.isEmpty())
        QTimer::singleShot(0, this, [this, defaultCli, defaultWallet]() {
            configure(defaultCli, defaultWallet);
        });
    connect(&m_client,
            &CommonsLogos::LogosCliClient::completed,
            this,
            [this](const QString&, const QString& operation, const QJsonObject& result) {
                completeOperation(operation, result);
            });
    connect(&m_client,
            &CommonsLogos::LogosCliClient::failed,
            this,
            [this](const QString&, const QString& operation, const QString& errorMessage) {
                failOperation(operation, errorMessage);
            });
}

void CommonsPrimitivesBackend::resetSessionState()
{
    setActiveProfileLabel(QString());
    setActiveProfileKind(QString());
    setActiveProfileAccount(QString());
    setReadOnly(false);
    setConnectionCliPath(QString());
    setConnectionWalletDir(QString());
    m_allowlistWitnessPath.clear();
    m_thresholdWitnessPath.clear();
    setAllowlistWitnessLabel(QStringLiteral("No witness selected"));
    setThresholdWitnessLabel(QStringLiteral("No witness selected"));
    setDistributionStateAccount(QString());
    setGroupStateAccount(QString());
    setDistributionSummary(QStringLiteral("No live distribution state loaded."));
    setGroupSummary(QStringLiteral("No live group state loaded."));
    setCaptureDirectory(QString());
    setLastOperation(QString());
    setActiveOperation(QString());
    setLastError(QString());
    setLastResultJson(QStringLiteral("{}"));
}

void CommonsPrimitivesBackend::clearStateForOperation(const QString& operation)
{
    if (operation.startsWith(QStringLiteral("allowlist."))) {
        setDistributionStateAccount(QString());
        setDistributionSummary(QStringLiteral("Inspect a distribution to load its current state."));
    } else if (operation.startsWith(QStringLiteral("threshold."))) {
        setGroupStateAccount(QString());
        setGroupSummary(QStringLiteral("Inspect a group to load its current state."));
    }
}

QString CommonsPrimitivesBackend::reloadSavedProfiles()
{
    // The operator opts in to a local catalog. A catalog is navigation metadata,
    // never authority to submit a transaction or reveal a membership witness.
    if (m_client.isBusy())
        return compactJson(QJsonObject{{"loaded", false}, {"error", "Wait for the current operation."}});
    m_savedProfiles = QJsonArray();
    setSavedProfilesJson(QStringLiteral("[]"));
    const auto filename = qEnvironmentVariable("COMMONS_UI_PROFILES");
    if (filename.isEmpty())
        return compactJson(QJsonObject{{"loaded", true}, {"count", 0}});
    const QFileInfo info(filename);
    QFile file(filename);
    if (!info.isAbsolute() || !info.isFile() || info.isSymLink()
        || info.size() > 32768 || !file.open(QIODevice::ReadOnly)) {
        const QString error = QStringLiteral("Saved workspaces could not be read. Use Connection settings or check the local catalog.");
        setLastError(error);
        setStatusText(error);
        return compactJson(QJsonObject{{"loaded", false}, {"error", error}});
    }
    const auto document = QJsonDocument::fromJson(file.readAll());
    const auto object = document.object();
    const auto values = object.value("profiles").toArray();
    static const QRegularExpression accountPattern(QStringLiteral("^[0-9a-fA-F]{64}$"));
    QJsonArray accepted;
    QJsonArray display;
    bool valid = document.isObject() && object.value("version").toInt() == 1
        && object.value("profiles").isArray() && values.size() <= 32;
    for (const auto& value : values) {
        const auto profile = value.toObject();
        const auto kind = profile.value("kind").toString();
        const auto label = profile.value("label").toString().trimmed();
        const auto account = profile.value("state_account").toString().toLower();
        valid = valid && value.isObject() && !label.isEmpty() && label.size() <= 100
            && (kind == "allowlist" || kind == "threshold")
            && accountPattern.match(account).hasMatch()
            && QFileInfo(profile.value("cli_path").toString()).isAbsolute()
            && QFileInfo(profile.value("wallet_dir").toString()).isAbsolute();
        if (!valid)
            break;
        accepted.append(profile);
        display.append(QJsonObject{{"label", label}, {"kind", kind}, {"state_account", account}});
    }
    if (!valid) {
        const QString error = QStringLiteral("The saved workspace catalog is invalid. No workspace was loaded.");
        setLastError(error);
        setStatusText(error);
        return compactJson(QJsonObject{{"loaded", false}, {"error", error}});
    }
    m_savedProfiles = accepted;
    setSavedProfilesJson(QString::fromUtf8(QJsonDocument(display).toJson(QJsonDocument::Compact)));
    return compactJson(QJsonObject{{"loaded", true}, {"count", accepted.size()}});
}

QString CommonsPrimitivesBackend::openSavedProfile(int index)
{
    if (m_client.isBusy())
        return compactJson(QJsonObject{{"opened", false}, {"error", "Wait for the current operation."}});
    if (index < 0 || index >= m_savedProfiles.size()) {
        const QString error = QStringLiteral("Choose a saved workspace from the list.");
        setLastError(error);
        setStatusText(error);
        return compactJson(QJsonObject{{"opened", false}, {"error", error}});
    }
    const auto profile = m_savedProfiles.at(index).toObject();
    const auto response = QJsonDocument::fromJson(configure(profile.value("cli_path").toString(),
        profile.value("wallet_dir").toString()).toUtf8()).object();
    if (!response.value("configured").toBool())
        return compactJson(QJsonObject{{"opened", false}, {"error", lastError()}});
    const auto kind = profile.value("kind").toString();
    const auto account = profile.value("state_account").toString().toLower();
    setActiveProfileLabel(profile.value("label").toString().trimmed());
    setActiveProfileKind(kind);
    setActiveProfileAccount(account);
    // Selecting a workspace only reads chain state. Credentials remain unselected
    // and every proof-producing action still requires a deliberate button press.
    if (kind == "allowlist")
        inspectDistribution(account);
    else
        inspectGroup(account);
    return compactJson(QJsonObject{{"opened", true}, {"read_only", true}});
}

QString CommonsPrimitivesBackend::configure(QString cliPath, QString walletDir)
{
    if (m_client.isBusy())
        return reject(CommonsLogos::PrimitiveOperation::AllowlistInspect,
                      QStringLiteral("Wait for the current CLI operation to finish."));

    // A new profile must not retain the previous profile's witness or results.
    resetSessionState();
    QString error;
    if (!m_client.configure(cliPath, walletDir, &error)) {
        m_client.clearConfiguration();
        setConfigured(false);
        setBackendState(QStringLiteral("Not configured"));
        setLastError(error);
        setStatusText(error);
        QJsonObject response;
        response.insert(QStringLiteral("configured"), false);
        response.insert(QStringLiteral("error"), error);
        setLastResultJson(prettyJson(response));
        Q_EMIT operationFailed(QStringLiteral("configure"), error);
        return compactJson(response);
    }

    setConfigured(true);
    const QFileInfo readOnlyMarker(QDir(m_client.walletDir()).filePath(QStringLiteral(".commons-readonly")));
    setReadOnly(readOnlyMarker.exists() || readOnlyMarker.isSymLink());
    setConnectionCliPath(m_client.cliPath());
    setConnectionWalletDir(m_client.walletDir());
    const QString capture = QDir(m_client.walletDir()).filePath(QStringLiteral("evidence"));
    if (!QFileInfo(capture).isSymLink() && QDir().mkpath(capture)
        && QFileInfo(capture).canonicalFilePath().startsWith(m_client.walletDir() + QLatin1Char('/')))
        setCaptureDirectory(QFileInfo(capture).canonicalFilePath());
    else setCaptureDirectory(QString());
    setBackendState(QStringLiteral("Ready"));
    setLastError(QString());
    setStatusText(readOnly() ? QStringLiteral("Viewing current testnet state. This workspace cannot send transactions.") : QStringLiteral("Configured for testnet. No transaction has been sent."));

    QJsonObject response;
    response.insert(QStringLiteral("configured"), true);
    response.insert(QStringLiteral("network"), QStringLiteral("testnet"));
    response.insert(QStringLiteral("schema_version"), 1);
    setLastResultJson(prettyJson(response));
    Q_EMIT operationFinished(QStringLiteral("configure"), compactJson(response));
    return compactJson(response);
}

QString CommonsPrimitivesBackend::selectAllowlistWitness(QString witnessPath)
{
    return selectWitnessFile(witnessPath, false);
}

QString CommonsPrimitivesBackend::selectThresholdWitness(QString witnessPath)
{
    return selectWitnessFile(witnessPath, true);
}

QString CommonsPrimitivesBackend::createDistribution(QString stateAccount, QString rootHex, int memberCount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("root"), rootHex.trimmed());
    arguments.insert(QStringLiteral("member_count"), memberCount);
    return queue(CommonsLogos::PrimitiveOperation::AllowlistCreateDistribution, arguments);
}

QString CommonsPrimitivesBackend::claimAllowlist(QString stateAccount)
{
    if (m_allowlistWitnessPath.isEmpty()) {
        return reject(CommonsLogos::PrimitiveOperation::AllowlistClaim,
                      QStringLiteral("Select an allowlist witness file first."));
    }

    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("witness_file"), m_allowlistWitnessPath);
    return queue(CommonsLogos::PrimitiveOperation::AllowlistClaim, arguments);
}

QString CommonsPrimitivesBackend::inspectDistribution(QString stateAccount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    return queue(CommonsLogos::PrimitiveOperation::AllowlistInspect, arguments);
}

QString CommonsPrimitivesBackend::createGroup(QString stateAccount, QString rootHex,
                                            int memberCount,
                                            int threshold,
                                            QString initialValue)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("root"), rootHex.trimmed());
    arguments.insert(QStringLiteral("member_count"), memberCount);
    arguments.insert(QStringLiteral("threshold"), threshold);
    arguments.insert(QStringLiteral("initial_value"), initialValue.trimmed());
    return queue(CommonsLogos::PrimitiveOperation::ThresholdCreateGroup, arguments);
}

QString CommonsPrimitivesBackend::proposeParameter(QString stateAccount, QString nextValue)
{
    if (m_thresholdWitnessPath.isEmpty()) {
        return reject(CommonsLogos::PrimitiveOperation::ThresholdPropose,
                      QStringLiteral("Select a threshold witness file first."));
    }

    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("witness_file"), m_thresholdWitnessPath);
    arguments.insert(QStringLiteral("next_value"), nextValue.trimmed());
    return queue(CommonsLogos::PrimitiveOperation::ThresholdPropose, arguments);
}

QString CommonsPrimitivesBackend::approveParameter(QString stateAccount)
{
    if (m_thresholdWitnessPath.isEmpty()) {
        return reject(CommonsLogos::PrimitiveOperation::ThresholdApprove,
                      QStringLiteral("Select a threshold witness file first."));
    }

    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("witness_file"), m_thresholdWitnessPath);
    return queue(CommonsLogos::PrimitiveOperation::ThresholdApprove, arguments);
}

QString CommonsPrimitivesBackend::executeParameter(QString stateAccount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    return queue(CommonsLogos::PrimitiveOperation::ThresholdExecute, arguments);
}

QString CommonsPrimitivesBackend::inspectGroup(QString stateAccount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    return queue(CommonsLogos::PrimitiveOperation::ThresholdInspect, arguments);
}

QString CommonsPrimitivesBackend::schemaJson()
{
    return CommonsLogos::contractSchemaJson();
}

QString CommonsPrimitivesBackend::selectWitnessFile(const QString& rawPath, const bool thresholdWitness)
{
    if (m_client.isBusy()) {
        return compactJson(QJsonObject{{QStringLiteral("selected"), false},
            {QStringLiteral("error"), QStringLiteral("Wait for the current operation to finish.")}});
    }
    // Failed replacement must not leave an earlier witness armed.
    if (thresholdWitness) {
        m_thresholdWitnessPath.clear();
        setThresholdWitnessLabel(QStringLiteral("No witness selected"));
    } else {
        m_allowlistWitnessPath.clear();
        setAllowlistWitnessLabel(QStringLiteral("No witness selected"));
    }
    const QString path = CommonsLogos::localPathFromUi(rawPath);
    const QFileInfo info(path);
    if (m_client.isBusy() || !m_client.isConfigured() || !info.isAbsolute() || !info.exists() || !info.isFile() || info.isSymLink() || !info.canonicalFilePath().startsWith(m_client.walletDir() + QLatin1Char('/'))) {
        const QString message = QStringLiteral("Select a regular witness file inside the configured wallet directory while idle.");
        setLastError(message);
        setStatusText(message);
        QJsonObject response;
        response.insert(QStringLiteral("selected"), false);
        response.insert(QStringLiteral("error"), message);
        setLastResultJson(prettyJson(response));
        Q_EMIT operationFailed(QStringLiteral("select_witness"), message);
        return compactJson(response);
    }

    const QString canonical = info.canonicalFilePath();
    const QString label = info.fileName();
    if (thresholdWitness) {
        m_thresholdWitnessPath = canonical;
        setThresholdWitnessLabel(label);
    } else {
        m_allowlistWitnessPath = canonical;
        setAllowlistWitnessLabel(label);
    }
    setLastError(QString());
    setStatusText(QStringLiteral("Witness file selected."));

    QJsonObject response;
    response.insert(QStringLiteral("selected"), true);
    response.insert(QStringLiteral("label"), label);
    setLastResultJson(prettyJson(response));
    Q_EMIT operationFinished(QStringLiteral("select_witness"), compactJson(response));
    return compactJson(response);
}

QString CommonsPrimitivesBackend::queue(CommonsLogos::PrimitiveOperation operation,
                                      const QJsonObject& arguments)
{
    const bool reading = operation == CommonsLogos::PrimitiveOperation::AllowlistInspect
        || operation == CommonsLogos::PrimitiveOperation::ThresholdInspect;
    if (readOnly() && !reading)
        return reject(operation, QStringLiteral("This workspace is for viewing only. Choose a member wallet to submit transactions."));
    if (!m_client.isBusy())
        clearStateForOperation(CommonsLogos::operationId(operation));
    const CommonsLogos::StartResult result = m_client.start(operation, arguments);
    if (!result.accepted) {
        setLastOperation(result.operation);
        setLastError(result.errorMessage);
        setStatusText(result.errorMessage);
        setLastResultJson(prettyJson(QJsonObject{{QStringLiteral("success"), false},
            {QStringLiteral("operation"), result.operation}, {QStringLiteral("error"), result.errorMessage}}));
        Q_EMIT operationFailed(result.operation, result.errorMessage);
        return result.toJson();
    }

    setBusy(true);
    const QString displayName = CommonsLogos::operationDisplayName(operation);
    setActiveOperation(displayName);
    setLastOperation(result.operation);
    setLastError(QString());
    const bool proving = operation == CommonsLogos::PrimitiveOperation::AllowlistClaim
        || operation == CommonsLogos::PrimitiveOperation::ThresholdPropose
        || operation == CommonsLogos::PrimitiveOperation::ThresholdApprove;
    setStatusText(reading ? QStringLiteral("Reading testnet state...")
        : proving ? QStringLiteral("Generating a private proof locally, then submitting to testnet...")
                  : QStringLiteral("Submitting to testnet and waiting for confirmation..."));
    return result.toJson();
}

QString CommonsPrimitivesBackend::reject(CommonsLogos::PrimitiveOperation operation,
                                       const QString& message)
{
    const QString operationName = CommonsLogos::operationId(operation);
    if (!m_client.isBusy()) clearStateForOperation(operationName);
    setLastOperation(operationName);
    setLastError(message);
    setStatusText(message);
    setLastResultJson(prettyJson(QJsonObject{{QStringLiteral("success"), false},
        {QStringLiteral("operation"), operationName}, {QStringLiteral("error"), message}}));
    Q_EMIT operationFailed(operationName, message);

    QJsonObject response;
    response.insert(QStringLiteral("accepted"), false);
    response.insert(QStringLiteral("operation"), operationName);
    response.insert(QStringLiteral("error"), message);
    return compactJson(response);
}

void CommonsPrimitivesBackend::completeOperation(const QString& operation,
                                               const QJsonObject& result)
{
    setBusy(false);
    setActiveOperation(QString());
    setLastOperation(operation);
    setLastError(QString());
    setLastResultJson(prettyJson(result));
    const bool reading = operation.endsWith(QStringLiteral(".inspect_state"));
    const auto block = result.value(QStringLiteral("block_id"));
    if (reading)
        setStatusText(block.isDouble()
            ? QStringLiteral("Testnet state read at block %1.").arg(block.toInteger())
            : QStringLiteral("Testnet state loaded."));
    else
        setStatusText(block.isDouble()
            ? QStringLiteral("Confirmed in testnet block %1.").arg(block.toInteger())
            : QStringLiteral("Operation completed."));

    if (operation.startsWith(QStringLiteral("allowlist.")))
        updateDistributionState(result);
    if (operation.startsWith(QStringLiteral("threshold.")))
        updateGroupState(result);

    Q_EMIT operationFinished(operation, compactJson(result));
}

void CommonsPrimitivesBackend::failOperation(const QString& operation,
                                           const QString& errorMessage)
{
    setBusy(false);
    setActiveOperation(QString());
    setLastOperation(operation);
    clearStateForOperation(operation);
    setLastError(errorMessage);
    setStatusText(errorMessage);
    setLastResultJson(prettyJson(QJsonObject{{QStringLiteral("success"), false},
        {QStringLiteral("operation"), operation}, {QStringLiteral("error"), errorMessage}}));
    Q_EMIT operationFailed(operation, errorMessage);
}

void CommonsPrimitivesBackend::updateDistributionState(const QJsonObject& result)
{
    const QJsonObject state = stateObject(result);
    const QString stateAccount = stringValue(result, QStringLiteral("state_account"));
    if (!stateAccount.isEmpty())
        setDistributionStateAccount(stateAccount);

    const int memberCount = intValue(state, QStringLiteral("member_count"));
    int claimsCount = intValue(state, QStringLiteral("claims_count"));
    if (claimsCount < 0)
        claimsCount = intValue(state, QStringLiteral("claims"));

    QStringList parts;
    if (claimsCount >= 0 && memberCount >= 0)
        parts.append(QStringLiteral("Claims %1/%2").arg(claimsCount).arg(memberCount));
    else if (memberCount >= 0)
        parts.append(QStringLiteral("Members %1").arg(memberCount));
    insertIfPresent(parts, QStringLiteral("state"), stateAccount);

    if (parts.isEmpty())
        setDistributionSummary(QStringLiteral("Distribution state returned by CLI."));
    else
        setDistributionSummary(parts.join(QStringLiteral("; ")));
}

void CommonsPrimitivesBackend::updateGroupState(const QJsonObject& result)
{
    const QJsonObject state = stateObject(result);
    const QString stateAccount = stringValue(result, QStringLiteral("state_account"));
    if (!stateAccount.isEmpty())
        setGroupStateAccount(stateAccount);

    QStringList parts;
    insertIfPresent(parts, QStringLiteral("value"), stringValue(state, QStringLiteral("value")));

    const int memberCount = intValue(state, QStringLiteral("member_count"));
    const int threshold = intValue(state, QStringLiteral("threshold"));
    if (threshold >= 0 && memberCount >= 0)
        parts.append(QStringLiteral("threshold %1/%2").arg(threshold).arg(memberCount));
    else if (threshold >= 0)
        parts.append(QStringLiteral("threshold %1").arg(threshold));

    const QJsonObject proposal = state.value(QStringLiteral("proposal")).toObject();
    if (!proposal.isEmpty()) {
        const QString sequence = stringValue(proposal, QStringLiteral("sequence"));
        const QString nextValue = stringValue(proposal, QStringLiteral("next_value"));
        int approvals = intValue(proposal, QStringLiteral("approvals_count"));
        if (approvals < 0)
            approvals = intValue(proposal, QStringLiteral("approvals"));
        const bool executed = proposal.value(QStringLiteral("executed")).toBool(false);

        QString proposalText = QStringLiteral("proposal");
        if (!sequence.isEmpty())
            proposalText += QStringLiteral(" #") + sequence;
        if (!nextValue.isEmpty())
            proposalText += QStringLiteral(" -> ") + nextValue;
        if (approvals >= 0 && threshold >= 0)
            proposalText += QStringLiteral(", approvals %1/%2").arg(approvals).arg(threshold);
        else if (approvals >= 0)
            proposalText += QStringLiteral(", approvals %1").arg(approvals);
        proposalText += executed ? QStringLiteral(", executed") : QStringLiteral(", pending");
        parts.append(proposalText);
    } else if (state.contains(QStringLiteral("proposal"))) {
        parts.append(QStringLiteral("no proposal"));
    }

    insertIfPresent(parts, QStringLiteral("state"), stateAccount);

    if (parts.isEmpty())
        setGroupSummary(QStringLiteral("Group state returned by CLI."));
    else
        setGroupSummary(parts.join(QStringLiteral("; ")));
}
