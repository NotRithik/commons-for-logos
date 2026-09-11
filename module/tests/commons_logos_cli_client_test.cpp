// TEST HARNESS ONLY.
// These tests validate the GUI SDK's CLI boundary with a compiled fake CLI.

#include "CommonsLogosCliClient.h"

#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QElapsedTimer>
#include <QtTest/QtTest>

using CommonsLogos::LogosCliClient;
using CommonsLogos::PrimitiveOperation;

namespace {

QString hex32(const QChar fill = QLatin1Char('a'))
{
    return QString(64, fill);
}

QString fakeCliPath()
{
    return QString::fromLocal8Bit(COMMONS_TEST_FAKE_CLI);
}

QString makeTestnetWallet(QTemporaryDir& temp)
{
    QDir root(temp.path());
    if (!root.exists(QStringLiteral("testnet-wallet")))
        root.mkdir(QStringLiteral("testnet-wallet"));
    const QString wallet = root.filePath(QStringLiteral("testnet-wallet"));
    QFile marker(QDir(wallet).filePath(QStringLiteral(".commons-logos-testnet-wallet")));
    marker.open(QIODevice::WriteOnly);
    marker.close();
    return wallet;
}

QString makeWitnessFile(QTemporaryDir& temp)
{
    QDir root(temp.path());
    const QString path = root.filePath(QStringLiteral("testnet-wallet/member-witness.json"));
    QFile file(path);
    file.open(QIODevice::WriteOnly);
    file.write("{\"TEST HARNESS\":\"private witness placeholder\"}\n");
    file.close();
    return path;
}

QJsonObject firstCompletedResult(QSignalSpy& spy)
{
    const QVariantList args = spy.takeFirst();
    return args.at(2).toJsonObject();
}

QString firstFailureMessage(QSignalSpy& spy)
{
    const QVariantList args = spy.takeFirst();
    return args.at(2).toString();
}

} // namespace

class CommonsLogosCliClientTest : public QObject {
    Q_OBJECT

private slots:
    void init()
    {
        qRegisterMetaType<QJsonObject>("QJsonObject");
        qputenv("COMMONS_FAKE_CLI_MODE", "echo");
    }

    void cleanup()
    {
        qunsetenv("COMMONS_FAKE_CLI_MODE");
        qunsetenv("COMMONS_TEST_SECRET");
    }

    void executionReceiptMustProveTheRequestedState_data()
    {
        QTest::addColumn<QByteArray>("mode"); QTest::addColumn<bool>("accepted");
        QTest::newRow("complete") << QByteArray("execute-good") << true;
        QTest::newRow("old-pending") << QByteArray("execute-pending") << false;
        QTest::newRow("wrong-value") << QByteArray("execute-wrong-value") << false;
        QTest::newRow("wrong-sequence") << QByteArray("execute-wrong-sequence") << false;
        QTest::newRow("wrong-account") << QByteArray("execute-wrong-account") << false;
        QTest::newRow("too-few-approvals") << QByteArray("execute-low-threshold") << false;
    }
    void executionReceiptMustProveTheRequestedState()
    {
        QFETCH(QByteArray, mode); QFETCH(bool, accepted);
        qputenv("COMMONS_FAKE_CLI_MODE",mode);
        QTemporaryDir temp; QVERIFY(temp.isValid());
        LogosCliClient client;QString error;
        QVERIFY(client.configure(fakeCliPath(),makeTestnetWallet(temp),&error));
        QSignalSpy complete(&client,&LogosCliClient::completed), failed(&client,&LogosCliClient::failed);
        QVERIFY(client.start(PrimitiveOperation::ThresholdExecute,{{"state_account",hex32()}}).accepted);
        QTRY_COMPARE_WITH_TIMEOUT(complete.count()+failed.count(),1,5000);
        QCOMPARE(complete.count(),accepted?1:0);QCOMPARE(failed.count(),accepted?0:1);
    }

    void configurationRequiresNamedCli()
    {
        QTemporaryDir temp;
        QVERIFY(temp.isValid());
        const QString wallet = makeTestnetWallet(temp);

        LogosCliClient client;
        QString error;
        QVERIFY(!client.configure(QStringLiteral("/bin/sh"), wallet, &error));
        QVERIFY(error.contains(QStringLiteral("commons-logos-cli")));
        QVERIFY(!client.isConfigured());
    }

    void configurationAcceptsCommonsExecutableName()
    {
        QTemporaryDir temp;
        QVERIFY(temp.isValid());
        const QString renamed = QDir(temp.path()).filePath(QStringLiteral("commons-logos-cli"));
        QVERIFY(QFile::copy(fakeCliPath(), renamed));
        QFile(renamed).setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner | QFileDevice::ExeOwner);
        LogosCliClient client;
        QString error;
        QVERIFY(client.configure(renamed, makeTestnetWallet(temp), &error));
    }

    void configurationAcceptsTestnetWallet()
    {
        QTemporaryDir temp;
        QVERIFY(temp.isValid());
        const QString wallet = makeTestnetWallet(temp);

        LogosCliClient client;
        QString error;
        QVERIFY2(client.configure(fakeCliPath(), wallet, &error), qPrintable(error));
        QVERIFY(client.isConfigured());
        QCOMPARE(client.walletDir(), QFileInfo(wallet).canonicalFilePath());
    }

    void claimUsesFixedArgumentsAndJsonStdin()
    {
        QTemporaryDir temp;
        QVERIFY(temp.isValid());
        const QString wallet = makeTestnetWallet(temp);
        const QString witness = makeWitnessFile(temp);

        LogosCliClient client;
        QString error;
        QVERIFY2(client.configure(fakeCliPath(), wallet, &error), qPrintable(error));

        QSignalSpy completed(&client, &LogosCliClient::completed);
        QSignalSpy failed(&client, &LogosCliClient::failed);

        QJsonObject arguments;
        arguments.insert(QStringLiteral("state_account"), hex32());
        arguments.insert(QStringLiteral("witness_file"), witness);
        const auto start = client.start(PrimitiveOperation::AllowlistClaim, arguments);
        QVERIFY2(start.accepted, qPrintable(start.errorMessage));

        QTRY_COMPARE_WITH_TIMEOUT(completed.count(), 1, 5000);
        QCOMPARE(failed.count(), 0);

        const QJsonObject result = firstCompletedResult(completed);
        QCOMPARE(result.value(QStringLiteral("success")).toBool(), true);
        QCOMPARE(result.value(QStringLiteral("operation")).toString(),
                 QStringLiteral("allowlist.claim"));
        QCOMPARE(result.value(QStringLiteral("risc0_dev_mode")).toString(),
                 QStringLiteral("0"));
        QCOMPARE(result.value(QStringLiteral("network")).toString(),
                 QStringLiteral("testnet"));
        QCOMPARE(result.value(QStringLiteral("argv_contains_selected_file")).toBool(), false);
        QCOMPARE(result.value(QStringLiteral("stdin_contains_selected_file")).toBool(), true);

        const QJsonArray argv = result.value(QStringLiteral("argv")).toArray();
        QCOMPARE(argv.size(), 3);
        QCOMPARE(argv.at(0).toString(), QStringLiteral("primitives"));
        QCOMPARE(argv.at(1).toString(), QStringLiteral("allowlist-claim"));
        QCOMPARE(argv.at(2).toString(), QStringLiteral("--json-stdin"));
    }

    void invalidJsonFailsWithoutRawOutput()
    {
        qputenv("COMMONS_FAKE_CLI_MODE", "invalid-json");

        QTemporaryDir temp;
        QVERIFY(temp.isValid());
        const QString wallet = makeTestnetWallet(temp);

        LogosCliClient client;
        QString error;
        QVERIFY2(client.configure(fakeCliPath(), wallet, &error), qPrintable(error));

        QSignalSpy completed(&client, &LogosCliClient::completed);
        QSignalSpy failed(&client, &LogosCliClient::failed);

        QJsonObject arguments;
        arguments.insert(QStringLiteral("state_account"), hex32());
        const auto start = client.start(PrimitiveOperation::AllowlistInspect, arguments);
        QVERIFY2(start.accepted, qPrintable(start.errorMessage));

        QTRY_COMPARE_WITH_TIMEOUT(failed.count(), 1, 5000);
        QCOMPARE(completed.count(), 0);
        const QString message = firstFailureMessage(failed);
        QVERIFY(message.contains(QStringLiteral("invalid JSON")));
        QVERIFY(!message.contains(QStringLiteral("not-json")));
    }

    void cliFailureRedactsWalletAndWitnessPaths()
    {
        qputenv("COMMONS_FAKE_CLI_MODE", "fail");

        QTemporaryDir temp;
        QVERIFY(temp.isValid());
        const QString wallet = makeTestnetWallet(temp);
        const QString witness = makeWitnessFile(temp);

        LogosCliClient client;
        QString error;
        QVERIFY2(client.configure(fakeCliPath(), wallet, &error), qPrintable(error));

        QSignalSpy failed(&client, &LogosCliClient::failed);

        QJsonObject arguments;
        arguments.insert(QStringLiteral("state_account"), hex32());
        arguments.insert(QStringLiteral("witness_file"), witness);
        const auto start = client.start(PrimitiveOperation::AllowlistClaim, arguments);
        QVERIFY2(start.accepted, qPrintable(start.errorMessage));

        QTRY_COMPARE_WITH_TIMEOUT(failed.count(), 1, 5000);
        const QString message = firstFailureMessage(failed);
        QVERIFY(!message.contains(wallet));
        QVERIFY(!message.contains(witness));
        QVERIFY(message.contains(QStringLiteral("[redacted path]")));
    }

    void validatesThresholdBoundsBeforeSpawning()
    {
        QJsonObject arguments;
        arguments.insert(QStringLiteral("root"), hex32(QLatin1Char('b')));
        arguments.insert(QStringLiteral("state_account"), hex32());
        arguments.insert(QStringLiteral("member_count"), 2);
        arguments.insert(QStringLiteral("threshold"), 3);
        arguments.insert(QStringLiteral("initial_value"), QStringLiteral("7"));

        QString error;
        QVERIFY(!LogosCliClient::validateOperationArguments(
            PrimitiveOperation::ThresholdCreateGroup, arguments, &error));
        QVERIFY(error.contains(QStringLiteral("threshold")));
    }
    void exactArgumentAllowlistRejectsUnknownKeys()
    {
        QString error;
        QJsonObject args{{"state_account", hex32()}, {"shell", "not-a-command"}};
        QVERIFY(!LogosCliClient::validateOperationArguments(PrimitiveOperation::AllowlistInspect, args, &error));
        QVERIFY(error.contains("Unknown argument"));
    }
    void everyCreateRequiresThePreselectedStateAccount()
    {
        QString error;
        QJsonObject args{{"root", hex32()}, {"member_count", 10}};
        QVERIFY(!LogosCliClient::validateOperationArguments(PrimitiveOperation::AllowlistCreateDistribution, args, &error));
        args.insert("state_account", hex32());
        QVERIFY(LogosCliClient::validateOperationArguments(PrimitiveOperation::AllowlistCreateDistribution, args, &error));
    }
    void memberCountsRejectInvalidTypes_data()
    {
        QTest::addColumn<QJsonValue>("value");
        QTest::newRow("fraction") << QJsonValue(1.5);
        QTest::newRow("zero") << QJsonValue(0);
        QTest::newRow("negative") << QJsonValue(-1);
        QTest::newRow("above-max") << QJsonValue(257);
        QTest::newRow("string") << QJsonValue(QStringLiteral("10"));
        QTest::newRow("boolean") << QJsonValue(true);
    }
    void memberCountsRejectInvalidTypes()
    {
        QFETCH(QJsonValue, value); QString error;
        QJsonObject args{{"state_account", hex32()}, {"root", hex32()}, {"member_count", value}};
        QVERIFY(!LogosCliClient::validateOperationArguments(PrimitiveOperation::AllowlistCreateDistribution, args, &error));
    }
    void signedI64ExtremesAreAcceptedWithoutDoubleConversion()
    {
        QString error;
        for (const QString value : {QStringLiteral("9223372036854775807"), QStringLiteral("-9223372036854775808")}) {
            QJsonObject args{{"state_account", hex32()}, {"root", hex32()}, {"member_count", 3}, {"threshold", 2}, {"initial_value", value}};
            QVERIFY2(LogosCliClient::validateOperationArguments(PrimitiveOperation::ThresholdCreateGroup, args, &error), qPrintable(error));
        }
    }
    void signedI64OverflowRejected()
    {
        QString error;
        for (const QString value : {QStringLiteral("9223372036854775808"), QStringLiteral("-9223372036854775809"), QStringLiteral("1e5")}) {
            QJsonObject args{{"state_account", hex32()}, {"root", hex32()}, {"member_count", 3}, {"threshold", 2}, {"initial_value", value}};
            QVERIFY(!LogosCliClient::validateOperationArguments(PrimitiveOperation::ThresholdCreateGroup, args, &error));
        }
    }
    void witnessMustStayInsideConfiguredWallet()
    {
        QTemporaryDir temp; const QString wallet = makeTestnetWallet(temp);
        const QString outside = QDir(temp.path()).filePath("outside.bin"); QFile f(outside); QVERIFY(f.open(QIODevice::WriteOnly)); f.write("test-only"); f.close();
        LogosCliClient client; QString error; QVERIFY(client.configure(fakeCliPath(), wallet, &error));
        auto result=client.start(PrimitiveOperation::AllowlistClaim, QJsonObject{{"state_account",hex32()},{"witness_file",outside}});
        QVERIFY(!result.accepted); QVERIFY(result.errorMessage.contains("inside"));
    }
    void witnessSymlinkIsNotAccepted()
    {
        QTemporaryDir temp; const QString wallet=makeTestnetWallet(temp);const QString original=makeWitnessFile(temp);
        const QString linked=QDir(wallet).filePath("linked.bin");QVERIFY(QFile::link(original,linked));
        LogosCliClient client;QString error;QVERIFY(client.configure(fakeCliPath(),wallet,&error));
        QVERIFY(!client.start(PrimitiveOperation::AllowlistClaim,QJsonObject{{"state_account",hex32()},{"witness_file",linked}}).accepted);
    }
    void timeoutClearsBusyAndAllowsRetry()
    {
        QTemporaryDir temp;const QString wallet=makeTestnetWallet(temp);LogosCliClient client(nullptr,120);QString error;
        QVERIFY(client.configure(fakeCliPath(),wallet,&error));QSignalSpy failures(&client,&LogosCliClient::failed);QSignalSpy successes(&client,&LogosCliClient::completed);
        QElapsedTimer timer; timer.start(); qputenv("COMMONS_FAKE_CLI_MODE","hang");QVERIFY(client.start(PrimitiveOperation::AllowlistInspect,QJsonObject{{"state_account",hex32()}}).accepted);
        QTRY_COMPARE_WITH_TIMEOUT(failures.count(),1,4000);QVERIFY(!client.isBusy());QVERIFY(firstFailureMessage(failures).contains("timed out")); QVERIFY(timer.elapsed()<2000);
        qputenv("COMMONS_FAKE_CLI_MODE","echo");QVERIFY(client.start(PrimitiveOperation::AllowlistInspect,QJsonObject{{"state_account",hex32()}}).accepted);
        QTRY_COMPARE_WITH_TIMEOUT(successes.count(),1,4000);
    }
    void outputLimitsKillOnlyOurChild_data()
    {
        QTest::addColumn<QByteArray>("mode");QTest::newRow("stdout")<<QByteArray("oversize-stdout");QTest::newRow("stderr")<<QByteArray("oversize-stderr");
    }
    void outputLimitsKillOnlyOurChild()
    {
        QFETCH(QByteArray,mode);qputenv("COMMONS_FAKE_CLI_MODE",mode);
        QTemporaryDir temp;const QString wallet=makeTestnetWallet(temp);LogosCliClient client;QString error;QVERIFY(client.configure(fakeCliPath(),wallet,&error));
        QSignalSpy failures(&client,&LogosCliClient::failed);QVERIFY(client.start(PrimitiveOperation::AllowlistInspect,QJsonObject{{"state_account",hex32()}}).accepted);
        QTRY_COMPARE_WITH_TIMEOUT(failures.count(),1,5000);QVERIFY(!client.isBusy());const QString message=firstFailureMessage(failures);QVERIFY(message.contains("output limit"));QVERIFY(message.size()<200);
    }
    void concurrentRequestsAndConfigurationChangesAreRefused()
    {
        qputenv("COMMONS_FAKE_CLI_MODE","hang");QTemporaryDir temp;const QString wallet=makeTestnetWallet(temp);LogosCliClient client(nullptr,150);QString error;QVERIFY(client.configure(fakeCliPath(),wallet,&error));
        QSignalSpy failures(&client,&LogosCliClient::failed);auto first=client.start(PrimitiveOperation::AllowlistInspect,QJsonObject{{"state_account",hex32()}});QVERIFY(first.accepted);
        QVERIFY(!client.start(PrimitiveOperation::AllowlistInspect,QJsonObject{{"state_account",hex32()}}).accepted);
        QVERIFY(!client.configure(fakeCliPath(),wallet,&error));client.clearConfiguration();QVERIFY(client.isConfigured());QVERIFY(client.isBusy());
        QTRY_COMPARE_WITH_TIMEOUT(failures.count(),1,4000);QVERIFY(!client.isBusy());
    }
    void processDoesNotInheritUnrelatedEnvironment()
    {
        qputenv("COMMONS_TEST_SECRET","SYNTHETIC_ENVIRONMENT_CANARY_NOT_A_CREDENTIAL");
        QTemporaryDir temp;const QString wallet=makeTestnetWallet(temp);LogosCliClient client;QString error;QVERIFY(client.configure(fakeCliPath(),wallet,&error));
        QSignalSpy success(&client,&LogosCliClient::completed);QVERIFY(client.start(PrimitiveOperation::AllowlistInspect,QJsonObject{{"state_account",hex32()}}).accepted);
        QTRY_COMPARE_WITH_TIMEOUT(success.count(),1,4000);QVERIFY(!firstCompletedResult(success).value("environment_canary_present").toBool());
    }
    void forcesLocalProofsRegardlessOfParentEnvironment()
    {
        // Synthetic sentinels only: no credential or hosted request is used.
        const QByteArray oldProver = qgetenv("RISC0_PROVER");
        const QByteArray oldMode = qgetenv("RISC0_DEV_MODE");
        qputenv("RISC0_PROVER", "bonsai");
        qputenv("RISC0_DEV_MODE", "1");
        qputenv("BONSAI_API_KEY", "SYNTHETIC_TEST_SENTINEL_NOT_A_KEY");
        qputenv("BONSAI_API_URL", "https://invalid.example");
        QTemporaryDir temp;
        const QString wallet = makeTestnetWallet(temp);
        LogosCliClient client;
        QString error;
        QVERIFY(client.configure(fakeCliPath(), wallet, &error));
        QSignalSpy success(&client, &LogosCliClient::completed);
        QVERIFY(client.start(PrimitiveOperation::AllowlistInspect,
                QJsonObject{{"state_account", hex32()}}).accepted);
        QTRY_COMPARE_WITH_TIMEOUT(success.count(), 1, 4000);
        const auto result = firstCompletedResult(success);
        QCOMPARE(result.value("risc0_prover").toString(), QStringLiteral("ipc"));
        QCOMPARE(result.value("risc0_executor").toString(), QStringLiteral("ipc"));
        QCOMPARE(result.value("risc0_dev_mode").toString(), QStringLiteral("0"));
        QVERIFY(!result.value("hosted_prover_credentials_present").toBool());
        qunsetenv("BONSAI_API_KEY");
        qunsetenv("BONSAI_API_URL");
        if (oldProver.isNull()) qunsetenv("RISC0_PROVER"); else qputenv("RISC0_PROVER", oldProver);
        if (oldMode.isNull()) qunsetenv("RISC0_DEV_MODE"); else qputenv("RISC0_DEV_MODE", oldMode);
    }

    void unsupportedEnumDoesNotBecomeACommand()
    {
        QString error;QVERIFY(!LogosCliClient::validateOperationArguments(static_cast<PrimitiveOperation>(999),QJsonObject{{"state_account",hex32()}},&error));
    }

};

QTEST_MAIN(CommonsLogosCliClientTest)
#include "commons_logos_cli_client_test.moc"
