#pragma once

#include <QJsonObject>
#include <QString>
#include <QStringList>

namespace CommonsLogos {

enum class PrimitiveOperation {
    AllowlistCreateDistribution,
    AllowlistClaim,
    AllowlistInspect,
    ThresholdCreateGroup,
    ThresholdPropose,
    ThresholdApprove,
    ThresholdExecute,
    ThresholdInspect,
};

QString operationId(PrimitiveOperation operation);
QString operationDisplayName(PrimitiveOperation operation);
QStringList cliArgumentsForOperation(PrimitiveOperation operation);
bool isAllowedOperationId(const QString& operationId);

QString contractSchemaJson();
QJsonObject contractSchemaObject();

bool validateHex32(const QString& value);
bool validateI64Decimal(const QString& value, qlonglong* parsed = nullptr);
bool validateCliPath(const QString& cliPath, QString* errorMessage = nullptr);
bool validateWalletDir(const QString& walletDir,
                       QString* canonicalWalletDir = nullptr,
                       QString* errorMessage = nullptr);

QString localPathFromUi(const QString& value);
QString fileLabel(const QString& path);
QString sanitizeForUi(const QString& message, const QStringList& sensitivePaths = {});
QJsonObject redactSensitiveJsonObject(const QJsonObject& object,
                                      const QStringList& sensitivePaths = {});

} // namespace CommonsLogos
