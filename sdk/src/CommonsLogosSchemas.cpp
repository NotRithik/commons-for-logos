#include "CommonsLogosSchemas.h"

#include <QDir>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonValue>
#include <QRegularExpression>
#include <QUrl>

namespace CommonsLogos {
namespace {

const char* schemaLiteral()
{
    return R"JSON({
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://raw.githubusercontent.com/NotRithik/commons-for-logos/main/sdk/schema.json",
  "title": "Commons Logos Primitive CLI Contract",
  "type": "object",
  "required": [
    "schema_version",
    "network",
    "wallet_dir",
    "operation",
    "arguments"
  ],
  "properties": {
    "schema_version": {
      "const": 1
    },
    "network": {
      "const": "testnet"
    },
    "wallet_dir": {
      "type": "string",
      "minLength": 1
    },
    "operation": {
      "enum": [
        "allowlist.create_distribution",
        "allowlist.claim",
        "allowlist.inspect_state",
        "threshold.create_group",
        "threshold.propose",
        "threshold.approve",
        "threshold.execute",
        "threshold.inspect_state"
      ]
    },
    "arguments": {
      "type": "object"
    }
  },
  "definitions": {
    "hash32_hex": {
      "type": "string",
      "pattern": "^(0x)?[0-9a-fA-F]{64}$"
    },
    "member_count": {
      "type": "integer",
      "minimum": 1,
      "maximum": 256
    },
    "i64_decimal": {
      "type": "string",
      "pattern": "^-?(0|[1-9][0-9]{0,18})$"
    },
    "state_account": {
      "type": "string",
      "pattern": "^(0x)?[0-9a-fA-F]{64}$"
    },
    "witness_file": {
      "type": "string",
      "minLength": 1
    }
  },
  "success_response": {
    "type": "object",
    "required": [
      "success"
    ],
    "properties": {
      "success": {
        "const": true
      },
      "tx_hash": {
        "type": "string"
      },
      "state_account": {
        "$ref": "#/definitions/state_account"
      },
      "state": {
        "type": "object"
      }
    },
    "additionalProperties": true
  },
  "failure_response": {
    "type": "object",
    "required": [
      "success",
      "error"
    ],
    "properties": {
      "success": {
        "const": false
      },
      "error": {
        "oneOf": [
          {
            "type": "string"
          },
          {
            "type": "object",
            "required": [
              "message"
            ],
            "properties": {
              "message": {
                "type": "string"
              }
            },
            "additionalProperties": true
          }
        ]
      }
    },
    "additionalProperties": true
  },
  "allOf": [
    {
      "if": {
        "properties": {
          "operation": {
            "const": "allowlist.create_distribution"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account",
              "root",
              "member_count"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              },
              "root": {
                "$ref": "#/definitions/hash32_hex"
              },
              "member_count": {
                "$ref": "#/definitions/member_count"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "allowlist.claim"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account",
              "witness_file"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              },
              "witness_file": {
                "$ref": "#/definitions/witness_file"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "allowlist.inspect_state"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "threshold.create_group"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account",
              "root",
              "member_count",
              "threshold",
              "initial_value"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              },
              "root": {
                "$ref": "#/definitions/hash32_hex"
              },
              "member_count": {
                "$ref": "#/definitions/member_count"
              },
              "threshold": {
                "$ref": "#/definitions/member_count"
              },
              "initial_value": {
                "$ref": "#/definitions/i64_decimal"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "threshold.propose"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account",
              "witness_file",
              "next_value"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              },
              "witness_file": {
                "$ref": "#/definitions/witness_file"
              },
              "next_value": {
                "$ref": "#/definitions/i64_decimal"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "threshold.approve"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account",
              "witness_file"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              },
              "witness_file": {
                "$ref": "#/definitions/witness_file"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "threshold.execute"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              }
            },
            "additionalProperties": false
          }
        }
      }
    },
    {
      "if": {
        "properties": {
          "operation": {
            "const": "threshold.inspect_state"
          }
        }
      },
      "then": {
        "properties": {
          "arguments": {
            "type": "object",
            "required": [
              "state_account"
            ],
            "properties": {
              "state_account": {
                "$ref": "#/definitions/state_account"
              }
            },
            "additionalProperties": false
          }
        }
      }
    }
  ],
  "additionalProperties": false
})JSON";
}

QString normalizedHex(QString value)
{
    value = value.trimmed();
    if (value.startsWith(QStringLiteral("0x"), Qt::CaseInsensitive))
        value = value.mid(2);
    return value;
}

bool setError(QString* errorMessage, const QString& value)
{
    if (errorMessage)
        *errorMessage = value;
    return false;
}

bool pathSegmentLooksTestnet(const QString& canonicalPath)
{
    const QString lower = QDir::fromNativeSeparators(canonicalPath).toLower();
    const QStringList parts = lower.split(QLatin1Char('/'), Qt::SkipEmptyParts);
    for (const QString& part : parts) {
        if (part.contains(QStringLiteral("testnet")))
            return true;
    }
    return false;
}

bool hasTestnetMarker(const QDir& dir)
{
    return dir.exists(QStringLiteral(".commons-logos-testnet-wallet"))
        || dir.exists(QStringLiteral(".logos-testnet"))
        || dir.exists(QStringLiteral("testnet.config.yaml"));
}

bool isSensitiveKey(const QString& key)
{
    const QString lower = key.toLower();
    return lower.contains(QStringLiteral("witness"))
        || lower.contains(QStringLiteral("secret"))
        || lower.contains(QStringLiteral("private_key"))
        || lower.contains(QStringLiteral("privatekey"))
        || lower.contains(QStringLiteral("viewing_public_key"))
        || lower.contains(QStringLiteral("nullifier"))
        || lower.contains(QStringLiteral("mnemonic"))
        || lower.contains(QStringLiteral("password"))
        || lower.contains(QStringLiteral("seed"))
        || lower == QStringLiteral("salt");
}

QJsonValue redactValue(const QJsonValue& value,
                       const QStringList& sensitivePaths,
                       const QString& key = QString())
{
    if (isSensitiveKey(key))
        return QStringLiteral("[redacted]");

    if (value.isObject()) {
        QJsonObject out;
        const QJsonObject object = value.toObject();
        for (auto it = object.constBegin(); it != object.constEnd(); ++it)
            out.insert(it.key(), redactValue(it.value(), sensitivePaths, it.key()));
        return out;
    }

    if (value.isArray()) {
        QJsonArray out;
        const QJsonArray array = value.toArray();
        for (const QJsonValue& item : array)
            out.append(redactValue(item, sensitivePaths));
        return out;
    }

    if (value.isString()) {
        QString s = value.toString();
        for (const QString& rawPath : sensitivePaths) {
            const QString path = localPathFromUi(rawPath);
            if (!path.isEmpty())
                s.replace(path, QStringLiteral("[redacted path]"), Qt::CaseSensitive);
        }
        return s;
    }

    return value;
}

} // namespace

QString operationId(const PrimitiveOperation operation)
{
    switch (operation) {
    case PrimitiveOperation::AllowlistCreateDistribution:
        return QStringLiteral("allowlist.create_distribution");
    case PrimitiveOperation::AllowlistClaim:
        return QStringLiteral("allowlist.claim");
    case PrimitiveOperation::AllowlistInspect:
        return QStringLiteral("allowlist.inspect_state");
    case PrimitiveOperation::ThresholdCreateGroup:
        return QStringLiteral("threshold.create_group");
    case PrimitiveOperation::ThresholdPropose:
        return QStringLiteral("threshold.propose");
    case PrimitiveOperation::ThresholdApprove:
        return QStringLiteral("threshold.approve");
    case PrimitiveOperation::ThresholdExecute:
        return QStringLiteral("threshold.execute");
    case PrimitiveOperation::ThresholdInspect:
        return QStringLiteral("threshold.inspect_state");
    }
    return {};
}

QString operationDisplayName(const PrimitiveOperation operation)
{
    switch (operation) {
    case PrimitiveOperation::AllowlistCreateDistribution:
        return QStringLiteral("Create distribution");
    case PrimitiveOperation::AllowlistClaim:
        return QStringLiteral("Claim allowlist entry");
    case PrimitiveOperation::AllowlistInspect:
        return QStringLiteral("Inspect distribution state");
    case PrimitiveOperation::ThresholdCreateGroup:
        return QStringLiteral("Create threshold group");
    case PrimitiveOperation::ThresholdPropose:
        return QStringLiteral("Propose parameter change");
    case PrimitiveOperation::ThresholdApprove:
        return QStringLiteral("Approve parameter change");
    case PrimitiveOperation::ThresholdExecute:
        return QStringLiteral("Execute parameter change");
    case PrimitiveOperation::ThresholdInspect:
        return QStringLiteral("Inspect group state");
    }
    return {};
}

QStringList cliArgumentsForOperation(const PrimitiveOperation operation)
{
    switch (operation) {
    case PrimitiveOperation::AllowlistCreateDistribution:
        return { QStringLiteral("primitives"), QStringLiteral("allowlist-create"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::AllowlistClaim:
        return { QStringLiteral("primitives"), QStringLiteral("allowlist-claim"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::AllowlistInspect:
        return { QStringLiteral("primitives"), QStringLiteral("allowlist-inspect"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::ThresholdCreateGroup:
        return { QStringLiteral("primitives"), QStringLiteral("threshold-create"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::ThresholdPropose:
        return { QStringLiteral("primitives"), QStringLiteral("threshold-propose"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::ThresholdApprove:
        return { QStringLiteral("primitives"), QStringLiteral("threshold-approve"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::ThresholdExecute:
        return { QStringLiteral("primitives"), QStringLiteral("threshold-execute"),
                 QStringLiteral("--json-stdin") };
    case PrimitiveOperation::ThresholdInspect:
        return { QStringLiteral("primitives"), QStringLiteral("threshold-inspect"),
                 QStringLiteral("--json-stdin") };
    }
    return {};
}

bool isAllowedOperationId(const QString& id)
{
    return id == operationId(PrimitiveOperation::AllowlistCreateDistribution)
        || id == operationId(PrimitiveOperation::AllowlistClaim)
        || id == operationId(PrimitiveOperation::AllowlistInspect)
        || id == operationId(PrimitiveOperation::ThresholdCreateGroup)
        || id == operationId(PrimitiveOperation::ThresholdPropose)
        || id == operationId(PrimitiveOperation::ThresholdApprove)
        || id == operationId(PrimitiveOperation::ThresholdExecute)
        || id == operationId(PrimitiveOperation::ThresholdInspect);
}

QString contractSchemaJson()
{
    return QString::fromUtf8(schemaLiteral());
}

QJsonObject contractSchemaObject()
{
    QJsonParseError error;
    const QJsonDocument doc = QJsonDocument::fromJson(contractSchemaJson().toUtf8(), &error);
    if (error.error != QJsonParseError::NoError || !doc.isObject())
        return {};
    return doc.object();
}

bool validateHex32(const QString& value)
{
    static const QRegularExpression re(QStringLiteral("^[0-9a-fA-F]{64}$"));
    return re.match(normalizedHex(value)).hasMatch();
}

bool validateI64Decimal(const QString& value, qlonglong* parsed)
{
    const QString trimmed = value.trimmed();
    static const QRegularExpression re(QStringLiteral("^-?(0|[1-9][0-9]{0,18})$"));
    if (!re.match(trimmed).hasMatch())
        return false;
    bool ok = false;
    const qlonglong result = trimmed.toLongLong(&ok, 10);
    if (!ok)
        return false;
    if (parsed)
        *parsed = result;
    return true;
}

bool validateCliPath(const QString& rawCliPath, QString* errorMessage)
{
    const QString cliPath = localPathFromUi(rawCliPath);
    if (cliPath.isEmpty())
        return setError(errorMessage, QStringLiteral("Select the Commons CLI executable."));

    const QFileInfo info(cliPath);
    if (!info.isAbsolute())
        return setError(errorMessage, QStringLiteral("The CLI path must be absolute."));
    if (!info.exists() || !info.isFile())
        return setError(errorMessage, QStringLiteral("The configured client file does not exist."));
    if (!info.isExecutable())
        return setError(errorMessage, QStringLiteral("The configured client file is not executable."));

    const QString fileName = info.fileName();
    if (fileName != QStringLiteral("commons-logos-cli")
        && fileName != QStringLiteral("commons-logos-cli.exe")) {
        return setError(errorMessage,
                        QStringLiteral("Select the commons-logos-cli executable."));
    }

    return true;
}

bool validateWalletDir(const QString& rawWalletDir,
                       QString* canonicalWalletDir,
                       QString* errorMessage)
{
    const QString walletDir = localPathFromUi(rawWalletDir);
    if (walletDir.isEmpty())
        return setError(errorMessage, QStringLiteral("Select a testnet wallet directory."));

    const QFileInfo info(walletDir);
    if (!info.isAbsolute())
        return setError(errorMessage, QStringLiteral("The wallet directory path must be absolute."));
    if (!info.exists() || !info.isDir())
        return setError(errorMessage, QStringLiteral("The selected wallet directory does not exist."));

    const QString canonical = info.canonicalFilePath();
    if (canonical.isEmpty())
        return setError(errorMessage, QStringLiteral("The wallet directory could not be resolved safely."));

    const QDir dir(canonical);
    if (!pathSegmentLooksTestnet(canonical) && !hasTestnetMarker(dir)) {
        return setError(errorMessage,
                        QStringLiteral("The wallet directory must be testnet-only. Use a path containing 'testnet' or add a .commons-logos-testnet-wallet marker."));
    }

    if (canonicalWalletDir)
        *canonicalWalletDir = canonical;
    return true;
}

QString localPathFromUi(const QString& value)
{
    const QString trimmed = value.trimmed();
    if (trimmed.startsWith(QStringLiteral("file:"), Qt::CaseInsensitive)) {
        const QUrl url(trimmed);
        if (url.isLocalFile())
            return url.toLocalFile();
    }
    return QDir::fromNativeSeparators(trimmed);
}

QString fileLabel(const QString& path)
{
    const QString local = localPathFromUi(path);
    if (local.isEmpty())
        return QStringLiteral("No file selected");
    return QFileInfo(local).fileName();
}

QString sanitizeForUi(const QString& message, const QStringList& sensitivePaths)
{
    QString out = message;
    for (const QString& rawPath : sensitivePaths) {
        const QString path = localPathFromUi(rawPath);
        if (!path.isEmpty())
            out.replace(path, QStringLiteral("[redacted path]"), Qt::CaseSensitive);
    }

    out.replace(QRegularExpression(QStringLiteral("[\\x00-\\x08\\x0b\\x0c\\x0e-\\x1f\\x7f]")),
                QStringLiteral(""));
    out = out.simplified();
    constexpr qsizetype maxLength = 1000;
    if (out.size() > maxLength)
        out = out.left(maxLength) + QStringLiteral("...");
    return out;
}

QJsonObject redactSensitiveJsonObject(const QJsonObject& object,
                                      const QStringList& sensitivePaths)
{
    return redactValue(object, sensitivePaths).toObject();
}

} // namespace CommonsLogos
