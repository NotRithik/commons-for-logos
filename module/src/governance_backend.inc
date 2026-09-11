// Local onboarding for the native owner interface. No arbitrary command or
// network endpoint can be supplied by a QML request.
#include <QProcess>
#include <QProcessEnvironment>
#include <QStandardPaths>
#include <QJsonParseError>

QString CommonsPrimitivesBackend::governanceHome() const
{
    const QString configuredHome = qEnvironmentVariable("COMMONS_GOVERNANCE_HOME");
    return configuredHome.isEmpty()
        ? QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation) + "/commons-governance"
        : QDir::cleanPath(configuredHome);
}

QString CommonsPrimitivesBackend::governanceFailure(const QString& message)
{
    setLastError(message);
    setStatusText(message);
    Q_EMIT operationFailed("governance.setup", message);
    return compactJson(QJsonObject{{"accepted", false}, {"error", message}});
}

QString CommonsPrimitivesBackend::governanceRequest(const QString& action, const QString& identity,
                                                    const QJsonObject& arguments)
{
    static const QStringList allowed {"catalog", "create-identity", "prepare-policy", "prepare-list", "join-policy"};
    if (!allowed.contains(action)) return governanceFailure("Unknown governance setup action.");
    if (busy() || m_client.isBusy() || m_governanceProcess)
        return governanceFailure("Wait for the current operation to finish before changing identities or policies.");
    if (!m_client.isConfigured())
        return governanceFailure("Connect the installed Commons client first. A view-only workspace is sufficient for setup.");
    if (!QFileInfo(governanceHome()).isAbsolute()) return governanceFailure("Governance home must be an absolute directory.");
    const QJsonObject request {{"schema_version",1},{"home",governanceHome()},{"profile",identity},{"arguments",arguments}};
    const QByteArray input = QJsonDocument(request).toJson(QJsonDocument::Compact);
    if (input.size() > 256*1024) return governanceFailure("This setup request is too large. Use at most 256 enrollments.");

    auto* process = new QProcess(this);
    m_governanceProcess = process;
    m_governanceOutput.clear();
    process->setProgram(m_client.cliPath());
    process->setArguments({"governance", action, "--json-stdin"});
    process->setProcessChannelMode(QProcess::SeparateChannels);
    QProcessEnvironment env;
    const auto inherited = QProcessEnvironment::systemEnvironment();
    for (const auto& name : QStringList {"HOME","PATH","TMPDIR","SSL_CERT_FILE","LANG","LC_ALL","DYLD_LIBRARY_PATH"})
        if (inherited.contains(name)) env.insert(name, inherited.value(name));
    env.insert("RISC0_DEV_MODE","0");
    env.insert("RISC0_PROVER","ipc");
    process->setProcessEnvironment(env);
    setBusy(true);
    setActiveOperation("Governance setup");
    setLastOperation("governance." + action);
    setLastError(QString());
    setStatusText(action == "create-identity"
        ? "Creating a new local identity. Reading the testnet checkpoint; no transaction will be sent..."
        : "Preparing governance locally. No transaction will be sent...");

    connect(process, &QProcess::readyReadStandardOutput, this, [this, process] {
        const QByteArray chunk = process->readAllStandardOutput();
        if (m_governanceProcess != process) return;
        if (m_governanceOutput.size() + chunk.size() > 8*1024*1024) {
            process->setProperty("commonsTooLarge", true);
            process->kill();
        } else m_governanceOutput += chunk;
    });
    connect(process, &QProcess::readyReadStandardError, process, [process] {
        process->readAllStandardError(); // Dependency logs may contain paths; never echo them into chat or UI.
    });
    connect(process, &QProcess::errorOccurred, this, [this, process](QProcess::ProcessError error) {
        if (error != QProcess::FailedToStart || m_governanceProcess != process) return;
        m_governanceProcess = nullptr;
        m_governanceOutput.clear();
        setBusy(false); setActiveOperation(QString());
        governanceFailure("The Commons setup client could not start. Check the installed client path.");
        process->deleteLater();
    });
    connect(process, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), this,
        [this, process, action](int exitCode, QProcess::ExitStatus exitStatus) {
        if (m_governanceProcess != process) return;
        const QByteArray tail = process->readAllStandardOutput();
        if (m_governanceOutput.size()+tail.size() <= 8*1024*1024) m_governanceOutput += tail;
        else process->setProperty("commonsTooLarge",true);
        QJsonParseError parse;
        const QJsonDocument document = QJsonDocument::fromJson(m_governanceOutput,&parse);
        const QJsonObject result = document.object();
        m_governanceProcess = nullptr;
        m_governanceOutput.clear();
        setBusy(false); setActiveOperation(QString());
        QString error;
        if (process->property("commonsTooLarge").toBool()) error = "This identity's catalogue is too large to display. Split the local library into smaller profiles.";
        else if (process->property("commonsTimedOut").toBool()) error = "Setup timed out while reading testnet. No transaction was sent. Refresh the identity list before trying again.";
        else if (parse.error != QJsonParseError::NoError || !document.isObject()) error = "The installed client did not return a valid governance response. Install the matching new client package.";
        else if (exitCode != 0 || exitStatus != QProcess::NormalExit || result.value("success") != true)
            error = result.value("error").toObject().value("message").toString("Governance setup did not finish. No transaction was submitted.");
        if (!error.isEmpty()) { governanceFailure(error.left(500)); process->deleteLater(); return; }
        if (!result.value("profiles").isArray() || !result.value("policies").isArray()
            || result.value("transactions_submitted").toInt(-1) != 0) {
            governanceFailure("The client returned an unexpected setup result."); process->deleteLater(); return;
        }
        const QString profileDir = result.value("profile_dir").toString();
        if (!profileDir.isEmpty()) {
            const QFileInfo info(profileDir);
            const QString canonicalHome = QFileInfo(governanceHome()).canonicalFilePath();
            if (canonicalHome.isEmpty() || !info.isDir() || info.isSymLink()
                || QFileInfo(info.canonicalFilePath()).dir().canonicalPath() != canonicalHome) {
                governanceFailure("The returned identity is outside the selected governance library."); process->deleteLater(); return;
            }
            const QString cli = m_client.cliPath();
            configure(cli, info.canonicalFilePath());
            if (!configured()) { process->deleteLater(); return; }
            setActiveProfileLabel(result.value("identity_label").toString());
            setActiveProfileKind("threshold");
            setActiveProfileAccount(QString());
        }
        m_governanceCatalogue = result;
        setGovernanceStateJson(compactJson(result));
        setActiveGovernancePolicyJson("{}");
        setStatusText(result.value("message").toString("Governance library loaded."));
        const QJsonObject receipt {{"success",true},{"operation","governance."+action},
            {"identities",result.value("profiles").toArray().size()},
            {"policies",result.value("policies").toArray().size()},{"transactions_submitted",0}};
        setLastResultJson(prettyJson(receipt));
        Q_EMIT operationFinished("governance."+action,compactJson(receipt));
        process->deleteLater();
    });
    QTimer::singleShot(90000, process, [process] {
        if (process->state()!=QProcess::NotRunning) {process->setProperty("commonsTimedOut",true); process->kill();}
    });
    process->start();
    process->write(input + '\n');
    process->closeWriteChannel();
    return compactJson(QJsonObject{{"accepted",true},{"operation","governance."+action}});
}

QString CommonsPrimitivesBackend::governanceRefresh()
{
    return governanceRequest("catalog",m_governanceCatalogue.value("selected_profile").toString(),{});
}
QString CommonsPrimitivesBackend::governanceCreateIdentity(QString label)
{
    return governanceRequest("create-identity","",{{"label",label.trimmed()}});
}
QString CommonsPrimitivesBackend::governanceSelectIdentity(QString identity)
{
    bool found=false;
    for (const auto& row : m_governanceCatalogue.value("profiles").toArray())
        if (row.toObject().value("id").toString()==identity) found=true;
    if (!found) return governanceFailure("Choose an identity from this library.");
    return governanceRequest("catalog",identity,{});
}
QString CommonsPrimitivesBackend::governancePreparePolicy(QString title, QString field, QString initialValue,
                                                           int threshold, QString enrollmentsJson)
{
    QJsonParseError parse;
    const auto document=QJsonDocument::fromJson(enrollmentsJson.toUtf8(),&parse);
    if (parse.error!=QJsonParseError::NoError || !document.isArray() || document.array().isEmpty())
        return governanceFailure("Add at least one member enrollment. Use the provided enrollment JSON, not a private wallet or key.");
    return governanceRequest("prepare-policy",m_governanceCatalogue.value("selected_profile").toString(),
        {{"title",title.trimmed()},{"field_name",field.trimmed()},{"initial_value",initialValue.trimmed()},
         {"threshold",threshold},{"members",document.array()}});
}
QString CommonsPrimitivesBackend::governanceJoinPolicy(QString invitationJson)
{
    QJsonParseError parse;
    const auto document=QJsonDocument::fromJson(invitationJson.toUtf8(),&parse);
    if (parse.error!=QJsonParseError::NoError || !document.isObject()) return governanceFailure("Paste the invitation JSON supplied by the organizer.");
    return governanceRequest("join-policy",m_governanceCatalogue.value("selected_profile").toString(),{{"invitation",document.object()}});
}
QString CommonsPrimitivesBackend::governanceOpenPolicy(int index)
{
    if (busy() || m_governanceProcess) return governanceFailure("Wait for the current operation to finish.");
    const auto policies=m_governanceCatalogue.value("policies").toArray();
    if (index<0 || index>=policies.size()) return governanceFailure("Choose a saved policy.");
    QJsonObject policy=policies.at(index).toObject();
    const QString walletDir=m_governanceCatalogue.value("profile_dir").toString();
    if (walletDir.isEmpty()) return governanceFailure("Select a member identity first.");
    if (m_client.walletDir()!=QFileInfo(walletDir).canonicalFilePath()) {
        const QString cli=m_client.cliPath(); configure(cli,walletDir);
    }
    if (!configured() || readOnly()) return governanceFailure("Select a writable member identity.");
    setActiveProfileLabel(m_governanceCatalogue.value("identity_label").toString()+" / "+policy.value("title").toString());
    const bool membership = policy.value("kind").toString() == "allowlist";
    if (policy.contains("kind") && !membership && policy.value("kind").toString() != "threshold")
        return governanceFailure("This workspace type is not supported by the installed client.");
    setActiveProfileKind(membership ? "allowlist" : "threshold");
    setActiveProfileAccount(policy.value("state_account").toString());
    // Make repeated local selections observable even for the same policy.
    policy.insert("selection_revision", ++m_governanceSelectionRevision);
    setActiveGovernancePolicyJson(compactJson(policy));
    m_thresholdWitnessPath.clear(); setThresholdWitnessLabel("No witness selected"); setThresholdWitnessReady(false);
    m_allowlistWitnessPath.clear(); setAllowlistWitnessLabel("No witness selected"); setAllowlistWitnessReady(false);
    const QString witness=policy.value("witness_file").toString();
    if (!witness.isEmpty()) {
        if (membership) selectAllowlistWitness(witness);
        else selectThresholdWitness(witness);
    }
    setStatusText("Policy selected. Refresh its live state, or review and publish this draft. Selecting it does not send a transaction.");
    return compactJson(QJsonObject{{"opened",true},{"state_account",policy.value("state_account")}});
}


QString CommonsPrimitivesBackend::governanceJoinOwnPolicy(int index)
{
    if (busy() || m_governanceProcess) return governanceFailure("Wait for the current operation to finish.");
    const auto rows = m_governanceCatalogue.value("policies").toArray();
    if (index < 0 || index >= rows.size()) return governanceFailure("Choose a saved policy first.");
    const auto own = m_governanceCatalogue.value("enrollment").toObject().value("account_id").toString();
    if (own.isEmpty()) return governanceFailure("Select your member identity first.");
    for (const auto& item : rows.at(index).toObject().value("invitations").toArray()) {
        const auto invitation = item.toObject();
        if (invitation.value("member").toObject().value("account_id").toString() == own)
            return governanceJoinPolicy(compactJson(invitation));
    }
    return governanceFailure("This identity is not included in the saved member invitations. Import the invitation supplied for your own identity.");
}


QString CommonsPrimitivesBackend::membershipPrepareList(QString title, QString enrollmentsJson)
{
    QJsonParseError parse;
    const auto document = QJsonDocument::fromJson(enrollmentsJson.toUtf8(), &parse);
    if (parse.error != QJsonParseError::NoError || !document.isArray()
        || document.array().isEmpty() || document.array().size() > 256)
        return governanceFailure("Add between 1 and 256 distinct member enrollments. Never paste a private wallet or key.");
    return governanceRequest("prepare-list", m_governanceCatalogue.value("selected_profile").toString(),
        {{"title", title.trimmed()}, {"members", document.array()}});
}
