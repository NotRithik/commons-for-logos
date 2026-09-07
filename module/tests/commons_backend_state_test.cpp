// Isolated backend/UI-state tests. The compiled fixture is NOT a real wallet.
#include "commons_primitives_backend.h"
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QtTest/QtTest>

namespace {
QString address() { return QString(64, QLatin1Char('a')); }
QJsonObject object(const QString& text) { return QJsonDocument::fromJson(text.toUtf8()).object(); }
QString makeWallet(const QString& parent, const QString& name = QStringLiteral("testnet-wallet")) {
    const QString path = QDir(parent).filePath(name);
    QDir().mkpath(path);
    QFile marker(QDir(path).filePath(QStringLiteral(".commons-logos-testnet-wallet")));
    if (!marker.open(QIODevice::WriteOnly)) return {};
    marker.close();
    return path;
}
QString witness(const QString& wallet, const QString& name = QStringLiteral("member.bin")) {
    const QString path = QDir(wallet).filePath(name);
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly)) return {};
    f.write("EXPLICIT TEST FIXTURE - not a proof witness");
    f.close();
    return path;
}
QString cli() { return QString::fromLocal8Bit(COMMONS_TEST_FAKE_CLI); }
}

class CommonsBackendStateTest : public QObject {
    Q_OBJECT
private slots:
    void init() { qputenv("COMMONS_FAKE_CLI_MODE", "echo"); }
    void cleanup() { qunsetenv("COMMONS_FAKE_CLI_MODE"); }
    void initiallyNotConfigured() {
        CommonsPrimitivesBackend backend;
        QVERIFY(!backend.configured());
        QVERIFY(!backend.busy());
        QVERIFY(backend.captureDirectory().isEmpty());
        QVERIFY(backend.distributionStateAccount().isEmpty());
    }
    void switchingWalletDisarmsBothWitnessesAndClearsResults() {
        QTemporaryDir temp; QVERIFY(temp.isValid());
        const QString a = makeWallet(temp.path(), "testnet-a");
        const QString b = makeWallet(temp.path(), "testnet-b");
        CommonsPrimitivesBackend backend;
        QVERIFY(object(backend.configure(cli(), a))["configured"].toBool());
        QVERIFY(object(backend.selectAllowlistWitness(witness(a)))["selected"].toBool());
        QVERIFY(object(backend.selectThresholdWitness(witness(a, "threshold.bin")))["selected"].toBool());
        QSignalSpy finished(&backend, &CommonsPrimitivesUiSource::operationFinished);
        backend.inspectDistribution(address());
        QTRY_COMPARE_WITH_TIMEOUT(finished.count(), 1, 4000);
        QVERIFY(!backend.distributionStateAccount().isEmpty());
        QVERIFY(object(backend.configure(cli(), b))["configured"].toBool());
        QVERIFY(backend.distributionStateAccount().isEmpty());
        QVERIFY(backend.groupStateAccount().isEmpty());
        QCOMPARE(backend.allowlistWitnessLabel(), QStringLiteral("No witness selected"));
        QCOMPARE(backend.thresholdWitnessLabel(), QStringLiteral("No witness selected"));
        QVERIFY(!object(backend.claimAllowlist(address()))["accepted"].toBool());
        QVERIFY(!object(backend.approveParameter(address()))["accepted"].toBool());
        QVERIFY(!backend.busy());
    }
    void invalidConfigurationClearsPreviousCaptureDirectory() {
        QTemporaryDir temp; const auto wallet = makeWallet(temp.path());
        CommonsPrimitivesBackend backend;
        backend.configure(cli(), wallet);
        QVERIFY(!backend.captureDirectory().isEmpty());
        backend.selectAllowlistWitness(witness(wallet));
        QVERIFY(!object(backend.configure("/missing/not-the-cli", wallet))["configured"].toBool());
        QVERIFY(!backend.configured());
        QVERIFY(backend.captureDirectory().isEmpty());
        QCOMPARE(backend.allowlistWitnessLabel(), QStringLiteral("No witness selected"));
        QVERIFY(!object(backend.lastResultJson())["configured"].toBool());
    }
    void rejectedWitnessReplacementDisarmsTheOldWitness() {
        QTemporaryDir temp; const auto wallet = makeWallet(temp.path());
        CommonsPrimitivesBackend backend; backend.configure(cli(), wallet);
        backend.selectThresholdWitness(witness(wallet));
        QSignalSpy failed(&backend, &CommonsPrimitivesUiSource::operationFailed);
        const auto reply = object(backend.selectThresholdWitness(temp.filePath("outside.bin")));
        QVERIFY(!reply["selected"].toBool());
        QCOMPARE(failed.count(), 1);
        QCOMPARE(backend.thresholdWitnessLabel(), QStringLiteral("No witness selected"));
        QVERIFY(!object(backend.lastResultJson())["selected"].toBool());
        QVERIFY(!object(backend.approveParameter(address()))["accepted"].toBool());
        QVERIFY(!backend.busy());
    }
    void successfulWitnessSelectionClearsPreviousError() {
        QTemporaryDir temp; const auto wallet = makeWallet(temp.path());
        CommonsPrimitivesBackend backend; backend.configure(cli(), wallet);
        backend.selectAllowlistWitness("/not-a-witness");
        QVERIFY(!backend.lastError().isEmpty());
        QSignalSpy finished(&backend, &CommonsPrimitivesUiSource::operationFinished);
        backend.selectAllowlistWitness(witness(wallet));
        QVERIFY(backend.lastError().isEmpty());
        QVERIFY(object(backend.lastResultJson())["selected"].toBool());
        QCOMPARE(finished.count(), 1);
        QVERIFY(!backend.lastResultJson().contains(wallet));
    }
    void invalidReadClearsPreviouslyDisplayedState() {
        QTemporaryDir temp; CommonsPrimitivesBackend backend;
        backend.configure(cli(), makeWallet(temp.path()));
        QSignalSpy finished(&backend, &CommonsPrimitivesUiSource::operationFinished);
        backend.inspectDistribution(address());
        QTRY_COMPARE_WITH_TIMEOUT(finished.count(), 1, 4000);
        QCOMPARE(backend.distributionStateAccount(), address());
        backend.inspectDistribution("bad-state");
        QVERIFY(backend.distributionStateAccount().isEmpty());
        QVERIFY(!backend.distributionSummary().contains("Claims 1/10"));
        QVERIFY(!object(backend.lastResultJson())["success"].toBool());
        QVERIFY(!backend.busy());
    }
    void failedReadCannotKeepAnOlderSuccessSummary() {
        QTemporaryDir temp; CommonsPrimitivesBackend backend;
        backend.configure(cli(), makeWallet(temp.path()));
        QSignalSpy finished(&backend, &CommonsPrimitivesUiSource::operationFinished);
        backend.inspectGroup(address());
        QTRY_COMPARE_WITH_TIMEOUT(finished.count(), 1, 4000);
        QVERIFY(!backend.groupStateAccount().isEmpty());
        qputenv("COMMONS_FAKE_CLI_MODE", "fail");
        QSignalSpy failed(&backend, &CommonsPrimitivesUiSource::operationFailed);
        backend.inspectGroup(address());
        QTRY_COMPARE_WITH_TIMEOUT(failed.count(), 1, 4000);
        QVERIFY(backend.groupStateAccount().isEmpty());
        QVERIFY(!object(backend.lastResultJson())["success"].toBool());
        QVERIFY(!backend.busy());
    }
    void readStatusDoesNotClaimProofGeneration() {
        QTemporaryDir temp; CommonsPrimitivesBackend backend;
        backend.configure(cli(), makeWallet(temp.path()));
        backend.inspectDistribution(address());
        QCOMPARE(backend.statusText(), QStringLiteral("Reading testnet state..."));
        QTRY_VERIFY_WITH_TIMEOUT(!backend.busy(), 4000);
        QCOMPARE(backend.statusText(), QStringLiteral("Testnet state loaded."));
    }
    void activeOperationCannotSwitchWallets() {
        QTemporaryDir temp; const auto wallet = makeWallet(temp.path());
        CommonsPrimitivesBackend backend; backend.configure(cli(), wallet);
        const auto capture = backend.captureDirectory();
        qputenv("COMMONS_FAKE_CLI_MODE", "hang");
        backend.inspectDistribution(address());
        QVERIFY(backend.busy());
        const auto reply = object(backend.configure(cli(), makeWallet(temp.path(), "testnet-b")));
        QVERIFY(!reply["accepted"].toBool());
        QCOMPARE(backend.captureDirectory(), capture);
        QVERIFY(backend.busy());
        QVERIFY(backend.configured());
    }
};

QTEST_GUILESS_MAIN(CommonsBackendStateTest)
#include "commons_backend_state_test.moc"
