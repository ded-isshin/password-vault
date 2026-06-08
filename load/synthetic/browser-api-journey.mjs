#!/usr/bin/env node
import { randomUUID, webcrypto } from "node:crypto";

const AUTH_PROTOCOL = "derived-auth-v1";
const KDF_PROFILE_ID = "pbkdf2-sha256-browser-v1";
const KDF_ITERATIONS = 600000;
const ACCOUNT_KEYSET_CRYPTO_VERSION = "account-keyset-v1";
const VAULT_KEY_WRAP_CRYPTO_VERSION = "vault-key-wrap-v1";
const VAULT_CRYPTO_PROFILE_ID = "vault-crypto-v1";
const ITEM_ENVELOPE_CRYPTO_VERSION = "item-envelope-v1";
const ITEM_ENVELOPE_AEAD = "AES-256-GCM";
const SCRAM_PROFILE_ID = "pv-scram-sha-256-v1";
const SCRAM_ITERATIONS = 150000;
const VAULT_CHECKPOINT_VERSION = "vault-checkpoint-v1";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

const subtle = webcrypto.subtle;

function boolEnv(name, defaultValue = false) {
  const value = process.env[name];
  if (value === undefined || value === "") {
    return defaultValue;
  }
  return ["1", "true", "yes", "on"].includes(value.toLowerCase());
}

function sanitizeRunId(value) {
  const sanitized = String(value || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
  return sanitized || `run-${Date.now()}`;
}

function validateSyntheticPrefix(value) {
  if (!/^[a-z0-9.-]{1,32}$/.test(value)) {
    throw new Error("SYNTHETIC_LOGIN_PREFIX must be a lowercase safe label up to 32 characters.");
  }
}

function validateSyntheticDomain(value) {
  if (
    value.length < ".invalid".length + 1 ||
    value.length > 80 ||
    !value.endsWith(".invalid") ||
    value.startsWith(".") ||
    value.includes("..") ||
    !/^[a-z0-9.-]+$/.test(value)
  ) {
    throw new Error("SYNTHETIC_EMAIL_DOMAIN must be a safe reserved .invalid domain.");
  }
}

function loadConfig() {
  if (boolEnv("SYNTHETIC_TLS_INSECURE", false)) {
    process.env.NODE_TLS_REJECT_UNAUTHORIZED = "0";
  }

  const baseUrl = new URL(process.env.BASE_URL || "http://127.0.0.1:8080");
  const metricsBaseUrl = new URL(process.env.METRICS_BASE_URL || defaultMetricsBaseUrl(baseUrl));
  const allowNonLocal = boolEnv("SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL", false);
  if (!allowNonLocal && !isLocalBaseUrl(baseUrl)) {
    throw new Error(
      "Refusing non-local BASE_URL. Set SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true for an explicit live-edge run.",
    );
  }

  const checkMetrics =
    process.env.SYNTHETIC_CHECK_METRICS === undefined
      ? isLocalBaseUrl(baseUrl)
      : boolEnv("SYNTHETIC_CHECK_METRICS", true);
  if (checkMetrics && !allowNonLocal && !isLocalBaseUrl(metricsBaseUrl)) {
    throw new Error(
      "Refusing non-local METRICS_BASE_URL. Set SYNTHETIC_ALLOW_NON_LOCAL_BASE_URL=true for an explicit live-edge run.",
    );
  }
  const runId = sanitizeRunId(process.env.RUN_ID || `local-${Date.now()}`);
  const prefix = sanitizeRunId(process.env.SYNTHETIC_LOGIN_PREFIX || "synthetic");
  const domain = String(process.env.SYNTHETIC_EMAIL_DOMAIN || "loadtest.invalid")
    .trim()
    .toLowerCase();
  validateSyntheticPrefix(prefix);
  validateSyntheticDomain(domain);
  const timeoutMs = Number(process.env.SYNTHETIC_TIMEOUT_MS || "120000");
  if (!Number.isFinite(timeoutMs) || timeoutMs < 1000) {
    throw new Error("SYNTHETIC_TIMEOUT_MS must be at least 1000.");
  }

  return {
    baseUrl,
    metricsBaseUrl,
    checkMetrics,
    runId,
    loginHandle: `${prefix}-${runId}-${base64Url(randomBytes(6))}@${domain}`,
    masterPassword: `Synthetic-${runId}-${base64Url(randomBytes(18))}-A1!`,
    timeoutMs,
  };
}

function defaultMetricsBaseUrl(baseUrl) {
  if (!isLocalBaseUrl(baseUrl)) {
    return baseUrl.toString();
  }
  const metricsUrl = new URL(baseUrl.toString());
  metricsUrl.port = "9090";
  return metricsUrl.toString();
}

function isLocalBaseUrl(baseUrl) {
  const host = baseUrl.hostname.toLowerCase();
  return host === "localhost" || host === "::1" || host === "127.0.0.1" || host.startsWith("127.");
}

function logStep(message) {
  console.log(`[synthetic] ${message}`);
}

function textBytes(value) {
  return encoder.encode(value);
}

function jsonBytes(value) {
  return textBytes(JSON.stringify(value));
}

function randomBytes(length) {
  const bytes = new Uint8Array(length);
  webcrypto.getRandomValues(bytes);
  return bytes;
}

function concatBytes(...parts) {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const output = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    output.set(part, offset);
    offset += part.length;
  }
  return output;
}

function bytesEqual(left, right) {
  if (left.length !== right.length) {
    return false;
  }
  let diff = 0;
  for (let index = 0; index < left.length; index += 1) {
    diff |= left[index] ^ right[index];
  }
  return diff === 0;
}

function wipe(...items) {
  for (const item of items) {
    if (item instanceof Uint8Array) {
      item.fill(0);
    }
  }
}

function base64Url(input) {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  return Buffer.from(bytes)
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
}

function base64UrlToBytes(value) {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(
    Math.ceil(value.length / 4) * 4,
    "=",
  );
  return new Uint8Array(Buffer.from(padded, "base64"));
}

function displayAccountSecretKey(secretKey) {
  const encoded = base64Url(secretKey);
  const groups = encoded.match(/.{1,6}/g) || [encoded];
  return `PVSK1-${groups.join(" ")}`;
}

function parseAccountSecretKey(value) {
  const trimmed = value.trim();
  const withoutPrefix = trimmed.startsWith("PVSK1-") ? trimmed.slice("PVSK1-".length) : trimmed;
  const compact = withoutPrefix.replace(/\s/g, "");
  if (!/^[A-Za-z0-9_-]+$/.test(compact)) {
    throw new Error("Account secret key format is invalid.");
  }
  const decoded = decodeAccountSecretKeyCandidate(compact);
  if (decoded) {
    return decoded;
  }

  const legacyCompact = withoutPrefix.replace(/[\s-]/g, "");
  if (legacyCompact !== compact) {
    const legacyDecoded = decodeAccountSecretKeyCandidate(legacyCompact);
    if (legacyDecoded) {
      return legacyDecoded;
    }
  }

  throw new Error("Account secret key must decode to 32 bytes.");
}

function assertAccountSecretKeyRoundTrip() {
  const keyWithBase64UrlSymbols = new Uint8Array(32).fill(251);
  const parsed = parseAccountSecretKey(displayAccountSecretKey(keyWithBase64UrlSymbols));
  assert(
    bytesEqual(parsed, keyWithBase64UrlSymbols),
    "Account secret key display must preserve base64url '-' and '_' symbols.",
  );
  wipe(keyWithBase64UrlSymbols, parsed);
}

function decodeAccountSecretKeyCandidate(value) {
  try {
    const decoded = base64UrlToBytes(value);
    return decoded.length === 32 ? decoded : null;
  } catch {
    return null;
  }
}

function stableJson(value) {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map((entry) => stableJson(entry)).join(",")}]`;
  }
  const keys = Object.keys(value).sort();
  return `{${keys.map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`).join(",")}}`;
}

function canonicalBytes(value) {
  return textBytes(stableJson(value));
}

function normalizeLoginHandle(value) {
  return value.trim().replace(/[A-Z]/g, (char) => char.toLowerCase());
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function testHeadHash(fillByte) {
  return base64Url(new Uint8Array(32).fill(fillByte));
}

function isVaultHeadHash(value) {
  try {
    return typeof value === "string" && base64UrlToBytes(value).length === 32;
  } catch {
    return false;
  }
}

function isValidHeadSeq(value) {
  return Number.isSafeInteger(value) && value >= 0;
}

function assertCheckpointCompatibleWithServerHead(checkpoint, serverHeadSeq, serverHeadHash) {
  if (!checkpoint) {
    return;
  }
  assert(isValidHeadSeq(checkpoint.headSeq), "Stored checkpoint sequence must be valid.");
  assert(isVaultHeadHash(checkpoint.headHash), "Stored checkpoint head hash must be valid.");
  assert(isValidHeadSeq(serverHeadSeq), "Server checkpoint sequence must be valid.");
  assert(isVaultHeadHash(serverHeadHash), "Server checkpoint head hash must be valid.");
  if (checkpoint.headSeq > serverHeadSeq) {
    throw new Error("Possible vault rollback detected: this browser has seen a newer vault head than the server returned.");
  }
  if (checkpoint.headSeq === serverHeadSeq && checkpoint.headHash !== serverHeadHash) {
    throw new Error("Possible vault fork detected: server head differs from this browser's stored checkpoint.");
  }
}

function markCheckpointProgress(vault) {
  const checkpoint = vault.storedCheckpoint;
  if (!checkpoint || vault.storedCheckpointVerified) {
    return;
  }
  if (vault.headSeq < checkpoint.headSeq) {
    return;
  }
  if (vault.headSeq === checkpoint.headSeq && vault.headHash === checkpoint.headHash) {
    vault.storedCheckpointVerified = true;
    return;
  }
  throw new Error("Possible vault rollback or fork detected: server chain did not match this browser's stored checkpoint.");
}

function assertStoredCheckpointVerified(vault) {
  if (vault.storedCheckpoint && !vault.storedCheckpointVerified) {
    throw new Error("Possible vault rollback detected: sync did not reach this browser's stored checkpoint.");
  }
}

function checkpointRecord(vault) {
  assert(isValidHeadSeq(vault.headSeq), "Checkpoint sequence must be valid.");
  assert(isVaultHeadHash(vault.headHash), "Checkpoint head hash must be valid.");
  return {
    version: VAULT_CHECKPOINT_VERSION,
    vault_id: vault.vaultId,
    head_seq: vault.headSeq,
    head_hash: vault.headHash,
  };
}

function latestVaultCheckpoint(checkpoints) {
  let latest = null;
  const hashesBySeq = new Map();
  for (const checkpoint of checkpoints) {
    const existingHash = hashesBySeq.get(checkpoint.headSeq);
    if (existingHash && existingHash !== checkpoint.headHash) {
      throw new Error("Stored vault checkpoints contain conflicting heads at the same sequence.");
    }
    hashesBySeq.set(checkpoint.headSeq, checkpoint.headHash);
    if (!latest || checkpoint.headSeq > latest.headSeq) {
      latest = checkpoint;
    }
  }
  return latest;
}

function assertCheckpointWriteIsMonotonic(existingCheckpoint, nextHeadSeq, nextHeadHash) {
  if (!existingCheckpoint) {
    return;
  }
  if (existingCheckpoint.headSeq > nextHeadSeq) {
    throw new Error("Refusing to overwrite a newer stored vault checkpoint.");
  }
  if (existingCheckpoint.headSeq === nextHeadSeq && existingCheckpoint.headHash !== nextHeadHash) {
    throw new Error("Refusing to overwrite a stored vault checkpoint with a forked head.");
  }
}

function assertRejectsSyncCheckpoint(label, fn) {
  try {
    fn();
  } catch {
    return;
  }
  throw new Error(`${label} unexpectedly succeeded.`);
}

function assertVaultCheckpointGuards() {
  const genesis = testHeadHash(0);
  const head1 = testHeadHash(1);
  const head2 = testHeadHash(2);
  const head3 = testHeadHash(3);
  const forkHead1 = testHeadHash(11);
  const forkHead2 = testHeadHash(12);
  const vault = {
    vaultId: "vault-checkpoint-test",
    headSeq: 0,
    headHash: genesis,
    storedCheckpoint: {
      vaultId: "vault-checkpoint-test",
      headSeq: 1,
      headHash: head1,
    },
    storedCheckpointVerified: false,
  };

  assertCheckpointCompatibleWithServerHead(vault.storedCheckpoint, 2, head2);
  markCheckpointProgress(vault);
  assert(!vault.storedCheckpointVerified, "Checkpoint must not verify before its sequence is reached.");
  vault.headSeq = 1;
  vault.headHash = head1;
  markCheckpointProgress(vault);
  assert(vault.storedCheckpointVerified, "Checkpoint must verify at the matching stored head.");
  vault.headSeq = 2;
  vault.headHash = head2;
  assertStoredCheckpointVerified(vault);
  const persisted = checkpointRecord(vault);
  assert(persisted.version === VAULT_CHECKPOINT_VERSION, "Checkpoint version must be persisted.");
  assert(persisted.head_seq === 2 && persisted.head_hash === head2, "Checkpoint must persist the latest head.");
  assert(latestVaultCheckpoint([{ headSeq: 1, headHash: head1 }, { headSeq: 2, headHash: head2 }]).headHash === head2, "Latest checkpoint must win.");
  assertRejectsSyncCheckpoint("same-sequence stored fork", () =>
    latestVaultCheckpoint([{ headSeq: 1, headHash: head1 }, { headSeq: 1, headHash: forkHead1 }]),
  );
  assertCheckpointWriteIsMonotonic({ headSeq: 1, headHash: head1 }, 2, head2);
  assertRejectsSyncCheckpoint("stale checkpoint overwrite", () =>
    assertCheckpointWriteIsMonotonic({ headSeq: 2, headHash: head2 }, 1, head1),
  );
  assertRejectsSyncCheckpoint("fork checkpoint overwrite", () =>
    assertCheckpointWriteIsMonotonic({ headSeq: 2, headHash: head2 }, 2, forkHead2),
  );

  assertRejectsSyncCheckpoint("newer local checkpoint", () =>
    assertCheckpointCompatibleWithServerHead({ headSeq: 3, headHash: head3 }, 2, head2),
  );
  assertRejectsSyncCheckpoint("same sequence fork", () =>
    assertCheckpointCompatibleWithServerHead({ headSeq: 2, headHash: forkHead2 }, 2, head2),
  );
  assertRejectsSyncCheckpoint("mismatched checkpoint chain", () =>
    markCheckpointProgress({
      headSeq: 1,
      headHash: forkHead1,
      storedCheckpoint: { headSeq: 1, headHash: head1 },
      storedCheckpointVerified: false,
    }),
  );
  assertRejectsSyncCheckpoint("unreached checkpoint", () =>
    assertStoredCheckpointVerified({
      headSeq: 0,
      headHash: genesis,
      storedCheckpoint: { headSeq: 1, headHash: head1 },
      storedCheckpointVerified: false,
    }),
  );
  assertRejectsSyncCheckpoint("invalid checkpoint hash", () =>
    assertCheckpointCompatibleWithServerHead({ headSeq: 1, headHash: "head-1" }, 1, head1),
  );
}

function assertNoStore(response, label) {
  const cacheControl = response.headers.get("cache-control") || "";
  assert(cacheControl.toLowerCase().includes("no-store"), `${label} must set Cache-Control: no-store.`);
}

function splitSetCookieHeader(value) {
  if (!value) {
    return [];
  }
  const parts = [];
  let start = 0;
  for (let index = 0; index < value.length; index += 1) {
    if (value[index] === "," && /^\s*[A-Za-z0-9_.-]+=/.test(value.slice(index + 1))) {
      parts.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }
  parts.push(value.slice(start).trim());
  return parts.filter(Boolean);
}

function getSetCookieHeaders(response) {
  if (typeof response.headers.getSetCookie === "function") {
    return response.headers.getSetCookie();
  }
  return splitSetCookieHeader(response.headers.get("set-cookie"));
}

function assertSessionCookieFlags(response, label) {
  const sessionCookie = getSetCookieHeaders(response).find((cookie) =>
    cookie.startsWith("__Host-pv_session="),
  );
  assert(sessionCookie, `${label} must set __Host-pv_session.`);
  const attrs = sessionCookie
    .split(";")
    .slice(1)
    .map((entry) => entry.trim().toLowerCase());
  assert(attrs.includes("secure"), `${label} session cookie must be Secure.`);
  assert(attrs.includes("httponly"), `${label} session cookie must be HttpOnly.`);
  assert(attrs.includes("samesite=strict"), `${label} session cookie must be SameSite=Strict.`);
  assert(attrs.includes("path=/"), `${label} session cookie must use Path=/.`);
  assert(!attrs.some((attr) => attr.startsWith("domain=")), `${label} session cookie must not set Domain.`);
}

class CookieJar {
  constructor(name) {
    this.name = name;
    this.cookies = new Map();
  }

  header() {
    return [...this.cookies.values()].join("; ");
  }

  absorb(response) {
    for (const cookie of getSetCookieHeaders(response)) {
      const [pair, ...attributes] = cookie.split(";").map((part) => part.trim());
      const separator = pair.indexOf("=");
      if (separator < 1) {
        continue;
      }
      const name = pair.slice(0, separator);
      const value = pair.slice(separator + 1);
      const clear = value === "" || attributes.some((attr) => attr.toLowerCase() === "max-age=0");
      if (clear) {
        this.cookies.delete(name);
      } else {
        this.cookies.set(name, `${name}=${value}`);
      }
    }
  }
}

async function requestJson(config, path, options = {}) {
  const response = await requestRaw(config, path, options);
  const text = await response.text();
  const body = text ? JSON.parse(text) : {};
  const expected = options.expectStatus || 200;
  if (response.status !== expected) {
    const code = body?.error?.code || "unknown";
    throw new Error(`${options.label || path} expected HTTP ${expected}, got ${response.status} (${code}).`);
  }
  return { response, body };
}

async function requestText(config, path, options = {}) {
  const response = await requestRaw(config, path, options);
  const text = await response.text();
  const expected = options.expectStatus || 200;
  if (response.status !== expected) {
    throw new Error(`${options.label || path} expected HTTP ${expected}, got ${response.status}.`);
  }
  return { response, text };
}

async function requestRaw(config, path, options = {}) {
  const url = new URL(path, options.baseUrl || config.baseUrl);
  const headers = {
    Accept: options.accept || "application/json",
    ...(options.headers || {}),
  };
  const method = options.method || "GET";
  const jar = options.jar;

  if (jar?.header()) {
    headers.Cookie = jar.header();
  }
  if (options.csrfToken) {
    headers["X-PV-CSRF"] = options.csrfToken;
  }
  let body;
  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
    headers["Sec-Fetch-Site"] = "same-origin";
    body = JSON.stringify(options.body);
  }

  const response = await fetch(url, {
    method,
    headers,
    body,
    signal: AbortSignal.timeout(config.timeoutMs),
  });
  jar?.absorb(response);
  if (options.expectNoStore) {
    assertNoStore(response, options.label || path);
  }
  return response;
}

async function getCsrf(config, jar) {
  const { body } = await requestJson(config, "/v1/csrf", {
    jar,
    expectNoStore: true,
    label: "csrf",
  });
  assert(typeof body.csrf_token === "string" && body.csrf_token.length > 20, "CSRF token is missing.");
  return body.csrf_token;
}

function validateKdfProfile(profile) {
  const id = String(profile?.id || "");
  const algorithm = String(profile?.algorithm || "").replace(/_/g, "-").toUpperCase();
  const hash = String(profile?.hash || "").toUpperCase();
  const iterations = Number(profile?.iterations);
  const algorithmOk =
    algorithm === "PBKDF2-HMAC-SHA-256" || (algorithm === "PBKDF2" && hash === "SHA-256");

  assert(
    id === KDF_PROFILE_ID && algorithmOk && hash === "SHA-256" && iterations === KDF_ITERATIONS,
    "Unsupported KDF profile.",
  );
  return { hash: "SHA-256", iterations };
}

function validateRegisterStart(response) {
  assert(response.auth_protocol === AUTH_PROTOCOL, "Unsupported registration auth protocol.");
  validateKdfProfile(response.kdf_profile);
  assert(response.auth_verifier_profile === SCRAM_PROFILE_ID, "Unsupported registration SCRAM profile.");
  assert(Number(response.auth_verifier_iterations) === SCRAM_ITERATIONS, "Unsupported SCRAM iteration count.");
}

function validateLoginStart(response) {
  assert(response.auth_protocol === AUTH_PROTOCOL, "Unsupported login auth protocol.");
  validateKdfProfile(response.kdf_profile);
  assert(response.auth_verifier_profile === SCRAM_PROFILE_ID, "Unsupported login SCRAM profile.");
  assert(Number(response.auth_verifier_iterations) === SCRAM_ITERATIONS, "Unsupported login verifier iterations.");
}

async function pbkdf2Bytes(secretBytes, saltBytes, iterations, hash, lengthBytes = 32) {
  const key = await subtle.importKey("raw", secretBytes, "PBKDF2", false, ["deriveBits"]);
  const bits = await subtle.deriveBits(
    {
      name: "PBKDF2",
      salt: saltBytes,
      iterations,
      hash,
    },
    key,
    lengthBytes * 8,
  );
  return new Uint8Array(bits);
}

async function hkdfBytes(inputKeyMaterial, saltBytes, infoText, lengthBytes = 32) {
  const key = await subtle.importKey("raw", inputKeyMaterial, "HKDF", false, ["deriveBits"]);
  const bits = await subtle.deriveBits(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt: saltBytes,
      info: textBytes(infoText),
    },
    key,
    lengthBytes * 8,
  );
  return new Uint8Array(bits);
}

async function hmacDigest(keyBytes, dataBytes, hash) {
  const key = await subtle.importKey("raw", keyBytes, { name: "HMAC", hash }, false, ["sign"]);
  return new Uint8Array(await subtle.sign("HMAC", key, dataBytes));
}

async function hmacSha256(keyBytes, dataBytes) {
  return hmacDigest(keyBytes, dataBytes, "SHA-256");
}

async function sha256(dataBytes) {
  return new Uint8Array(await subtle.digest("SHA-256", dataBytes));
}

async function deriveMasterSecret(masterPassword, accountSecretKey, accountSalt, kdfProfile) {
  const profile = validateKdfProfile(kdfProfile);
  const input = concatBytes(
    textBytes("password-vault/master-secret-input/v1"),
    new Uint8Array([0]),
    textBytes(masterPassword),
    new Uint8Array([0]),
    accountSecretKey,
  );
  const masterSecret = await pbkdf2Bytes(input, accountSalt, profile.iterations, profile.hash);
  wipe(input);
  return masterSecret;
}

async function deriveClientKeys(masterPassword, accountSecretKey, accountSalt, kdfProfile) {
  const masterSecret = await deriveMasterSecret(masterPassword, accountSecretKey, accountSalt, kdfProfile);
  const authSecret = await hkdfBytes(
    masterSecret,
    accountSalt,
    "password-vault/hkdf/auth-secret/v1",
  );
  const unlockKey = await hkdfBytes(
    masterSecret,
    accountSalt,
    "password-vault/hkdf/unlock-key/v1",
  );
  wipe(masterSecret);
  return { authSecret, unlockKey };
}

async function deriveScramVerifier(authSecret, salt, iterations) {
  const saltedPassword = await pbkdf2Bytes(authSecret, salt, Number(iterations), "SHA-256");
  const clientKey = await hmacSha256(saltedPassword, textBytes("Client Key"));
  const storedKey = await sha256(clientKey);
  const serverKey = await hmacSha256(saltedPassword, textBytes("Server Key"));
  wipe(saltedPassword, clientKey);
  return {
    auth_stored_key: base64Url(storedKey),
    auth_server_key: base64Url(serverKey),
  };
}

async function deriveScramClientProof(authSecret, salt, iterations, authMessage) {
  const saltedPassword = await pbkdf2Bytes(authSecret, salt, Number(iterations), "SHA-256");
  const clientKey = await hmacSha256(saltedPassword, textBytes("Client Key"));
  const storedKey = await sha256(clientKey);
  const clientSignature = await hmacSha256(storedKey, authMessage);
  const proof = new Uint8Array(clientKey.length);
  for (let index = 0; index < clientKey.length; index += 1) {
    proof[index] = clientKey[index] ^ clientSignature[index];
  }
  wipe(saltedPassword, clientKey, storedKey, clientSignature);
  return proof;
}

function pushTranscriptField(parts, name, value) {
  parts.push(`${name}=${textBytes(value).length}:${value}\n`);
}

function loginAuthMessage({
  challengeId,
  authProtocol,
  loginHandleNormalized,
  clientNonce,
  serverNonce,
  clientFinalWithoutProof,
}) {
  const parts = ["password-vault/login-auth-message/v1\n"];
  pushTranscriptField(parts, "challenge_id", challengeId);
  pushTranscriptField(parts, "auth_protocol", authProtocol);
  pushTranscriptField(parts, "login_handle_normalized", loginHandleNormalized);
  pushTranscriptField(parts, "client_nonce", base64Url(clientNonce));
  pushTranscriptField(parts, "server_nonce", base64Url(serverNonce));
  pushTranscriptField(parts, "client_final_without_proof", base64Url(clientFinalWithoutProof));
  return textBytes(parts.join(""));
}

async function aesGcmEncrypt(keyBytes, plaintext, aadBytes = null) {
  const key = await subtle.importKey("raw", keyBytes, { name: "AES-GCM" }, false, ["encrypt"]);
  const nonce = randomBytes(12);
  const algorithm = {
    name: "AES-GCM",
    iv: nonce,
    tagLength: 128,
  };
  if (aadBytes) {
    algorithm.additionalData = aadBytes;
  }
  const ciphertext = new Uint8Array(await subtle.encrypt(algorithm, key, jsonBytes(plaintext)));
  return {
    nonce: base64Url(nonce),
    ciphertext: base64Url(ciphertext),
  };
}

async function aesGcmDecrypt(keyBytes, envelope, aadBytes = null) {
  const key = await subtle.importKey("raw", keyBytes, { name: "AES-GCM" }, false, ["decrypt"]);
  const algorithm = {
    name: "AES-GCM",
    iv: base64UrlToBytes(envelope.nonce),
    tagLength: 128,
  };
  if (aadBytes) {
    algorithm.additionalData = aadBytes;
  }
  const plaintext = await subtle.decrypt(algorithm, key, base64UrlToBytes(envelope.ciphertext));
  return JSON.parse(decoder.decode(plaintext));
}

async function assertAesGcmDecryptRejects(label, fn) {
  try {
    await fn();
  } catch (error) {
    assert(
      error?.name === "OperationError",
      `${label} rejected with unexpected error: ${error?.name || error?.constructor?.name || "unknown"}.`,
    );
    return;
  }
  throw new Error(`${label} unexpectedly succeeded.`);
}

function tamperBase64Url(value) {
  const bytes = base64UrlToBytes(value);
  assert(bytes.length > 0, "Cannot tamper empty base64url value.");
  bytes[0] ^= 0x01;
  return base64Url(bytes);
}

async function assertAesGcmTamperRejected() {
  const keyBytes = randomBytes(32);
  const aadFields = {
    vaultId: randomUUID(),
    itemId: randomUUID(),
    revisionId: randomUUID(),
    operation: "create",
    baseRevisionSeq: 0,
    baseHeadSeq: 0,
    keyId: "vault-item-key-pbkdf2-sha256-browser-v1",
  };
  const aad = itemAad(aadFields);
  try {
    const plaintext = {
      version: "item-plaintext-v1",
      title: "Synthetic tamper self-test",
      username: "synthetic-user",
      password: "synthetic-only",
    };
    const envelope = await aesGcmEncrypt(keyBytes, plaintext, aad);
    const roundTrip = await aesGcmDecrypt(keyBytes, envelope, aad);
    assert(roundTrip.title === plaintext.title, "AES-GCM self-test round trip failed.");

    await assertAesGcmDecryptRejects("AES-GCM tampered ciphertext", () =>
      aesGcmDecrypt(keyBytes, { ...envelope, ciphertext: tamperBase64Url(envelope.ciphertext) }, aad),
    );
    await assertAesGcmDecryptRejects("AES-GCM tampered nonce", () =>
      aesGcmDecrypt(keyBytes, { ...envelope, nonce: tamperBase64Url(envelope.nonce) }, aad),
    );
    await assertAesGcmDecryptRejects("AES-GCM tampered authenticated metadata", () =>
      aesGcmDecrypt(keyBytes, envelope, itemAad({ ...aadFields, keyId: `${aadFields.keyId}-tampered` })),
    );
  } finally {
    wipe(keyBytes);
  }
}

async function buildEncryptedRegistrationPayload(startResponse, masterPassword, loginHandle) {
  const createdAt = new Date().toISOString();
  const accountSalt = base64UrlToBytes(startResponse.account_salt);
  const verifierSalt = base64UrlToBytes(startResponse.auth_verifier_salt);
  const accountSecretKey = randomBytes(32);
  const accountSecretDisplay = displayAccountSecretKey(accountSecretKey);
  const accountSecretForLogin = parseAccountSecretKey(accountSecretDisplay);
  assert(bytesEqual(accountSecretKey, accountSecretForLogin), "Account secret display round trip failed.");

  const accountKey = randomBytes(32);
  const vaultKey = randomBytes(32);
  const keyId = `browser-unlock-key-${randomUUID()}`;
  const vaultId = randomUUID();
  const { authSecret, unlockKey } = await deriveClientKeys(
    masterPassword,
    accountSecretKey,
    accountSalt,
    startResponse.kdf_profile,
  );
  const scramVerifier = await deriveScramVerifier(
    authSecret,
    verifierSalt,
    startResponse.auth_verifier_iterations,
  );

  const accountKeyset = {
    version: ACCOUNT_KEYSET_CRYPTO_VERSION,
    created_at: createdAt,
    login_handle: normalizeLoginHandle(loginHandle),
    account_key: base64Url(accountKey),
    account_secret_key: {
      format: "PVSK1-base64url-32",
      stored_with_server: false,
    },
    kdf_profile: startResponse.kdf_profile,
    auth_verifier_profile: startResponse.auth_verifier_profile,
  };
  const vaultKeyMetadata = {
    version: VAULT_KEY_WRAP_CRYPTO_VERSION,
    created_at: createdAt,
    vault_id: vaultId,
    crypto_profile_id: VAULT_CRYPTO_PROFILE_ID,
    wrapped_by: keyId,
    vault_key: base64Url(vaultKey),
  };

  const encryptedAccountKeyset = await aesGcmEncrypt(unlockKey, accountKeyset);
  const encryptedVaultKey = await aesGcmEncrypt(unlockKey, vaultKeyMetadata);
  wipe(accountSalt, verifierSalt, accountSecretKey, accountKey, vaultKey, authSecret, unlockKey);

  return {
    accountSecretKey: accountSecretForLogin,
    finishPayload: {
      registration_id: startResponse.registration_id,
      auth_protocol: AUTH_PROTOCOL,
      auth_stored_key: scramVerifier.auth_stored_key,
      auth_server_key: scramVerifier.auth_server_key,
      encrypted_account_keyset: {
        crypto_version: ACCOUNT_KEYSET_CRYPTO_VERSION,
        key_id: keyId,
        nonce: encryptedAccountKeyset.nonce,
        ciphertext: encryptedAccountKeyset.ciphertext,
      },
      initial_vault: {
        vault_id: vaultId,
        encrypted_vault_key: {
          crypto_version: VAULT_KEY_WRAP_CRYPTO_VERSION,
          key_id: keyId,
          nonce: encryptedVaultKey.nonce,
          ciphertext: encryptedVaultKey.ciphertext,
        },
      },
      device: syntheticDevice("register"),
    },
  };
}

function syntheticDevice(flow) {
  return {
    label: `Synthetic ${flow} client`,
    client_type: "browser",
    public_metadata: {
      platform_hint: "node",
      browser_brands: ["synthetic-node"],
      static_ui_flow: "browser_api_synthetic_v1",
    },
  };
}

async function buildLoginFinishPayload(startResponse, input, clientNonce) {
  const loginHandleNormalized = normalizeLoginHandle(input.loginHandle);
  const accountSalt = base64UrlToBytes(startResponse.account_salt);
  const authVerifierSalt = base64UrlToBytes(startResponse.auth_verifier_salt);
  const serverNonce = base64UrlToBytes(startResponse.server_nonce);
  const clientFinalWithoutProof = textBytes("c=biws");
  const { authSecret, unlockKey } = await deriveClientKeys(
    input.masterPassword,
    input.accountSecretKey,
    accountSalt,
    startResponse.kdf_profile,
  );
  const authMessage = loginAuthMessage({
    challengeId: startResponse.login_challenge_id,
    authProtocol: AUTH_PROTOCOL,
    loginHandleNormalized,
    clientNonce,
    serverNonce,
    clientFinalWithoutProof,
  });
  const clientProof = await deriveScramClientProof(
    authSecret,
    authVerifierSalt,
    startResponse.auth_verifier_iterations,
    authMessage,
  );
  const payload = {
    login_challenge_id: startResponse.login_challenge_id,
    auth_protocol: AUTH_PROTOCOL,
    client_nonce: base64Url(clientNonce),
    server_nonce: startResponse.server_nonce,
    client_final_without_proof: base64Url(clientFinalWithoutProof),
    client_proof: base64Url(clientProof),
    device: syntheticDevice("login"),
  };
  wipe(accountSalt, authVerifierSalt, serverNonce, authSecret, authMessage, clientFinalWithoutProof, clientProof);
  return { payload, unlockKey };
}

function decodeBase32NoPadding(value) {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
  const normalized = value.toUpperCase().replace(/=+$/g, "").replace(/\s+/g, "");
  let buffer = 0;
  let bits = 0;
  const output = [];
  for (const char of normalized) {
    const index = alphabet.indexOf(char);
    if (index < 0) {
      throw new Error("Invalid base32 TOTP secret.");
    }
    buffer = (buffer << 5) | index;
    bits += 5;
    if (bits >= 8) {
      output.push((buffer >> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return new Uint8Array(output);
}

function totpHashName(algorithm) {
  const normalized = String(algorithm || "SHA1").replace(/-/g, "").toUpperCase();
  if (normalized === "SHA1") {
    return "SHA-1";
  }
  if (normalized === "SHA256") {
    return "SHA-256";
  }
  if (normalized === "SHA512") {
    return "SHA-512";
  }
  throw new Error(`Unsupported TOTP algorithm: ${algorithm}`);
}

async function totpCode(seed, profile, stepOffset = 0) {
  const period = Number(profile.period || profile.period_seconds || 30);
  const digits = Number(profile.digits || 6);
  const currentStep = Math.floor(Math.floor(Date.now() / 1000) / period);
  const counter = BigInt(currentStep + stepOffset);
  const counterBytes = new Uint8Array(8);
  new DataView(counterBytes.buffer).setBigUint64(0, counter, false);
  const digest = await hmacDigest(seed, counterBytes, totpHashName(profile.algorithm));
  const offset = digest[digest.length - 1] & 0x0f;
  const binary =
    ((digest[offset] & 0x7f) << 24) |
    (digest[offset + 1] << 16) |
    (digest[offset + 2] << 8) |
    digest[offset + 3];
  const value = binary % 10 ** digits;
  return String(value).padStart(digits, "0");
}

async function decryptVaultKey(unlockKey, vault) {
  const wrapped = vault.encrypted_vault_key;
  assert(wrapped.crypto_version === VAULT_KEY_WRAP_CRYPTO_VERSION, "Unsupported vault key wrap.");
  const metadata = await aesGcmDecrypt(unlockKey, wrapped, null);
  assert(metadata.version === VAULT_KEY_WRAP_CRYPTO_VERSION, "Vault key metadata version mismatch.");
  assert(metadata.vault_id === vault.vault_id, "Vault key metadata vault_id mismatch.");
  assert(metadata.crypto_profile_id === VAULT_CRYPTO_PROFILE_ID, "Vault crypto profile mismatch.");
  const vaultKey = base64UrlToBytes(metadata.vault_key);
  assert(vaultKey.length === 32, "Vault key must be 32 bytes.");
  return {
    vaultKey,
    keyId: wrapped.key_id,
  };
}

async function deriveVaultIntegrityKey(vaultKey, vaultId) {
  return hkdfBytes(
    vaultKey,
    textBytes(`password-vault/vault-integrity-salt/v1:${vaultId}`),
    "password-vault/vault-integrity-key/v1",
  );
}

async function deriveItemRevisionKey(vaultKey, vaultId, itemId, revisionId) {
  return hkdfBytes(
    vaultKey,
    textBytes(`password-vault/item-revision-salt/v1:${vaultId}:${itemId}:${revisionId}`),
    "password-vault/item-revision-key/v1",
  );
}

async function vaultStateFromServerVault(serverVault, unlockKey) {
  const serverHeadSeq = Number(serverVault.head_seq);
  const serverHeadHash = String(serverVault.head_hash || "");
  const genesisHeadHash = String(serverVault.genesis_head_hash || "");
  assert(isValidHeadSeq(serverHeadSeq) && isVaultHeadHash(serverHeadHash), "Server returned invalid vault head.");
  assert(
    serverHeadSeq === 0 || isVaultHeadHash(genesisHeadHash),
    "Server did not return the genesis vault head required for checkpoint replay.",
  );
  const { vaultKey, keyId } = await decryptVaultKey(unlockKey, serverVault);
  const integrityKey = await deriveVaultIntegrityKey(vaultKey, serverVault.vault_id);
  return {
    vaultId: serverVault.vault_id,
    keyId,
    vaultKey,
    integrityKey,
    headSeq: 0,
    headHash: genesisHeadHash || serverHeadHash,
    serverHeadSeq,
    serverHeadHash,
    items: new Map(),
  };
}

function itemAad({ vaultId, itemId, revisionId, operation, baseRevisionSeq, baseHeadSeq, keyId }) {
  return canonicalBytes({
    record_type: "vault-item-revision",
    crypto_version: ITEM_ENVELOPE_CRYPTO_VERSION,
    aead: ITEM_ENVELOPE_AEAD,
    vault_id: vaultId,
    item_id: itemId,
    revision_id: revisionId,
    operation,
    base_revision_seq: baseRevisionSeq,
    base_head_seq: baseHeadSeq,
    key_id: keyId,
  });
}

function changeMacPayload({
  operation,
  vaultId,
  itemId,
  revisionId,
  revisionSeq,
  headSeq,
  baseRevisionSeq,
  baseHeadSeq,
  baseHeadHash,
  previousHeadHash,
  envelopeHash,
  keyId,
}) {
  return {
    version: "password-vault/change-mac/v1",
    operation,
    vault_id: vaultId,
    item_id: itemId,
    revision_id: revisionId,
    revision_seq: revisionSeq,
    head_seq: headSeq,
    base_revision_seq: baseRevisionSeq,
    base_head_seq: baseHeadSeq,
    base_head_hash: baseHeadHash,
    previous_head_hash: previousHeadHash,
    envelope_hash: envelopeHash,
    key_id: keyId,
    crypto_version: ITEM_ENVELOPE_CRYPTO_VERSION,
  };
}

function headHashPayload({ vaultId, headSeq, previousHeadHash, changeMac }) {
  return {
    version: "password-vault/head-hash/v1",
    vault_id: vaultId,
    head_seq: headSeq,
    previous_head_hash: previousHeadHash,
    change_mac: changeMac,
  };
}

async function computeChangeMac(vault, payload) {
  return base64Url(await hmacSha256(vault.integrityKey, canonicalBytes(payload)));
}

async function computeHeadHash(vault, payload) {
  return base64Url(await hmacSha256(vault.integrityKey, canonicalBytes(payload)));
}

async function envelopeHash(envelope) {
  return base64Url(await sha256(canonicalBytes(envelope)));
}

async function buildEncryptedItemChange(vault, { operation, plaintext, itemId, baseRevisionSeq }) {
  const revisionId = randomUUID();
  const revisionSeq = baseRevisionSeq + 1;
  const headSeq = vault.headSeq + 1;
  const baseHeadSeq = vault.headSeq;
  const baseHeadHash = vault.headHash;
  const keyId = `vault-item-key-${VAULT_CRYPTO_PROFILE_ID}`;
  const contentKey = await deriveItemRevisionKey(vault.vaultKey, vault.vaultId, itemId, revisionId);
  const aad = itemAad({
    vaultId: vault.vaultId,
    itemId,
    revisionId,
    operation,
    baseRevisionSeq,
    baseHeadSeq,
    keyId,
  });
  const encrypted = await aesGcmEncrypt(contentKey, plaintext, aad);
  wipe(contentKey);

  const encryptedItemEnvelope = {
    crypto_version: ITEM_ENVELOPE_CRYPTO_VERSION,
    key_id: keyId,
    aead: ITEM_ENVELOPE_AEAD,
    nonce: encrypted.nonce,
    ciphertext: encrypted.ciphertext,
  };
  const encryptedEnvelopeHash = await envelopeHash(encryptedItemEnvelope);
  const changeMac = await computeChangeMac(
    vault,
    changeMacPayload({
      operation,
      vaultId: vault.vaultId,
      itemId,
      revisionId,
      revisionSeq,
      headSeq,
      baseRevisionSeq,
      baseHeadSeq,
      baseHeadHash,
      previousHeadHash: baseHeadHash,
      envelopeHash: encryptedEnvelopeHash,
      keyId,
    }),
  );
  const newHeadHash = await computeHeadHash(
    vault,
    headHashPayload({
      vaultId: vault.vaultId,
      headSeq,
      previousHeadHash: baseHeadHash,
      changeMac,
    }),
  );

  return {
    itemId,
    revisionId,
    revisionSeq,
    headSeq,
    baseRevisionSeq,
    baseHeadSeq,
    baseHeadHash,
    previousHeadHash: baseHeadHash,
    newHeadHash,
    changeMac,
    envelopeHash: encryptedEnvelopeHash,
    encryptedItemEnvelope,
  };
}

async function verifyAndApplyChange(vault, change) {
  const operation = String(change.operation);
  const itemId = String(change.item_id);
  const revisionId = String(change.revision_id);
  const revisionSeq = Number(change.revision_seq);
  const headSeq = Number(change.head_seq);
  const baseRevisionSeq = Number(change.base_revision_seq);
  const baseHeadSeq = Number(change.base_head_seq);
  const baseHeadHash = String(change.base_head_hash);
  const previousHeadHash = String(change.previous_head_hash);
  const envelope = change.encrypted_item_envelope;
  const keyId = String(envelope?.key_id || "");

  assert(baseHeadSeq === vault.headSeq, "Sync chain base_head_seq mismatch.");
  assert(baseHeadHash === vault.headHash, "Sync chain base_head_hash mismatch.");
  assert(previousHeadHash === vault.headHash, "Sync chain previous_head_hash mismatch.");
  assert(envelope?.crypto_version === ITEM_ENVELOPE_CRYPTO_VERSION, "Unsupported item envelope version.");
  assert(envelope?.aead === ITEM_ENVELOPE_AEAD, "Unsupported item envelope AEAD.");

  const actualEnvelopeHash = await envelopeHash(envelope);
  assert(actualEnvelopeHash === change.envelope_hash, "Encrypted item envelope hash mismatch.");
  const expectedChangeMac = await computeChangeMac(
    vault,
    changeMacPayload({
      operation,
      vaultId: vault.vaultId,
      itemId,
      revisionId,
      revisionSeq,
      headSeq,
      baseRevisionSeq,
      baseHeadSeq,
      baseHeadHash,
      previousHeadHash,
      envelopeHash: actualEnvelopeHash,
      keyId,
    }),
  );
  assert(expectedChangeMac === change.change_mac, "Vault change MAC mismatch.");
  const expectedHeadHash = await computeHeadHash(
    vault,
    headHashPayload({
      vaultId: vault.vaultId,
      headSeq,
      previousHeadHash,
      changeMac: expectedChangeMac,
    }),
  );
  assert(expectedHeadHash === change.head_hash, "Vault head hash mismatch.");

  const contentKey = await deriveItemRevisionKey(vault.vaultKey, vault.vaultId, itemId, revisionId);
  const plaintext = await aesGcmDecrypt(
    contentKey,
    envelope,
    itemAad({
      vaultId: vault.vaultId,
      itemId,
      revisionId,
      operation,
      baseRevisionSeq,
      baseHeadSeq,
      keyId,
    }),
  );
  wipe(contentKey);

  if (operation === "delete") {
    vault.items.delete(itemId);
  } else {
    vault.items.set(itemId, {
      itemId,
      revisionId,
      revisionSeq,
      headSeq,
      fields: plaintext,
    });
  }
  vault.headSeq = headSeq;
  vault.headHash = change.head_hash;
}

async function syncVault(config, jar, vault) {
  let hasMore = false;
  do {
    const query = new URLSearchParams({
      from_head_seq: String(vault.headSeq),
      from_head_hash: vault.headHash,
    });
    const { body } = await requestJson(config, `/v1/vaults/${vault.vaultId}/sync?${query.toString()}`, {
      jar,
      expectNoStore: true,
      label: "vault sync",
    });
    for (const change of body.changes || []) {
      await verifyAndApplyChange(vault, change);
    }
    hasMore = Boolean(body.has_more);
    if (!hasMore) {
      assert(Number(body.to_head?.seq) === vault.headSeq, "Server sync head seq mismatch.");
      assert(String(body.to_head?.hash || "") === vault.headHash, "Server sync head hash mismatch.");
    }
  } while (hasMore);
}

async function assertMetrics(config) {
  await requestText(config, "/metrics", {
    accept: "text/plain",
    expectStatus: 404,
    expectNoStore: false,
    label: "public API metrics denial",
  });

  const { text } = await requestText(config, "/metrics", {
    accept: "text/plain",
    baseUrl: config.metricsBaseUrl,
    expectNoStore: false,
    label: "metrics",
  });
  const requiredSeries = [
    ["password_vault_registration_events_total", { event: "start", outcome: "issued" }],
    ["password_vault_registration_events_total", { event: "finish", outcome: "success" }],
    ["password_vault_accounts_created_total", { outcome: "success" }],
    ["password_vault_login_starts_total", { outcome: "issued" }],
    ["password_vault_login_attempts_total", { outcome: "success", failure_class: "none" }],
    ["password_vault_mfa_events_total", { event: "totp_enrollment", outcome: "started" }],
    ["password_vault_mfa_events_total", { event: "totp_enrollment", outcome: "confirmed" }],
    ["password_vault_mfa_events_total", { event: "totp_login", outcome: "challenge_issued" }],
    ["password_vault_mfa_events_total", { event: "totp_login", outcome: "verified" }],
    ["password_vault_mfa_events_total", { event: "recovery_code_login", outcome: "verified" }],
    ["password_vault_session_events_total", { event: "created", outcome: "mfa_enrollment_required" }],
    ["password_vault_session_events_total", { event: "upgraded", outcome: "mfa_verified" }],
    ["password_vault_session_events_total", { event: "created", outcome: "mfa_verified" }],
    ["password_vault_session_events_total", { event: "created", outcome: "mfa_recovery" }],
    ["password_vault_vault_item_changes_total", { operation: "create", outcome: "success" }],
    ["password_vault_sync_requests_total", { outcome: "success", page: "complete" }],
    ["password_vault_db_pool_connections", { state: "max" }],
  ];
  for (const [name, labels] of requiredSeries) {
    const value = metricValue(text, name, labels);
    assert(value > 0, `Expected metric ${name}${formatLabels(labels)} to be greater than zero.`);
  }
}

function metricValue(metricsText, name, labels) {
  let total = 0;
  for (const line of metricsText.split("\n")) {
    if (!line.startsWith(name)) {
      continue;
    }
    const parsed = parseMetricLine(line);
    if (!parsed || parsed.name !== name) {
      continue;
    }
    if (Object.entries(labels).every(([key, value]) => parsed.labels[key] === value)) {
      total += parsed.value;
    }
  }
  return total;
}

function parseMetricLine(line) {
  const match = line.match(/^([a-zA-Z_:][a-zA-Z0-9_:]*)(?:\{([^}]*)\})?\s+(-?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?|[+-]?Inf|NaN)$/);
  if (!match) {
    return null;
  }
  return {
    name: match[1],
    labels: parseMetricLabels(match[2] || ""),
    value: Number(match[3]),
  };
}

function parseMetricLabels(labelText) {
  const labels = {};
  const labelPattern = /([a-zA-Z_][a-zA-Z0-9_]*)="((?:\\.|[^"\\])*)"/g;
  let match;
  while ((match = labelPattern.exec(labelText)) !== null) {
    labels[match[1]] = match[2].replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  }
  return labels;
}

function formatLabels(labels) {
  return `{${Object.entries(labels)
    .map(([key, value]) => `${key}="${value}"`)
    .join(",")}}`;
}

function itemCreateBody(itemId, change) {
  return {
    item_id: itemId,
    revision_id: change.revisionId,
    base_head_seq: change.baseHeadSeq,
    base_head_hash: change.baseHeadHash,
    new_head_hash: change.newHeadHash,
    change_mac: change.changeMac,
    envelope_hash: change.envelopeHash,
    encrypted_item_envelope: change.encryptedItemEnvelope,
  };
}

function createdChangeForLocalApply(itemId, change) {
  return {
    item_id: itemId,
    revision_id: change.revisionId,
    operation: "create",
    revision_seq: change.revisionSeq,
    head_seq: change.headSeq,
    previous_head_hash: change.previousHeadHash,
    head_hash: change.newHeadHash,
    base_revision_seq: change.baseRevisionSeq,
    base_head_seq: change.baseHeadSeq,
    base_head_hash: change.baseHeadHash,
    change_mac: change.changeMac,
    envelope_hash: change.envelopeHash,
    encrypted_item_envelope: change.encryptedItemEnvelope,
  };
}

async function main() {
  const config = loadConfig();
  let accountSecretKey;
  let loginUnlockKey;
  let totpSeed;
  let recoveryTotpSeed;
  let recoveryCode;
  let writerVault;
  let readerVault;

  try {
    assertAccountSecretKeyRoundTrip();
    assertVaultCheckpointGuards();
    await assertAesGcmTamperRejected();
    if (process.env.SYNTHETIC_SELF_TEST_ONLY === "true") {
      console.log(JSON.stringify({ status: "ok", self_test: "browser_crypto_and_checkpoint_guards" }));
      return;
    }
    logStep("checking health and readiness");
    await requestJson(config, "/healthz", { expectNoStore: false, label: "healthz" });
    await requestJson(config, "/readyz", { expectNoStore: false, label: "readyz" });

    const registrationJar = new CookieJar("registration");
    logStep("starting registration");
    const { body: registerStart } = await requestJson(config, "/v1/auth/register/start", {
      method: "POST",
      body: {
        login_handle: config.loginHandle,
        auth_protocol: AUTH_PROTOCOL,
      },
      expectNoStore: true,
      label: "register start",
    });
    validateRegisterStart(registerStart);

    logStep("deriving browser registration keys");
    const registration = await buildEncryptedRegistrationPayload(
      registerStart,
      config.masterPassword,
      config.loginHandle,
    );
    accountSecretKey = registration.accountSecretKey;

    logStep("finishing registration");
    const { response: registerFinishResponse, body: registerFinish } = await requestJson(
      config,
      "/v1/auth/register/finish",
      {
        method: "POST",
        jar: registrationJar,
        body: registration.finishPayload,
        expectStatus: 201,
        expectNoStore: true,
        label: "register finish",
      },
    );
    assertSessionCookieFlags(registerFinishResponse, "register finish");
    assert(registerFinish.session?.state === "mfa_enrollment_required", "Registration must create setup session.");
    assert(registerFinish.session?.vault_access === false, "Setup session must not have vault access.");

    logStep("enrolling TOTP");
    const enrollStartCsrf = await getCsrf(config, registrationJar);
    const { body: enrollStart } = await requestJson(config, "/v1/mfa/totp/enroll/start", {
      method: "POST",
      jar: registrationJar,
      csrfToken: enrollStartCsrf,
      body: {},
      expectNoStore: true,
      label: "totp enroll start",
    });
    assert(typeof enrollStart.factor_id === "string", "TOTP factor id is missing.");
    assert(enrollStart.totp_profile?.algorithm === "SHA1", "Unexpected TOTP algorithm.");
    assert(Number(enrollStart.totp_profile?.digits) === 6, "Unexpected TOTP digit count.");
    assert(Number(enrollStart.totp_profile?.period) === 30, "Unexpected TOTP period.");
    totpSeed = decodeBase32NoPadding(enrollStart.manual_secret);

    logStep("confirming TOTP enrollment");
    const enrollConfirmCsrf = await getCsrf(config, registrationJar);
    const enrollCode = await totpCode(totpSeed, enrollStart.totp_profile, 0);
    const { response: enrollConfirmResponse, body: enrollConfirm } = await requestJson(
      config,
      "/v1/mfa/totp/enroll/confirm",
      {
        method: "POST",
        jar: registrationJar,
        csrfToken: enrollConfirmCsrf,
        body: {
          factor_id: enrollStart.factor_id,
          code: enrollCode,
        },
        expectNoStore: true,
        label: "totp enroll confirm",
      },
    );
    assertSessionCookieFlags(enrollConfirmResponse, "totp enroll confirm");
    assert(enrollConfirm.session?.state === "mfa_verified", "TOTP confirm must upgrade session.");
    assert(enrollConfirm.session?.vault_access === true, "TOTP confirm must grant vault access.");
    assert(Array.isArray(enrollConfirm.recovery_codes), "TOTP confirm must return recovery codes once.");
    assert(enrollConfirm.recovery_codes.length === 10, "TOTP confirm must return 10 recovery codes.");
    recoveryCode = enrollConfirm.recovery_codes[0];

    const { body: verifiedSetupSession } = await requestJson(config, "/v1/session", {
      jar: registrationJar,
      expectNoStore: true,
      label: "verified setup session",
    });
    assert(verifiedSetupSession.authenticated === true, "Verified setup session must be authenticated.");
    assert(verifiedSetupSession.vault_access === true, "Verified setup session must have vault access.");

    logStep("logging out setup session to simulate return login");
    const logoutCsrf = await getCsrf(config, registrationJar);
    await requestText(config, "/v1/auth/logout", {
      method: "POST",
      jar: registrationJar,
      csrfToken: logoutCsrf,
      body: {},
      expectStatus: 204,
      expectNoStore: true,
      label: "logout",
    });
    assert(!registrationJar.header(), "Logout must clear the registration session cookie.");

    const loginJar = new CookieJar("return-login");
    const clientNonce = randomBytes(32);
    logStep("starting return login");
    const { body: loginStart } = await requestJson(config, "/v1/auth/login/start", {
      method: "POST",
      body: {
        login_handle: config.loginHandle,
        auth_protocol: AUTH_PROTOCOL,
        client_nonce: base64Url(clientNonce),
      },
      expectNoStore: true,
      label: "login start",
    });
    validateLoginStart(loginStart);

    logStep("finishing return login proof");
    const { payload: loginFinishPayload, unlockKey } = await buildLoginFinishPayload(
      loginStart,
      {
        loginHandle: config.loginHandle,
        masterPassword: config.masterPassword,
        accountSecretKey,
      },
      clientNonce,
    );
    loginUnlockKey = unlockKey;
    wipe(clientNonce);
    const { body: loginFinish } = await requestJson(config, "/v1/auth/login/finish", {
      method: "POST",
      jar: loginJar,
      body: loginFinishPayload,
      expectNoStore: true,
      label: "login finish",
    });
    assert(loginFinish.result === "mfa_required", "Return login must require TOTP.");
    assert(typeof loginFinish.mfa_challenge_id === "string", "MFA challenge id is missing.");
    assert((loginFinish.available_methods || []).includes("totp"), "TOTP must be an available MFA method.");

    logStep("verifying return-login TOTP");
    // The server accepts previous/current/next TOTP steps and rejects reused accepted steps.
    // Use the next step to avoid a same-window replay after enrollment without sleeping in CI.
    const loginTotpCode = await totpCode(totpSeed, enrollStart.totp_profile, 1);
    const { response: loginTotpResponse, body: loginTotp } = await requestJson(
      config,
      "/v1/auth/mfa/totp/verify",
      {
        method: "POST",
        jar: loginJar,
        body: {
          mfa_challenge_id: loginFinish.mfa_challenge_id,
          code: loginTotpCode,
        },
        expectNoStore: true,
        label: "login totp verify",
      },
    );
    assertSessionCookieFlags(loginTotpResponse, "login totp verify");
    assert(loginTotp.session?.state === "mfa_verified", "Login TOTP must create verified session.");
    assert(loginTotp.session?.vault_access === true, "Login TOTP session must have vault access.");

    logStep("unlocking vault metadata");
    const { body: vaultList } = await requestJson(config, "/v1/vaults", {
      jar: loginJar,
      expectNoStore: true,
      label: "vault list",
    });
    assert(Array.isArray(vaultList.vaults) && vaultList.vaults.length === 1, "Expected one personal vault.");
    writerVault = await vaultStateFromServerVault(vaultList.vaults[0], loginUnlockKey);
    await syncVault(config, loginJar, writerVault);
    assert(writerVault.items.size === 0, "New vault should start empty.");

    logStep("creating encrypted item and checking CSRF guard");
    const itemId = randomUUID();
    const plaintext = {
      version: "item-plaintext-v1",
      title: "Synthetic login",
      url: "https://example.invalid/login",
      username: "synthetic-user",
      password: `synthetic-${base64Url(randomBytes(18))}`,
      notes: "Synthetic CI item. Contains no real secret.",
      updated_at: new Date().toISOString(),
    };
    const createChange = await buildEncryptedItemChange(writerVault, {
      operation: "create",
      plaintext,
      itemId,
      baseRevisionSeq: 0,
    });
    const createBody = itemCreateBody(itemId, createChange);
    const { body: missingCsrfBody } = await requestJson(config, `/v1/vaults/${writerVault.vaultId}/items`, {
      method: "POST",
      jar: loginJar,
      body: createBody,
      expectStatus: 403,
      expectNoStore: true,
      label: "item create missing csrf",
    });
    assert(missingCsrfBody.error?.code === "csrf_required", "Item create without CSRF must fail closed.");

    const createCsrf = await getCsrf(config, loginJar);
    const { body: createResponse } = await requestJson(config, `/v1/vaults/${writerVault.vaultId}/items`, {
      method: "POST",
      jar: loginJar,
      csrfToken: createCsrf,
      body: createBody,
      expectStatus: 201,
      expectNoStore: true,
      label: "item create",
    });
    assert(createResponse.head_hash === createChange.newHeadHash, "Create response head hash mismatch.");
    assert(Number(createResponse.head_seq) === createChange.headSeq, "Create response head seq mismatch.");
    await verifyAndApplyChange(writerVault, createdChangeForLocalApply(itemId, createChange));

    logStep("syncing from genesis and decrypting item");
    const { body: vaultListAfterCreate } = await requestJson(config, "/v1/vaults", {
      jar: loginJar,
      expectNoStore: true,
      label: "vault list after create",
    });
    readerVault = await vaultStateFromServerVault(vaultListAfterCreate.vaults[0], loginUnlockKey);
    await syncVault(config, loginJar, readerVault);
    const readItem = readerVault.items.get(itemId);
    assert(readItem, "Synced item is missing.");
    assert(readItem.fields.title === plaintext.title, "Synced item title mismatch.");
    assert(readItem.fields.url === plaintext.url, "Synced item URL mismatch.");
    assert(readItem.fields.username === plaintext.username, "Synced item username mismatch.");
    assert(readItem.fields.password === plaintext.password, "Synced item password mismatch.");
    assert(readItem.fields.notes === plaintext.notes, "Synced item notes mismatch.");

    logStep("checking recovery-code login and forced TOTP re-enrollment");
    const recoveryLogoutCsrf = await getCsrf(config, loginJar);
    await requestText(config, "/v1/auth/logout", {
      method: "POST",
      jar: loginJar,
      csrfToken: recoveryLogoutCsrf,
      body: {},
      expectStatus: 204,
      expectNoStore: true,
      label: "recovery prep logout",
    });

    const recoveryJar = new CookieJar("recovery-login");
    const recoveryClientNonce = randomBytes(32);
    const { body: recoveryLoginStart } = await requestJson(config, "/v1/auth/login/start", {
      method: "POST",
      body: {
        login_handle: config.loginHandle,
        auth_protocol: AUTH_PROTOCOL,
        client_nonce: base64Url(recoveryClientNonce),
      },
      expectNoStore: true,
      label: "recovery login start",
    });
    validateLoginStart(recoveryLoginStart);
    const { payload: recoveryLoginFinishPayload } = await buildLoginFinishPayload(
      recoveryLoginStart,
      {
        loginHandle: config.loginHandle,
        masterPassword: config.masterPassword,
        accountSecretKey,
      },
      recoveryClientNonce,
    );
    wipe(recoveryClientNonce);
    const { body: recoveryLoginFinish } = await requestJson(config, "/v1/auth/login/finish", {
      method: "POST",
      jar: recoveryJar,
      body: recoveryLoginFinishPayload,
      expectNoStore: true,
      label: "recovery login finish",
    });
    assert(recoveryLoginFinish.result === "mfa_required", "Recovery login must require MFA.");
    assert(
      (recoveryLoginFinish.available_methods || []).includes("recovery_code"),
      "Recovery code must be an available MFA method.",
    );
    const { response: recoveryVerifyResponse, body: recoveryVerify } = await requestJson(
      config,
      "/v1/auth/mfa/recovery-code/verify",
      {
        method: "POST",
        jar: recoveryJar,
        body: {
          mfa_challenge_id: recoveryLoginFinish.mfa_challenge_id,
          recovery_code: recoveryCode,
        },
        expectNoStore: true,
        label: "recovery code verify",
      },
    );
    assertSessionCookieFlags(recoveryVerifyResponse, "recovery code verify");
    assert(recoveryVerify.session?.state === "mfa_recovery", "Recovery code must create recovery session.");
    assert(recoveryVerify.session?.vault_access === false, "Recovery session must not have vault access.");
    assert(recoveryVerify.next_step === "reenroll_totp", "Recovery session must require TOTP re-enrollment.");
    const { body: recoveryVaults } = await requestJson(config, "/v1/vaults", {
      jar: recoveryJar,
      expectStatus: 403,
      expectNoStore: true,
      label: "recovery session vault denial",
    });
    assert(recoveryVaults.error?.code === "mfa_required", "Recovery session must not access vaults.");

    const recoveryEnrollStartCsrf = await getCsrf(config, recoveryJar);
    const { body: recoveryEnrollStart } = await requestJson(config, "/v1/mfa/totp/enroll/start", {
      method: "POST",
      jar: recoveryJar,
      csrfToken: recoveryEnrollStartCsrf,
      body: {},
      expectNoStore: true,
      label: "recovery totp enroll start",
    });
    recoveryTotpSeed = decodeBase32NoPadding(recoveryEnrollStart.manual_secret);
    const recoveryEnrollConfirmCsrf = await getCsrf(config, recoveryJar);
    const recoveryEnrollCode = await totpCode(recoveryTotpSeed, recoveryEnrollStart.totp_profile, 0);
    const { body: recoveryEnrollConfirm } = await requestJson(
      config,
      "/v1/mfa/totp/enroll/confirm",
      {
        method: "POST",
        jar: recoveryJar,
        csrfToken: recoveryEnrollConfirmCsrf,
        body: {
          factor_id: recoveryEnrollStart.factor_id,
          code: recoveryEnrollCode,
        },
        expectNoStore: true,
        label: "recovery totp enroll confirm",
      },
    );
    assert(
      recoveryEnrollConfirm.session?.state === "mfa_verified",
      "Recovery TOTP confirm must restore verified session.",
    );
    assert(
      recoveryEnrollConfirm.session?.vault_access === true,
      "Recovery TOTP confirm must restore vault access.",
    );

    if (config.checkMetrics) {
      logStep("checking product metrics");
      await assertMetrics(config);
    } else {
      logStep("skipping metrics check for non-local base URL");
    }

    console.log(
      JSON.stringify({
        status: "ok",
        journey: "register_totp_return_login_unlock_create_sync_read_recovery_reenroll",
        metrics_checked: config.checkMetrics,
        vault_head_seq: readerVault.headSeq,
        item_count: readerVault.items.size,
      }),
    );
  } finally {
    wipe(accountSecretKey, loginUnlockKey, totpSeed, recoveryTotpSeed, writerVault?.vaultKey, writerVault?.integrityKey);
    wipe(readerVault?.vaultKey, readerVault?.integrityKey);
  }
}

main().catch((error) => {
  console.error(`[synthetic] failed: ${error.message}`);
  process.exitCode = 1;
});
