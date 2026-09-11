#pragma once

#include "CommonsLogosCliClient.h"
#include <QJsonArray>
#include <QByteArray>
class QProcess;
#include "logos_ui_plugin_context.h"
#include "rep_commons_primitives_ui_source.h"

class CommonsPrimitivesBackend : public CommonsPrimitivesUiSimpleSource,
                               public LogosUiPluginContext
{
public:
    CommonsPrimitivesBackend();

    QString configure(QString cliPath, QString walletDir) override;
    QString reloadSavedProfiles() override;
    QString openSavedProfile(int index) override;
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
    QString governanceRefresh() override;
    QString governanceCreateIdentity(QString label) override;
    QString governanceSelectIdentity(QString identity) override;
    QString governancePreparePolicy(QString title, QString field, QString initialValue, int threshold, QString enrollmentsJson) override;
    QString governanceJoinPolicy(QString invitationJson) override;
    QString governanceOpenPolicy(int index) override;
    QString governanceJoinOwnPolicy(int index) override;
    QString membershipPrepareList(QString title, QString enrollmentsJson) override;
    QString submitReviewedParameter(QString action, QString stateAccount, QString nextValue, QString fingerprint) override;
    QString schemaJson() override;

private:
    QString governanceHome() const;
    QString governanceFailure(const QString& message);
    QString governanceRequest(const QString& action, const QString& identity, const QJsonObject& arguments);
    QProcess* m_governanceProcess = nullptr;
    QByteArray m_governanceOutput;
    QJsonObject m_governanceCatalogue;
    int m_governanceSelectionRevision = 0;
    void resetSessionState();
    void clearStateForOperation(const QString& operation);
    QString selectWitnessFile(const QString& rawPath, bool thresholdWitness);
    QString queue(CommonsLogos::PrimitiveOperation operation, const QJsonObject& arguments);
    QString reject(CommonsLogos::PrimitiveOperation operation, const QString& message);
    void completeOperation(const QString& operation, const QJsonObject& result);
    void failOperation(const QString& operation, const QString& errorMessage);
    void updateDistributionState(const QJsonObject& result);
    void updateGroupState(const QJsonObject& result);

    QJsonArray m_savedProfiles;
    CommonsLogos::LogosCliClient m_client;
    QString m_allowlistWitnessPath;
    QString m_thresholdWitnessPath;
};
