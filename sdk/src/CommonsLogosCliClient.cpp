#include "CommonsLogosCliClient.h"

#include <QFileInfo>
#include <QDir>
#include <QSet>
#ifdef Q_OS_UNIX
#include <csignal>
#endif
#include <QJsonDocument>
#include <QJsonParseError>
#include <QJsonValue>
#include <QProcess>
#include <QProcessEnvironment>

namespace CommonsLogos {
namespace {

bool fail(QString* errorMessage, const QString& value)
{
    if (errorMessage)
        *errorMessage = value;
    return false;
}

bool intInRange(const QJsonObject& object,
                const QString& key,
                const int min,
                const int max,
                QString* errorMessage)
{
    const QJsonValue value = object.value(key);
    if (!value.isDouble())
        return fail(errorMessage, QStringLiteral("%1 is required.").arg(key));
    const double asDouble = value.toDouble();
    const int asInt = value.toInt();
    if (asDouble != static_cast<double>(asInt) || asInt < min || asInt > max) {
        return fail(errorMessage,
                    QStringLiteral("%1 must be between %2 and %3.")
                        .arg(key)
                        .arg(min)
                        .arg(max));
    }
    return true;
}

bool stringIsHex32(const QJsonObject& object,
                   const QString& key,
                   QString* errorMessage)
{
    const QJsonValue value = object.value(key);
    if (!value.isString() || !validateHex32(value.toString())) {
        return fail(errorMessage,
                    QStringLiteral("%1 must be a 32-byte hex value.").arg(key));
    }
    return true;
}

bool stringIsI64(const QJsonObject& object,
                 const QString& key,
                 QString* errorMessage)
{
    const QJsonValue value = object.value(key);
    if (!value.isString() || !validateI64Decimal(value.toString())) {
        return fail(errorMessage,
                    QStringLiteral("%1 must be a signed 64-bit decimal integer.").arg(key));
    }
    return true;
}

bool stringIsExistingFile(const QJsonObject& object,
                          const QString& key,
                          QString* errorMessage)
{
    const QJsonValue value = object.value(key);
    if (!value.isString())
        return fail(errorMessage, QStringLiteral("%1 is required.").arg(key));
    const QFileInfo info(localPathFromUi(value.toString()));
    if (!info.isAbsolute() || !info.exists() || !info.isFile()) {
        return fail(errorMessage,
                    QStringLiteral("%1 must be an existing absolute file path.").arg(key));
    }
    return true;
}

QString extractCliError(const QJsonObject& object)
{
    const QJsonValue error = object.value(QStringLiteral("error"));
    if (error.isString())
        return error.toString();
    if (error.isObject()) {
        const QJsonObject errorObject = error.toObject();
        const QJsonValue message = errorObject.value(QStringLiteral("message"));
        if (message.isString())
            return message.toString();
        return QString::fromUtf8(QJsonDocument(errorObject).toJson(QJsonDocument::Compact));
    }
    return {};
}

} // namespace

QString StartResult::toJson() const
{
    QJsonObject object;
    object.insert(QStringLiteral("accepted"), accepted);
    object.insert(QStringLiteral("request_id"), requestId);
    object.insert(QStringLiteral("operation"), operation);
    if (!errorMessage.isEmpty())
        object.insert(QStringLiteral("error"), errorMessage);
    return QString::fromUtf8(QJsonDocument(object).toJson(QJsonDocument::Compact));
}

LogosCliClient::LogosCliClient(QObject* parent, int timeoutMs)
    : QObject(parent), m_timeoutMs(qBound(1, timeoutMs, 4 * 60 * 60 * 1000))
{
    m_timeout.setSingleShot(true);
    connect(&m_timeout, &QTimer::timeout, this, [this]() {
        failCurrent(QStringLiteral("Operation timed out. Inspect chain state before retrying; a submitted transaction may still confirm."));
    });
}

LogosCliClient::~LogosCliClient()
{
    if (m_process) {
        stopOwnedProcess(m_process);
        m_process->deleteLater();
        m_process = nullptr;
    }
}

bool LogosCliClient::configure(const QString& rawCliPath,
                               const QString& rawWalletDir,
                               QString* errorMessage)
{
    if (isBusy()) return fail(errorMessage, QStringLiteral("Cannot change configuration while an operation is running."));
    QString err;
    if (!validateCliPath(rawCliPath, &err))
        return fail(errorMessage, err);

    QString canonicalWalletDir;
    if (!validateWalletDir(rawWalletDir, &canonicalWalletDir, &err))
        return fail(errorMessage, err);

    m_cliPath = localPathFromUi(rawCliPath);
    m_walletDir = canonicalWalletDir;
    if (errorMessage)
        errorMessage->clear();
    return true;
}

void LogosCliClient::clearConfiguration()
{
    if (isBusy()) return;
    m_cliPath.clear();
    m_walletDir.clear();
}

bool LogosCliClient::isConfigured() const
{
    return !m_cliPath.isEmpty() && !m_walletDir.isEmpty();
}

bool LogosCliClient::isBusy() const
{
    return m_process != nullptr;
}

QString LogosCliClient::cliPath() const
{
    return m_cliPath;
}

QString LogosCliClient::walletDir() const
{
    return m_walletDir;
}

StartResult LogosCliClient::start(const PrimitiveOperation operation,
                                  const QJsonObject& arguments)
{
    StartResult result;
    result.operation = operationId(operation);

    if (!isConfigured()) {
        result.errorMessage = QStringLiteral("Configure commons-logos-cli and a testnet wallet directory first.");
        return result;
    }
    if (m_process) {
        result.errorMessage = QStringLiteral("Another private transaction operation is already running.");
        return result;
    }

    QString err;
    if (!validateOperationArguments(operation, arguments, &err)) {
        result.errorMessage = err;
        return result;
    }

    if (arguments.contains(QStringLiteral("witness_file"))) {
        const QFileInfo witness(localPathFromUi(arguments.value(QStringLiteral("witness_file")).toString()));
        const QString canonical = witness.canonicalFilePath();
        if (witness.isSymLink() || !canonical.startsWith(m_walletDir + QDir::separator())) {
            result.errorMessage = QStringLiteral("Witness must be a regular file inside the configured testnet wallet directory.");
            return result;
        }
    }

    const QStringList cliArgs = cliArgumentsForOperation(operation);
    if (cliArgs.isEmpty() || !isAllowedOperationId(operationId(operation))) {
        result.errorMessage = QStringLiteral("Operation is not in the CLI allowlist.");
        return result;
    }

    result.accepted = true;
    result.requestId = QStringLiteral("commons-%1").arg(m_nextRequest++);
    m_currentRequestId = result.requestId;
    m_currentOperation = operation;
    m_currentSensitivePaths = sensitivePathsFor(arguments);

    const QJsonObject request = buildRequestObject(operation, m_walletDir, arguments);
    const QByteArray stdinPayload =
        QJsonDocument(request).toJson(QJsonDocument::Compact) + QByteArrayLiteral("\n");

    auto* process = new QProcess(this);
    m_process = process;
    process->setProgram(m_cliPath);
    process->setArguments(cliArgs);
    process->setWorkingDirectory(m_walletDir);
    process->setProcessChannelMode(QProcess::SeparateChannels);

    QProcessEnvironment env;
    const auto inherited = QProcessEnvironment::systemEnvironment();
    const QStringList allowed = {QStringLiteral("HOME"), QStringLiteral("PATH"), QStringLiteral("TMPDIR"),
        QStringLiteral("RUSTUP_HOME"), QStringLiteral("CARGO_HOME"), QStringLiteral("RISC0_HOME"),
        QStringLiteral("DYLD_LIBRARY_PATH"), QStringLiteral("SSL_CERT_FILE"), QStringLiteral("LANG"),
        QStringLiteral("LC_ALL"), QStringLiteral("RISC0_SERVER_PATH"), QStringLiteral("RAYON_NUM_THREADS")};
    for (const auto& key : allowed) if (inherited.contains(key)) env.insert(key, inherited.value(key));
#ifdef COMMONS_SDK_TESTING
    if (inherited.contains(QStringLiteral("COMMONS_FAKE_CLI_MODE")))
        env.insert(QStringLiteral("COMMONS_FAKE_CLI_MODE"), inherited.value(QStringLiteral("COMMONS_FAKE_CLI_MODE")));
#endif
    env.insert(QStringLiteral("RISC0_DEV_MODE"), QStringLiteral("0"));
    env.insert(QStringLiteral("RISC0_PROVER"), QStringLiteral("ipc"));
    env.insert(QStringLiteral("RISC0_EXECUTOR"), QStringLiteral("ipc"));
    env.insert(QStringLiteral("COMMONS_LOGOS_NETWORK"), QStringLiteral("testnet"));
    process->setProcessEnvironment(env);
#ifdef Q_OS_UNIX
    process->setUnixProcessParameters(QProcess::UnixProcessFlag::CreateNewSession);
#endif
    m_stdout.clear();
    m_stderrBytes = 0;
    connect(process, &QProcess::readyReadStandardOutput, this, &LogosCliClient::collectOutput);
    connect(process, &QProcess::readyReadStandardError, this, &LogosCliClient::collectOutput);

    connect(process, &QProcess::started, this, [this, process, stdinPayload]() {
        if (process != m_process)
            return;
        process->write(stdinPayload);
        process->closeWriteChannel();
        emit started(m_currentRequestId, operationId(m_currentOperation));
    });

    connect(process,
            qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
            this,
            [this, process](int exitCode, QProcess::ExitStatus exitStatus) {
                if (process != m_process)
                    return;
                finishProcess(exitCode, exitStatus);
            });

    connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
        if (process != m_process)
            return;
        if (error == QProcess::FailedToStart) {
            failCurrent(QStringLiteral("Could not start configured commons-logos-cli."));
        }
    });

    m_timeout.start(m_timeoutMs);
    process->start(QIODevice::ReadWrite);
    return result;
}

bool LogosCliClient::validateOperationArguments(const PrimitiveOperation operation,
                                                const QJsonObject& arguments,
                                                QString* errorMessage)
{
    QStringList allowedKeys;
    switch (operation) {
    case PrimitiveOperation::AllowlistCreateDistribution: allowedKeys = {"state_account", "root", "member_count"}; break;
    case PrimitiveOperation::ThresholdCreateGroup: allowedKeys = {"state_account", "root", "member_count", "threshold", "initial_value"}; break;
    case PrimitiveOperation::AllowlistClaim:
    case PrimitiveOperation::ThresholdApprove: allowedKeys = {"state_account", "witness_file"}; break;
    case PrimitiveOperation::ThresholdPropose: allowedKeys = {"state_account", "witness_file", "next_value"}; break;
    case PrimitiveOperation::AllowlistInspect:
    case PrimitiveOperation::ThresholdInspect:
    case PrimitiveOperation::ThresholdExecute: allowedKeys = {"state_account"}; break;
    default: return fail(errorMessage, QStringLiteral("Unknown operation."));
    }
    for (auto it=arguments.begin(); it!=arguments.end(); ++it)
        if (!allowedKeys.contains(it.key())) return fail(errorMessage, QStringLiteral("Unknown argument: %1").arg(it.key()));
    if (!stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage)) return false;
    switch (operation) {
    case PrimitiveOperation::AllowlistCreateDistribution:
        return stringIsHex32(arguments, QStringLiteral("root"), errorMessage)
            && intInRange(arguments, QStringLiteral("member_count"), 1, 256, errorMessage);
    case PrimitiveOperation::AllowlistClaim:
        return stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage)
            && stringIsExistingFile(arguments, QStringLiteral("witness_file"), errorMessage);
    case PrimitiveOperation::AllowlistInspect:
        return stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage);
    case PrimitiveOperation::ThresholdCreateGroup:
        if (!stringIsHex32(arguments, QStringLiteral("root"), errorMessage)
            || !intInRange(arguments, QStringLiteral("member_count"), 1, 256, errorMessage)
            || !intInRange(arguments, QStringLiteral("threshold"), 1, 256, errorMessage)) {
            return false;
        }
        if (arguments.value(QStringLiteral("threshold")).toInt()
            > arguments.value(QStringLiteral("member_count")).toInt()) {
            return fail(errorMessage,
                        QStringLiteral("threshold must be less than or equal to member_count."));
        }
        return stringIsI64(arguments, QStringLiteral("initial_value"), errorMessage);
    case PrimitiveOperation::ThresholdPropose:
        return stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage)
            && stringIsExistingFile(arguments, QStringLiteral("witness_file"), errorMessage)
            && stringIsI64(arguments, QStringLiteral("next_value"), errorMessage);
    case PrimitiveOperation::ThresholdApprove:
        return stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage)
            && stringIsExistingFile(arguments, QStringLiteral("witness_file"), errorMessage);
    case PrimitiveOperation::ThresholdExecute:
        return stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage);
    case PrimitiveOperation::ThresholdInspect:
        return stringIsHex32(arguments, QStringLiteral("state_account"), errorMessage);
    }

    return fail(errorMessage, QStringLiteral("Unsupported operation."));
}

QJsonObject LogosCliClient::buildRequestObject(const PrimitiveOperation operation,
                                               const QString& walletDir,
                                               const QJsonObject& arguments)
{
    QJsonObject request;
    request.insert(QStringLiteral("schema_version"), 1);
    request.insert(QStringLiteral("network"), QStringLiteral("testnet"));
    request.insert(QStringLiteral("wallet_dir"), walletDir);
    request.insert(QStringLiteral("operation"), operationId(operation));
    request.insert(QStringLiteral("arguments"), arguments);
    return request;
}

void LogosCliClient::finishProcess(const int exitCode, const QProcess::ExitStatus exitStatus)
{
    QProcess* process = m_process;
    if (!process)
        return;

    const QString requestId = m_currentRequestId;
    const QString operation = operationId(m_currentOperation);
    const QStringList sensitivePaths = m_currentSensitivePaths;
    collectOutput();
    if (process != m_process) return;
    m_timeout.stop();
    const QByteArray stdoutBytes = m_stdout;
    m_stdout.clear();
    const QByteArray stderrBytes;

    process->deleteLater();
    m_process = nullptr;
    m_currentRequestId.clear();
    m_currentSensitivePaths.clear();

    QJsonParseError parseError;
    const QJsonDocument doc = QJsonDocument::fromJson(stdoutBytes, &parseError);
    if (parseError.error != QJsonParseError::NoError || !doc.isObject()) {
        QString message = QStringLiteral("CLI returned invalid JSON.");
        if (exitStatus != QProcess::NormalExit)
            message = QStringLiteral("CLI crashed before returning JSON.");
        else if (exitCode != 0)
            message = QStringLiteral("CLI failed before returning JSON.");
        Q_UNUSED(stderrBytes);
        emit failed(requestId, operation, message);
        return;
    }

    const QJsonObject rawObject = doc.object();
    const QString cliError = sanitizeForUi(extractCliError(rawObject), sensitivePaths);
    if (exitStatus != QProcess::NormalExit || exitCode != 0) {
        QString message = QStringLiteral("CLI failed.");
        if (!cliError.isEmpty())
            message = cliError;
        emit failed(requestId, operation, message);
        return;
    }

    const QJsonValue successValue = rawObject.value(QStringLiteral("success"));
    if (!successValue.isBool() || !successValue.toBool()) {
        QString message = cliError;
        if (message.isEmpty())
            message = QStringLiteral("CLI reported failure.");
        emit failed(requestId, operation, message);
        return;
    }

    emit completed(requestId, operation, redactSensitiveJsonObject(rawObject, sensitivePaths));
}

void LogosCliClient::failCurrent(const QString& message)
{
    QProcess* process = m_process;
    if (!process) return;
    const QString requestId = m_currentRequestId;
    const QString operation = operationId(m_currentOperation);
    const QStringList paths = m_currentSensitivePaths;
    m_timeout.stop();
    m_process = nullptr;
    m_stdout.clear();
    m_currentRequestId.clear();
    m_currentSensitivePaths.clear();
    // Clear our state before stopping children: error/finished signals can be
    // delivered during teardown. Delete only after QProcess reports termination.
    process->disconnect(this);
    process->setParent(nullptr);
    connect(process, QOverload<int,QProcess::ExitStatus>::of(&QProcess::finished), process, &QObject::deleteLater);
    stopOwnedProcess(process);
    if (process->state() == QProcess::NotRunning) process->deleteLater();
    emit failed(requestId, operation, sanitizeForUi(message, paths));
}

void LogosCliClient::collectOutput()
{
    if (!m_process) return;
    const QByteArray out = m_process->readAllStandardOutput();
    const QByteArray err = m_process->readAllStandardError();
    m_stderrBytes += err.size();
    if (out.size() > MAX_OUTPUT_BYTES - m_stdout.size() || m_stderrBytes > MAX_OUTPUT_BYTES) {
        failCurrent(QStringLiteral("CLI output limit exceeded. No raw output is displayed."));
        return;
    }
    m_stdout.append(out);
}

void LogosCliClient::stopOwnedProcess(QProcess* process)
{
    if (!process || process->state() == QProcess::NotRunning) return;
#ifdef Q_OS_UNIX
    // Every spawned CLI has its own session. Stop its proof child processes too.
    const qint64 pid = process->processId();
    if (pid > 0) ::kill(-static_cast<pid_t>(pid), SIGKILL);
#endif
    process->kill();
}

QStringList LogosCliClient::sensitivePathsFor(const QJsonObject& arguments) const
{
    QStringList paths;
    if (!m_walletDir.isEmpty())
        paths.append(m_walletDir);
    if (!m_cliPath.isEmpty())
        paths.append(m_cliPath);

    const QJsonValue witnessFile = arguments.value(QStringLiteral("witness_file"));
    if (witnessFile.isString())
        paths.append(localPathFromUi(witnessFile.toString()));

    return paths;
}

} // namespace CommonsLogos
