// TEST HARNESS ONLY.
// Compiled by module/CMakeLists.txt to exercise the GUI SDK's process boundary.
// It is not a wallet, sequencer, prover, or source of live transaction data.

#include <QCoreApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QThread>

#include <iostream>
#include <iterator>

namespace {

void writeJson(const QJsonObject& object)
{
    const QByteArray bytes = QJsonDocument(object).toJson(QJsonDocument::Compact);
    std::cout.write(bytes.constData(), bytes.size());
    std::cout << std::endl;
}

} // namespace

int main(int argc, char** argv)
{
    QCoreApplication app(argc, argv);

    const std::string input((std::istreambuf_iterator<char>(std::cin)),
                            std::istreambuf_iterator<char>());
    const QByteArray stdinBytes(input.data(), static_cast<qsizetype>(input.size()));

    const QString mode = qEnvironmentVariable("ASTRA_FAKE_CLI_MODE", "echo");
    if (mode == QStringLiteral("hang")) { QThread::sleep(10); return 0; }
    if (mode == QStringLiteral("oversize-stdout")) { std::cout << std::string(2 * 1024 * 1024, 'x') << std::endl; QThread::sleep(2); return 0; }
    if (mode == QStringLiteral("oversize-stderr")) { std::cerr << std::string(2 * 1024 * 1024, 'x') << std::endl; QThread::sleep(2); return 0; }
    if (mode == QStringLiteral("invalid-json")) {
        std::cout << "{not-json" << std::endl;
        return 0;
    }

    QJsonParseError parseError;
    const QJsonDocument requestDoc = QJsonDocument::fromJson(stdinBytes, &parseError);
    const QJsonObject request = requestDoc.isObject() ? requestDoc.object() : QJsonObject();
    const QJsonObject requestArguments = request.value(QStringLiteral("arguments")).toObject();
    const QString selectedFile = requestArguments.value(QStringLiteral("witness_file")).toString();
    const QString walletDir = request.value(QStringLiteral("wallet_dir")).toString();

    if (mode == QStringLiteral("fail")) {
        QJsonObject out;
        out.insert(QStringLiteral("success"), false);
        out.insert(QStringLiteral("error"),
                   QStringLiteral("wallet %1 witness %2 failed").arg(walletDir, selectedFile));
        writeJson(out);
        return 2;
    }

    QJsonArray argvOut;
    bool argvContainsSelectedFile = false;
    const QStringList args = app.arguments().mid(1);
    for (const QString& arg : args) {
        argvOut.append(arg);
        if (!selectedFile.isEmpty() && arg.contains(selectedFile))
            argvContainsSelectedFile = true;
    }

    QJsonObject state;
    state.insert(QStringLiteral("member_count"), 10);
    state.insert(QStringLiteral("claims_count"), 1);
    state.insert(QStringLiteral("threshold"), 3);
    state.insert(QStringLiteral("value"), QStringLiteral("7"));

    QJsonObject out;
    out.insert(QStringLiteral("success"), true);
    out.insert(QStringLiteral("environment_canary_present"), qEnvironmentVariableIsSet("ASTRA_TEST_SECRET"));
    out.insert(QStringLiteral("operation"), request.value(QStringLiteral("operation")).toString());
    out.insert(QStringLiteral("network"), request.value(QStringLiteral("network")).toString());
    out.insert(QStringLiteral("risc0_dev_mode"), qEnvironmentVariable("RISC0_DEV_MODE"));
    out.insert(QStringLiteral("argv"), argvOut);
    out.insert(QStringLiteral("argv_contains_selected_file"), argvContainsSelectedFile);
    out.insert(QStringLiteral("stdin_contains_selected_file"),
               !selectedFile.isEmpty() && stdinBytes.contains(selectedFile.toUtf8()));
    out.insert(QStringLiteral("state_account"),
               requestArguments.value(QStringLiteral("state_account")).toString());
    out.insert(QStringLiteral("state"), state);
    writeJson(out);
    return 0;
}
