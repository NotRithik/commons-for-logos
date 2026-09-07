#!/usr/bin/env node

import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const qtMcpRoot = process.env.LOGOS_QT_MCP || resolve(__dirname, "../result-mcp");
const { test, run } = await import(resolve(qtMcpRoot, "test-framework/framework.mjs"));

async function findByObjectName(app, objectName) {
  const res = await app.inspector.send("findByProperty", {
    property: "objectName",
    value: objectName,
  });
  if (res.error) throw new Error(`findByProperty(${objectName}) failed: ${res.error}`);
  const match = (res.matches ?? [])[0] || null;
  if (!match) throw new Error(`objectName "${objectName}" not found`);
  return match;
}

async function setProperty(app, objectName, property, value) {
  const object = await findByObjectName(app, objectName);
  const expression = `${property} = ${JSON.stringify(value)}`;
  const res = await app.inspector.send("evaluate", { objectId: object.id, expression });
  if (res.error) throw new Error(`set ${objectName}.${property} failed: ${res.error}`);
}

async function readProperty(app, objectName, property) {
  const object = await findByObjectName(app, objectName);
  const res = await app.inspector.send("evaluate", { objectId: object.id, expression: property });
  if (res.error) throw new Error(`read ${objectName}.${property} failed: ${res.error}`);
  return res.result;
}

async function click(app, objectName) {
  const object = await findByObjectName(app, objectName);
  const res = await app.inspector.send("callMethod", {
    objectId: object.id,
    method: "clicked",
  });
  if (res.error) throw new Error(`click ${objectName} failed: ${res.error}`);
}

test("Commons: loads in explicit not-configured state", async (app) => {
  await app.waitFor(
    async () => {
      await app.expectTexts(["Commons for Logos", "Not configured", "Allowlist", "Shared approvals"]);
    },
    { timeout: 15000, interval: 500, description: "Commons UI to load" },
  );

  const createEnabled = await readProperty(app, "allowlist.create", "enabled");
  if (createEnabled !== false) {
    throw new Error(`allowlist.create enabled=${createEnabled}, expected false before configuration`);
  }
});

test("Commons: invalid configuration is handled by backend", async (app) => {
  await app.waitFor(
    async () => {
      const enabled = await readProperty(app, "config.apply", "enabled");
      if (enabled !== true) throw new Error("Configure button is not enabled yet");
    },
    { timeout: 15000, interval: 500, description: "backend controls to become ready" },
  );

  await setProperty(app, "config.cliPath", "text", "/bin/sh");
  await setProperty(app, "config.walletDir", "text", "/tmp/not-testnet-wallet");
  await click(app, "config.apply");

  await app.waitFor(
    async () => {
      await app.expectTexts(["Select the commons-logos-cli executable."]);
    },
    { timeout: 5000, interval: 250, description: "configuration error to render" },
  );
});

test("Commons: threshold tab exposes real workflow controls", async (app) => {
  await click(app, "tabs.threshold");

  await app.waitFor(
    async () => {
      await findByObjectName(app, "threshold.root");
      await findByObjectName(app, "threshold.memberCount");
      await findByObjectName(app, "threshold.threshold");
      await findByObjectName(app, "threshold.nextValue");
      await findByObjectName(app, "threshold.propose");
      await findByObjectName(app, "threshold.approve");
      await findByObjectName(app, "threshold.execute");
      await findByObjectName(app, "threshold.inspect");
    },
    { timeout: 5000, interval: 250, description: "threshold controls to exist" },
  );

  const proposeEnabled = await readProperty(app, "threshold.propose", "enabled");
  if (proposeEnabled !== false) {
    throw new Error(`threshold.propose enabled=${proposeEnabled}, expected false before configuration`);
  }
});

run();
