#pragma once

#include "CommonsLogosSchemas.h"

#include <QObject>
#include <QJsonObject>
#include <QProcess>
#include <QTimer>
#include <QString>

namespace CommonsLogos {

struct StartResult {
    bool accepted = false;
    QString requestId;
    QString operation;
    QString errorMessage;

    QString toJson() const;
};

class LogosCliClient : public QObject {
    Q_OBJECT

public:
    explicit LogosCliClient(QObject* parent = nullptr, int timeoutMs = 4 * 60 * 60 * 1000);
    ~LogosCliClient() override;

    bool configure(const QString& cliPath,
                   const QString& walletDir,
                   QString* errorMessage = nullptr);
    void clearConfiguration();

    bool isConfigured() const;
    bool isBusy() const;
    QString cliPath() const;
    QString walletDir() const;

    StartResult start(PrimitiveOperation operation, const QJsonObject& arguments);

    static bool validateOperationArguments(PrimitiveOperation operation,
                                           const QJsonObject& arguments,
                                           QString* errorMessage = nullptr);
    static QJsonObject buildRequestObject(PrimitiveOperation operation,
                                          const QString& walletDir,
                                          const QJsonObject& arguments);

signals:
    void started(QString requestId, QString operation);
    void completed(QString requestId, QString operation, QJsonObject result);
    void failed(QString requestId, QString operation, QString errorMessage);

private:
    void finishProcess(int exitCode, QProcess::ExitStatus exitStatus);
    void failCurrent(const QString& message);
    void collectOutput();
    void stopOwnedProcess(QProcess* process);
    QStringList sensitivePathsFor(const QJsonObject& arguments) const;

    QString m_cliPath;
    QString m_walletDir;
    QProcess* m_process = nullptr;
    QString m_currentRequestId;
    PrimitiveOperation m_currentOperation = PrimitiveOperation::AllowlistInspect;
    QStringList m_currentSensitivePaths;
    quint64 m_nextRequest = 1;
    QTimer m_timeout;
    int m_timeoutMs;
    QByteArray m_stdout;
    qsizetype m_stderrBytes = 0;
    static constexpr qsizetype MAX_OUTPUT_BYTES = 1024 * 1024;
};

} // namespace CommonsLogos
