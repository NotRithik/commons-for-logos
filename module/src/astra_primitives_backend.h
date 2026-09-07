#pragma once

#include "AstraLogosCliClient.h"
#include "logos_ui_plugin_context.h"
#include "rep_astra_primitives_ui_source.h"

class AstraPrimitivesBackend : public AstraPrimitivesUiSimpleSource,
                               public LogosUiPluginContext
{
public:
    AstraPrimitivesBackend();

    QString configure(QString cliPath, QString walletDir) override;
    QString selectAllowlistWitness(QString witnessPath) override;
    QString selectThresholdWitness(QString witnessPath) override;
    QString createDistribution(QString stateAccount, QString rootHex, int memberCount) override;
    QString claimAllowlist(QString stateAccount) override;
    QString inspectDistribution(QString stateAccount) override;
    QString createGroup(QString stateAccount, QString rootHex, int memberCount, int threshold, QString initialValue) override;
    QString proposeParameter(QString stateAccount, QString nextValue) override;
    QString approveParameter(QString stateAccount) override;
    QString executeParameter(QString stateAccount) override;
    QString inspectGroup(QString stateAccount) override;
    QString schemaJson() override;

private:
    void resetSessionState();
    void clearStateForOperation(const QString& operation);
    QString selectWitnessFile(const QString& rawPath, bool thresholdWitness);
    QString queue(AstraLogos::PrimitiveOperation operation, const QJsonObject& arguments);
    QString reject(AstraLogos::PrimitiveOperation operation, const QString& message);
    void completeOperation(const QString& operation, const QJsonObject& result);
    void failOperation(const QString& operation, const QString& errorMessage);
    void updateDistributionState(const QJsonObject& result);
    void updateGroupState(const QJsonObject& result);

    AstraLogos::LogosCliClient m_client;
    QString m_allowlistWitnessPath;
    QString m_thresholdWitnessPath;
};
