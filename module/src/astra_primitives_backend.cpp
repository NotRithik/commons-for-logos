#include "astra_primitives_backend.h"

#include <QFileInfo>
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

AstraPrimitivesBackend::AstraPrimitivesBackend()
{
    connect(&m_client,
            &AstraLogos::LogosCliClient::completed,
            this,
            [this](const QString&, const QString& operation, const QJsonObject& result) {
                completeOperation(operation, result);
            });
    connect(&m_client,
            &AstraLogos::LogosCliClient::failed,
            this,
            [this](const QString&, const QString& operation, const QString& errorMessage) {
                failOperation(operation, errorMessage);
            });
}

void AstraPrimitivesBackend::resetSessionState()
{
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

void AstraPrimitivesBackend::clearStateForOperation(const QString& operation)
{
    if (operation.startsWith(QStringLiteral("allowlist."))) {
        setDistributionStateAccount(QString());
        setDistributionSummary(QStringLiteral("Inspect a distribution to load its current state."));
    } else if (operation.startsWith(QStringLiteral("threshold."))) {
        setGroupStateAccount(QString());
        setGroupSummary(QStringLiteral("Inspect a group to load its current state."));
    }
}

QString AstraPrimitivesBackend::configure(QString cliPath, QString walletDir)
{
    if (m_client.isBusy())
        return reject(AstraLogos::PrimitiveOperation::AllowlistInspect,
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
    const QString capture = QDir(m_client.walletDir()).filePath(QStringLiteral("evidence"));
    if (!QFileInfo(capture).isSymLink() && QDir().mkpath(capture)
        && QFileInfo(capture).canonicalFilePath().startsWith(m_client.walletDir() + QLatin1Char('/')))
        setCaptureDirectory(QFileInfo(capture).canonicalFilePath());
    else setCaptureDirectory(QString());
    setBackendState(QStringLiteral("Ready"));
    setLastError(QString());
    setStatusText(QStringLiteral("Configured for testnet. No transaction has been sent."));

    QJsonObject response;
    response.insert(QStringLiteral("configured"), true);
    response.insert(QStringLiteral("network"), QStringLiteral("testnet"));
    response.insert(QStringLiteral("schema_version"), 1);
    setLastResultJson(prettyJson(response));
    Q_EMIT operationFinished(QStringLiteral("configure"), compactJson(response));
    return compactJson(response);
}

QString AstraPrimitivesBackend::selectAllowlistWitness(QString witnessPath)
{
    return selectWitnessFile(witnessPath, false);
}

QString AstraPrimitivesBackend::selectThresholdWitness(QString witnessPath)
{
    return selectWitnessFile(witnessPath, true);
}

QString AstraPrimitivesBackend::createDistribution(QString stateAccount, QString rootHex, int memberCount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("root"), rootHex.trimmed());
    arguments.insert(QStringLiteral("member_count"), memberCount);
    return queue(AstraLogos::PrimitiveOperation::AllowlistCreateDistribution, arguments);
}

QString AstraPrimitivesBackend::claimAllowlist(QString stateAccount)
{
    if (m_allowlistWitnessPath.isEmpty()) {
        return reject(AstraLogos::PrimitiveOperation::AllowlistClaim,
                      QStringLiteral("Select an allowlist witness file first."));
    }

    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("witness_file"), m_allowlistWitnessPath);
    return queue(AstraLogos::PrimitiveOperation::AllowlistClaim, arguments);
}

QString AstraPrimitivesBackend::inspectDistribution(QString stateAccount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    return queue(AstraLogos::PrimitiveOperation::AllowlistInspect, arguments);
}

QString AstraPrimitivesBackend::createGroup(QString stateAccount, QString rootHex,
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
    return queue(AstraLogos::PrimitiveOperation::ThresholdCreateGroup, arguments);
}

QString AstraPrimitivesBackend::proposeParameter(QString stateAccount, QString nextValue)
{
    if (m_thresholdWitnessPath.isEmpty()) {
        return reject(AstraLogos::PrimitiveOperation::ThresholdPropose,
                      QStringLiteral("Select a threshold witness file first."));
    }

    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("witness_file"), m_thresholdWitnessPath);
    arguments.insert(QStringLiteral("next_value"), nextValue.trimmed());
    return queue(AstraLogos::PrimitiveOperation::ThresholdPropose, arguments);
}

QString AstraPrimitivesBackend::approveParameter(QString stateAccount)
{
    if (m_thresholdWitnessPath.isEmpty()) {
        return reject(AstraLogos::PrimitiveOperation::ThresholdApprove,
                      QStringLiteral("Select a threshold witness file first."));
    }

    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    arguments.insert(QStringLiteral("witness_file"), m_thresholdWitnessPath);
    return queue(AstraLogos::PrimitiveOperation::ThresholdApprove, arguments);
}

QString AstraPrimitivesBackend::executeParameter(QString stateAccount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    return queue(AstraLogos::PrimitiveOperation::ThresholdExecute, arguments);
}

QString AstraPrimitivesBackend::inspectGroup(QString stateAccount)
{
    QJsonObject arguments;
    arguments.insert(QStringLiteral("state_account"), stateAccount.trimmed());
    return queue(AstraLogos::PrimitiveOperation::ThresholdInspect, arguments);
}

QString AstraPrimitivesBackend::schemaJson()
{
    return AstraLogos::contractSchemaJson();
}

QString AstraPrimitivesBackend::selectWitnessFile(const QString& rawPath, const bool thresholdWitness)
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
    const QString path = AstraLogos::localPathFromUi(rawPath);
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

QString AstraPrimitivesBackend::queue(AstraLogos::PrimitiveOperation operation,
                                      const QJsonObject& arguments)
{
    if (!m_client.isBusy())
        clearStateForOperation(AstraLogos::operationId(operation));
    const AstraLogos::StartResult result = m_client.start(operation, arguments);
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
    const QString displayName = AstraLogos::operationDisplayName(operation);
    setActiveOperation(displayName);
    setLastOperation(result.operation);
    setLastError(QString());
    const bool reading = operation == AstraLogos::PrimitiveOperation::AllowlistInspect
        || operation == AstraLogos::PrimitiveOperation::ThresholdInspect;
    const bool proving = operation == AstraLogos::PrimitiveOperation::AllowlistClaim
        || operation == AstraLogos::PrimitiveOperation::ThresholdPropose
        || operation == AstraLogos::PrimitiveOperation::ThresholdApprove;
    setStatusText(reading ? QStringLiteral("Reading testnet state...")
        : proving ? QStringLiteral("Generating a private proof locally, then submitting to testnet...")
                  : QStringLiteral("Submitting to testnet and waiting for confirmation..."));
    return result.toJson();
}

QString AstraPrimitivesBackend::reject(AstraLogos::PrimitiveOperation operation,
                                       const QString& message)
{
    const QString operationName = AstraLogos::operationId(operation);
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

void AstraPrimitivesBackend::completeOperation(const QString& operation,
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

void AstraPrimitivesBackend::failOperation(const QString& operation,
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

void AstraPrimitivesBackend::updateDistributionState(const QJsonObject& result)
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

void AstraPrimitivesBackend::updateGroupState(const QJsonObject& result)
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
