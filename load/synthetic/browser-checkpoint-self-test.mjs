#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { webcrypto } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import vm from "node:vm";

const __dirname = dirname(fileURLToPath(import.meta.url));
const appPath = resolve(__dirname, "../../crates/api/static/app.js");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertRejects(label, fn) {
  try {
    fn();
  } catch {
    return;
  }
  throw new Error(`${label} unexpectedly succeeded.`);
}

function base64Url(bytes) {
  return Buffer.from(bytes).toString("base64url");
}

function headHash(fillByte) {
  return base64Url(new Uint8Array(32).fill(fillByte));
}

function createMemoryStorage({ failSet = () => false } = {}) {
  const values = new Map();
  return {
    get length() {
      return values.size;
    },
    key(index) {
      return [...values.keys()][index] || null;
    },
    getItem(key) {
      return values.has(key) ? values.get(key) : null;
    },
    setItem(key, value) {
      if (failSet(String(key))) {
        throw new Error("synthetic localStorage write failure");
      }
      values.set(String(key), String(value));
    },
    removeItem(key) {
      values.delete(String(key));
    },
  };
}

function createElement() {
  const element = {
    textContent: "",
    className: "",
    disabled: false,
    hidden: false,
    value: "",
    checked: false,
    dataset: {},
    classList: {
      add() {},
      remove() {},
      toggle() {},
    },
    addEventListener() {},
    append() {},
    replaceChildren() {},
    setAttribute() {},
    removeAttribute() {},
    focus() {},
    querySelector() {
      return createElement();
    },
    querySelectorAll() {
      return [];
    },
  };
  return element;
}

function createDocument() {
  const elements = new Map();
  return {
    querySelector(selector) {
      if (!elements.has(selector)) {
        elements.set(selector, createElement());
      }
      return elements.get(selector);
    },
    createElement,
  };
}

async function loadApp(storage) {
  const code = await readFile(appPath, "utf8");
  const context = {
    atob: (value) => Buffer.from(value, "base64").toString("binary"),
    btoa: (value) => Buffer.from(value, "binary").toString("base64"),
    console,
    crypto: webcrypto,
    document: createDocument(),
    fetch: async () => ({
      json: async () => ({ status: "ok" }),
      text: async () => "",
      headers: { get: () => "" },
    }),
    TextDecoder,
    TextEncoder,
    URLSearchParams,
    window: { localStorage: storage },
  };
  context.globalThis = context;
  vm.createContext(context);
  vm.runInContext(code, context, { filename: appPath });
  return context;
}

const storage = createMemoryStorage();
const app = await loadApp(storage);
const vaultId = "vault-checkpoint-test";
const head1 = headHash(1);
const head2 = headHash(2);
const forkHead2 = headHash(12);

app.persistVaultCheckpoint({ vaultId, headSeq: 1, headHash: head1 });
assert(app.loadVaultCheckpoint(vaultId).headSeq === 1, "checkpoint seq 1 must load.");
assert(app.loadVaultCheckpoint(vaultId).headHash === head1, "checkpoint hash 1 must load.");

app.persistVaultCheckpoint({ vaultId, headSeq: 2, headHash: head2 });
assert(app.loadVaultCheckpoint(vaultId).headSeq === 2, "newer checkpoint must load.");
assert(app.loadVaultCheckpoint(vaultId).headHash === head2, "newer checkpoint hash must load.");

assertRejects("stale checkpoint overwrite", () =>
  app.persistVaultCheckpoint({ vaultId, headSeq: 1, headHash: head1 }),
);
assertRejects("same-sequence checkpoint fork", () =>
  app.persistVaultCheckpoint({ vaultId, headSeq: 2, headHash: forkHead2 }),
);
assertRejects("invalid checkpoint hash", () =>
  app.persistVaultCheckpoint({ vaultId, headSeq: 3, headHash: "not-a-32-byte-base64url-hash" }),
);

storage.setItem(
  app.vaultCheckpointRecordStorageKey(vaultId, 3, headHash(3)),
  JSON.stringify({
    version: "vault-checkpoint-v1",
    vault_id: vaultId,
    head_seq: 3,
    head_hash: headHash(3),
  }),
);
storage.setItem(
  app.vaultCheckpointStorageKey(vaultId),
  JSON.stringify({
    version: "vault-checkpoint-v1",
    vault_id: vaultId,
    head_seq: 1,
    head_hash: head1,
  }),
);
assert(app.loadVaultCheckpoint(vaultId).headSeq === 3, "append-only record must beat stale pointer.");

const malformedStorage = createMemoryStorage();
const malformedApp = await loadApp(malformedStorage);
malformedStorage.setItem(
  malformedApp.vaultCheckpointStorageKey(vaultId),
  JSON.stringify({
    version: "vault-checkpoint-v1",
    vault_id: vaultId,
    head_seq: 1,
    head_hash: "not-a-32-byte-base64url-hash",
  }),
);
assertRejects("malformed checkpoint load", () => malformedApp.loadVaultCheckpoint(vaultId));

const unavailableStorageApp = await loadApp(
  createMemoryStorage({
    failSet: () => true,
  }),
);
assertRejects("unavailable localStorage load", () => unavailableStorageApp.loadVaultCheckpoint(vaultId));

const writeFailureApp = await loadApp(
  createMemoryStorage({
    failSet: (key) => key !== "password-vault:vault-checkpoint:probe",
  }),
);
assertRejects("checkpoint write failure", () => writeFailureApp.persistVaultCheckpoint({ vaultId, headSeq: 1, headHash: head1 }));

console.log(JSON.stringify({ status: "ok", self_test: "browser_app_checkpoint_storage" }));
