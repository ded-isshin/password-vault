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

const encoder = new TextEncoder();
const decoder = new TextDecoder();

const state = {
  activeStep: "",
  csrfToken: "",
  factorId: "",
  loginMfaChallengeId: "",
  pendingFinishPayload: null,
  pendingRegisterSession: null,
  registrationFinished: false,
  accountSecretDisplay: "",
  pendingUnlockKey: null,
  vault: null,
  selectedItemId: "",
};

const elements = {
  showRegisterButton: document.querySelector("#showRegisterButton"),
  showLoginButton: document.querySelector("#showLoginButton"),
  registrationForm: document.querySelector("#registrationForm"),
  loginHandle: document.querySelector("#loginHandle"),
  masterPassword: document.querySelector("#masterPassword"),
  confirmPassword: document.querySelector("#confirmPassword"),
  registerButton: document.querySelector("#registerButton"),
  loginForm: document.querySelector("#loginForm"),
  returnLoginHandle: document.querySelector("#returnLoginHandle"),
  loginMasterPassword: document.querySelector("#loginMasterPassword"),
  accountSecretInput: document.querySelector("#accountSecretInput"),
  loginButton: document.querySelector("#loginButton"),
  statusMessage: document.querySelector("#statusMessage"),
  flowSteps: document.querySelector("#flowSteps"),
  accountSecretPanel: document.querySelector("#accountSecretPanel"),
  accountSecretKey: document.querySelector("#accountSecretKey"),
  copySecretButton: document.querySelector("#copySecretButton"),
  downloadSecretButton: document.querySelector("#downloadSecretButton"),
  secretSavedCheckbox: document.querySelector("#secretSavedCheckbox"),
  continueEnrollmentButton: document.querySelector("#continueEnrollmentButton"),
  totpPanel: document.querySelector("#totpPanel"),
  manualSecret: document.querySelector("#manualSecret"),
  otpauthUri: document.querySelector("#otpauthUri"),
  totpProfile: document.querySelector("#totpProfile"),
  factorId: document.querySelector("#factorId"),
  totpForm: document.querySelector("#totpForm"),
  totpCode: document.querySelector("#totpCode"),
  confirmTotpButton: document.querySelector("#confirmTotpButton"),
  loginMfaPanel: document.querySelector("#loginMfaPanel"),
  loginMfaForm: document.querySelector("#loginMfaForm"),
  loginTotpCode: document.querySelector("#loginTotpCode"),
  verifyLoginTotpButton: document.querySelector("#verifyLoginTotpButton"),
  recoveryPanel: document.querySelector("#recoveryPanel"),
  recoveryCodes: document.querySelector("#recoveryCodes"),
  sessionPanel: document.querySelector("#sessionPanel"),
  sessionList: document.querySelector("#sessionList"),
  vaultPanel: document.querySelector("#vaultPanel"),
  vaultStatus: document.querySelector("#vaultStatus"),
  syncVaultButton: document.querySelector("#syncVaultButton"),
  lockVaultButton: document.querySelector("#lockVaultButton"),
  vaultItemForm: document.querySelector("#vaultItemForm"),
  itemTitle: document.querySelector("#itemTitle"),
  itemUrl: document.querySelector("#itemUrl"),
  itemUsername: document.querySelector("#itemUsername"),
  itemPassword: document.querySelector("#itemPassword"),
  itemNotes: document.querySelector("#itemNotes"),
  saveItemButton: document.querySelector("#saveItemButton"),
  newItemButton: document.querySelector("#newItemButton"),
  deleteItemButton: document.querySelector("#deleteItemButton"),
  vaultItemList: document.querySelector("#vaultItemList"),
  healthStatus: document.querySelector("#healthStatus"),
  readyStatus: document.querySelector("#readyStatus"),
};

function setStatus(message, type = "") {
  elements.statusMessage.textContent = message;
  elements.statusMessage.className = `status-message ${type}`.trim();
}

function setRegistrationBusy(isBusy, label = "Create account") {
  elements.registerButton.disabled = isBusy;
  elements.loginHandle.disabled = isBusy;
  elements.masterPassword.disabled = isBusy;
  elements.confirmPassword.disabled = isBusy;
  elements.registerButton.textContent = isBusy ? "Working..." : label;
}

function setLoginBusy(isBusy, label = "Continue") {
  elements.loginButton.disabled = isBusy;
  elements.returnLoginHandle.disabled = isBusy;
  elements.loginMasterPassword.disabled = isBusy;
  elements.accountSecretInput.disabled = isBusy;
  elements.loginButton.textContent = isBusy ? "Working..." : label;
}

function setSecretContinueBusy(isBusy) {
  elements.continueEnrollmentButton.disabled = isBusy || !elements.secretSavedCheckbox.checked;
  elements.copySecretButton.disabled = isBusy;
  elements.downloadSecretButton.disabled = isBusy;
  elements.continueEnrollmentButton.textContent = isBusy ? "Working..." : "Continue enrollment";
}

function setTotpBusy(isBusy) {
  elements.confirmTotpButton.disabled = isBusy;
  elements.totpCode.disabled = isBusy;
  elements.confirmTotpButton.textContent = isBusy ? "Verifying..." : "Verify";
}

function setLoginTotpBusy(isBusy) {
  elements.verifyLoginTotpButton.disabled = isBusy;
  elements.loginTotpCode.disabled = isBusy;
  elements.verifyLoginTotpButton.textContent = isBusy ? "Verifying..." : "Verify";
}

function setVaultBusy(isBusy, label = "Save") {
  elements.syncVaultButton.disabled = isBusy || !state.vault;
  elements.lockVaultButton.disabled = isBusy || !state.vault;
  elements.vaultItemForm
    .querySelectorAll("input, textarea, button")
    .forEach((element) => {
      element.disabled = isBusy || !state.vault;
    });
  elements.deleteItemButton.disabled = isBusy || !state.vault || !state.selectedItemId;
  elements.saveItemButton.textContent = isBusy ? "Working..." : label;
}

function resetFlow() {
  lockVault(false);
  state.activeStep = "";
  state.csrfToken = "";
  state.factorId = "";
  state.loginMfaChallengeId = "";
  state.pendingFinishPayload = null;
  state.pendingRegisterSession = null;
  state.registrationFinished = false;
  state.accountSecretDisplay = "";
  clearPendingUnlockKey();
  for (const step of elements.flowSteps.querySelectorAll("li")) {
    step.dataset.status = "pending";
  }
}

function beginStep(step) {
  state.activeStep = step;
  setStepStatus(step, "active");
}

function finishStep(step) {
  setStepStatus(step, "done");
  if (state.activeStep === step) {
    state.activeStep = "";
  }
}

function failActiveStep() {
  if (state.activeStep) {
    setStepStatus(state.activeStep, "failed");
  }
}

function setStepStatus(step, status) {
  const node = elements.flowSteps.querySelector(`[data-step="${step}"]`);
  if (node) {
    node.dataset.status = status;
  }
}

function resetOutputs() {
  elements.accountSecretPanel.hidden = true;
  elements.accountSecretKey.textContent = "";
  elements.copySecretButton.disabled = false;
  elements.downloadSecretButton.disabled = false;
  elements.secretSavedCheckbox.checked = false;
  elements.continueEnrollmentButton.disabled = true;
  elements.continueEnrollmentButton.textContent = "Continue enrollment";
  elements.totpPanel.hidden = true;
  elements.manualSecret.textContent = "";
  elements.otpauthUri.value = "";
  elements.totpProfile.textContent = "";
  elements.factorId.textContent = "";
  elements.totpCode.value = "";
  elements.loginMfaPanel.hidden = true;
  elements.loginTotpCode.value = "";
  elements.loginTotpCode.disabled = false;
  elements.verifyLoginTotpButton.disabled = false;
  elements.verifyLoginTotpButton.textContent = "Verify";
  elements.recoveryPanel.hidden = true;
  elements.recoveryCodes.replaceChildren();
  elements.sessionPanel.hidden = true;
  elements.sessionList.replaceChildren();
  renderLockedVault();
}

function setMode(mode) {
  const registerMode = mode === "register";
  elements.registrationForm.hidden = !registerMode;
  elements.loginForm.hidden = registerMode;
  elements.showRegisterButton.setAttribute("aria-selected", String(registerMode));
  elements.showLoginButton.setAttribute("aria-selected", String(!registerMode));
  resetFlow();
  resetOutputs();
  setStatus(registerMode ? "Ready." : "Ready to sign in.");
  if (registerMode) {
    elements.loginHandle.focus();
  } else {
    elements.returnLoginHandle.focus();
  }
}

function ensureBrowserCrypto() {
  if (!globalThis.crypto?.subtle || !globalThis.crypto?.getRandomValues) {
    throw new Error("WebCrypto is unavailable. Use HTTPS or localhost.");
  }
}

function textBytes(value) {
  return encoder.encode(value);
}

function randomBytes(length) {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
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

function base64Url(input) {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function base64UrlToBytes(value) {
  const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(
    Math.ceil(value.length / 4) * 4,
    "=",
  );
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
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

function decodeAccountSecretKeyCandidate(value) {
  try {
    const decoded = base64UrlToBytes(value);
    return decoded.length === 32 ? decoded : null;
  } catch {
    return null;
  }
}

function jsonBytes(value) {
  return textBytes(JSON.stringify(value));
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

function validateKdfProfile(profile) {
  const id = String(profile?.id || "");
  const algorithm = String(profile?.algorithm || "").replace(/_/g, "-").toUpperCase();
  const hash = String(profile?.hash || "").toUpperCase();
  const iterations = Number(profile?.iterations);
  const algorithmOk =
    algorithm === "PBKDF2-HMAC-SHA-256" || (algorithm === "PBKDF2" && hash === "SHA-256");

  if (id !== KDF_PROFILE_ID || !algorithmOk || hash !== "SHA-256" || iterations !== KDF_ITERATIONS) {
    throw new Error(
      `Unsupported KDF profile. Expected ${KDF_PROFILE_ID} with PBKDF2-HMAC-SHA-256/${KDF_ITERATIONS}.`,
    );
  }

  return { hash: "SHA-256", iterations };
}

function validateRegisterStart(response) {
  if (response.auth_protocol !== AUTH_PROTOCOL) {
    throw new Error("Unsupported auth protocol from server.");
  }
  validateKdfProfile(response.kdf_profile);
  if (response.auth_verifier_profile !== SCRAM_PROFILE_ID) {
    throw new Error("Unsupported SCRAM verifier profile from server.");
  }
  if (!Number.isInteger(Number(response.auth_verifier_iterations))) {
    throw new Error("Invalid SCRAM verifier iterations from server.");
  }
}

function validateLoginStart(response) {
  if (response.auth_protocol !== AUTH_PROTOCOL) {
    throw new Error("Unsupported auth protocol from server.");
  }
  validateKdfProfile(response.kdf_profile);
  if (response.auth_verifier_profile !== SCRAM_PROFILE_ID) {
    throw new Error("Unsupported SCRAM verifier profile from server.");
  }
  if (Number(response.auth_verifier_iterations) !== SCRAM_ITERATIONS) {
    throw new Error("Unsupported login verifier iteration count from server.");
  }
}

function wipe(...items) {
  for (const item of items) {
    if (item instanceof Uint8Array) {
      item.fill(0);
    }
  }
}

function clearPendingUnlockKey() {
  if (state.pendingUnlockKey) {
    wipe(state.pendingUnlockKey);
    state.pendingUnlockKey = null;
  }
}

function clearVaultState() {
  if (state.vault) {
    wipe(state.vault.vaultKey, state.vault.integrityKey);
  }
  state.vault = null;
  state.selectedItemId = "";
}

async function pbkdf2Bytes(secretBytes, saltBytes, iterations, hash, lengthBytes = 32) {
  const key = await crypto.subtle.importKey("raw", secretBytes, "PBKDF2", false, ["deriveBits"]);
  const bits = await crypto.subtle.deriveBits(
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
  const key = await crypto.subtle.importKey("raw", inputKeyMaterial, "HKDF", false, [
    "deriveBits",
  ]);
  const bits = await crypto.subtle.deriveBits(
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

async function hmacSha256(keyBytes, dataBytes) {
  const key = await crypto.subtle.importKey(
    "raw",
    keyBytes,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  return new Uint8Array(await crypto.subtle.sign("HMAC", key, dataBytes));
}

async function sha256(dataBytes) {
  return new Uint8Array(await crypto.subtle.digest("SHA-256", dataBytes));
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
  const masterSecret = await deriveMasterSecret(
    masterPassword,
    accountSecretKey,
    accountSalt,
    kdfProfile,
  );
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
  const iterationCount = Number(iterations);
  if (!Number.isInteger(iterationCount) || iterationCount < 4096 || iterationCount > 1000000) {
    throw new Error("Invalid SCRAM verifier iteration count.");
  }
  if (salt.length < 16) {
    throw new Error("Invalid SCRAM verifier salt.");
  }

  const saltedPassword = await pbkdf2Bytes(authSecret, salt, iterationCount, "SHA-256");
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
  const iterationCount = Number(iterations);
  if (!Number.isInteger(iterationCount) || iterationCount < 4096 || iterationCount > 1000000) {
    throw new Error("Invalid SCRAM proof iteration count.");
  }
  if (salt.length < 16) {
    throw new Error("Invalid SCRAM proof salt.");
  }

  const saltedPassword = await pbkdf2Bytes(authSecret, salt, iterationCount, "SHA-256");
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
  const key = await crypto.subtle.importKey("raw", keyBytes, { name: "AES-GCM" }, false, [
    "encrypt",
  ]);
  const nonce = randomBytes(12);
  const algorithm = {
    name: "AES-GCM",
    iv: nonce,
    tagLength: 128,
  };
  if (aadBytes) {
    algorithm.additionalData = aadBytes;
  }
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(algorithm, key, jsonBytes(plaintext)),
  );
  return {
    nonce: base64Url(nonce),
    ciphertext: base64Url(ciphertext),
  };
}

async function aesGcmDecrypt(keyBytes, envelope, aadBytes = null) {
  const key = await crypto.subtle.importKey("raw", keyBytes, { name: "AES-GCM" }, false, [
    "decrypt",
  ]);
  const algorithm = {
    name: "AES-GCM",
    iv: base64UrlToBytes(envelope.nonce),
    tagLength: 128,
  };
  if (aadBytes) {
    algorithm.additionalData = aadBytes;
  }
  const plaintext = await crypto.subtle.decrypt(
    algorithm,
    key,
    base64UrlToBytes(envelope.ciphertext),
  );
  return JSON.parse(decoder.decode(plaintext));
}

async function buildEncryptedRegistrationPayload(startResponse, masterPassword, loginHandle) {
  const createdAt = new Date().toISOString();
  const accountSalt = base64UrlToBytes(startResponse.account_salt);
  const verifierSalt = base64UrlToBytes(startResponse.auth_verifier_salt);
  const accountSecretKey = randomBytes(32);
  const accountKey = randomBytes(32);
  const vaultKey = randomBytes(32);
  const keyId = `browser-unlock-key-${crypto.randomUUID()}`;
  const vaultId = crypto.randomUUID();
  const accountSecretDisplay = displayAccountSecretKey(accountSecretKey);
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
  wipe(accountSalt, verifierSalt, accountSecretKey, accountKey, vaultKey, authSecret);

  return {
    accountSecretDisplay,
    unlockKey,
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
      device: {
        label: browserDeviceLabel(),
        client_type: "browser",
        public_metadata: browserMetadata(),
      },
    },
  };
}

function browserDeviceLabel() {
  const platform = navigator.userAgentData?.platform || navigator.platform || "web";
  return `Browser on ${platform}`.slice(0, 128);
}

function browserMetadata() {
  const brands = navigator.userAgentData?.brands?.map((brand) => brand.brand).slice(0, 4) || [];
  return {
    platform_hint: navigator.userAgentData?.platform || navigator.platform || "web",
    browser_brands: brands,
    static_ui_flow: "browser_static_v1",
  };
}

function validateLoginInputs() {
  const loginHandle = elements.returnLoginHandle.value.trim();
  const masterPassword = elements.loginMasterPassword.value;

  if (!loginHandle) {
    throw new Error("Enter a login handle.");
  }
  if (masterPassword.length < 12) {
    throw new Error("Use at least 12 characters for the master password.");
  }
  const accountSecretKey = parseAccountSecretKey(elements.accountSecretInput.value);

  return { loginHandle, masterPassword, accountSecretKey };
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
    device: {
      label: browserDeviceLabel(),
      client_type: "browser",
      public_metadata: browserMetadata(),
    },
  };
  wipe(
    accountSalt,
    authVerifierSalt,
    serverNonce,
    authSecret,
    authMessage,
    clientFinalWithoutProof,
    clientProof,
  );
  return { payload, unlockKey };
}

async function jsonFetch(path, options = {}) {
  const headers = {
    ...(options.headers || {}),
  };
  const fetchOptions = {
    method: options.method || "GET",
    credentials: "same-origin",
    headers,
  };

  if (options.csrfToken) {
    headers["x-pv-csrf"] = options.csrfToken;
  }
  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
    fetchOptions.body = JSON.stringify(options.body);
  }

  const response = await fetch(path, fetchOptions);
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const message = body?.error?.message || "Request failed.";
    const code = body?.error?.code || response.status;
    throw new Error(`${message} (${code})`);
  }
  return body;
}

async function refreshStatus() {
  try {
    const health = await fetch("/healthz", { credentials: "same-origin" }).then((response) =>
      response.json(),
    );
    elements.healthStatus.textContent = health.status === "ok" ? "Online" : "Unknown";
  } catch {
    elements.healthStatus.textContent = "Unavailable";
  }

  try {
    const ready = await fetch("/readyz", { credentials: "same-origin" }).then((response) =>
      response.json(),
    );
    elements.readyStatus.textContent = ready.status === "ready" ? "Ready" : "Not ready";
  } catch {
    elements.readyStatus.textContent = "Unavailable";
  }
}

function renderAccountSecret(accountSecretDisplay) {
  state.accountSecretDisplay = accountSecretDisplay;
  elements.accountSecretKey.textContent = accountSecretDisplay;
  elements.secretSavedCheckbox.checked = false;
  elements.continueEnrollmentButton.disabled = true;
  elements.accountSecretPanel.hidden = false;
}

async function copyAccountSecret() {
  if (!state.accountSecretDisplay) {
    return;
  }
  try {
    await navigator.clipboard.writeText(state.accountSecretDisplay);
    setStatus("Account secret key copied.", "success");
  } catch {
    setStatus("Copy is unavailable in this browser. Use Download.", "error");
  }
}

function downloadAccountSecret() {
  if (!state.accountSecretDisplay) {
    return;
  }
  const body = [
    "Password Vault account secret key",
    "",
    state.accountSecretDisplay,
    "",
    "This key is required with the master password to unlock the vault.",
  ].join("\n");
  const blob = new Blob([body], { type: "text/plain" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "password-vault-account-secret-key.txt";
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
  setStatus("Account secret key download prepared.", "success");
}

function updateSecretSavedState() {
  elements.continueEnrollmentButton.disabled = !elements.secretSavedCheckbox.checked;
}

function renderTotpEnrollment(response) {
  state.factorId = response.factor_id;
  elements.manualSecret.textContent = response.manual_secret;
  elements.otpauthUri.value = response.otpauth_uri;
  elements.totpProfile.textContent = `${response.totp_profile.algorithm} / ${response.totp_profile.digits} / ${response.totp_profile.period}s`;
  elements.factorId.textContent = response.factor_id;
  elements.totpPanel.hidden = false;
  elements.totpCode.focus();
}

function renderRecoveryCodes(codes) {
  elements.recoveryCodes.replaceChildren();
  for (const code of codes) {
    const item = document.createElement("li");
    item.textContent = code;
    elements.recoveryCodes.append(item);
  }
  elements.recoveryPanel.hidden = false;
}

function renderSession(sessionResponse, fallbackSession) {
  const rows = [];
  if (sessionResponse?.authenticated) {
    rows.push(["authenticated", "true"]);
    rows.push(["account id", sessionResponse.account_id]);
    rows.push(["device id", sessionResponse.device_id || "none"]);
    rows.push(["state", sessionResponse.session_state]);
    rows.push(["vault access", String(sessionResponse.vault_access)]);
    rows.push(["idle expires", sessionResponse.idle_expires_at]);
    rows.push(["absolute expires", sessionResponse.absolute_expires_at]);
  } else if (fallbackSession) {
    rows.push(["authenticated", "from confirm response"]);
    rows.push(["state", fallbackSession.state]);
    rows.push(["vault access", String(fallbackSession.vault_access)]);
    rows.push(["idle expires", fallbackSession.idle_expires_at]);
    rows.push(["absolute expires", fallbackSession.absolute_expires_at]);
  } else {
    rows.push(["authenticated", "false"]);
  }

  elements.sessionList.replaceChildren();
  for (const [key, value] of rows) {
    const row = document.createElement("div");
    const term = document.createElement("dt");
    const detail = document.createElement("dd");
    term.textContent = key;
    detail.textContent = value;
    row.append(term, detail);
    elements.sessionList.append(row);
  }
  elements.sessionPanel.hidden = false;
}

function renderLockedVault() {
  clearVaultState();
  elements.vaultPanel.hidden = true;
  elements.vaultStatus.textContent = "Locked.";
  elements.vaultStatus.className = "status-message";
  elements.vaultItemList.replaceChildren();
  resetVaultForm();
  setVaultBusy(false);
}

function lockVault(updateStatus = true) {
  clearPendingUnlockKey();
  clearVaultState();
  elements.vaultPanel.hidden = true;
  elements.vaultStatus.textContent = "Locked.";
  elements.vaultStatus.className = "status-message";
  elements.vaultItemList.replaceChildren();
  resetVaultForm();
  setVaultBusy(false);
  if (updateStatus) {
    setStatus("Vault locked.", "success");
  }
}

function resetVaultForm() {
  state.selectedItemId = "";
  elements.itemTitle.value = "";
  elements.itemUrl.value = "";
  elements.itemUsername.value = "";
  elements.itemPassword.value = "";
  elements.itemNotes.value = "";
  elements.saveItemButton.textContent = "Save";
  elements.deleteItemButton.disabled = true;
  renderVaultItems();
}

function requireUnlockedVault() {
  if (!state.vault) {
    throw new Error("Vault is locked.");
  }
  return state.vault;
}

function setVaultStatus(message, type = "") {
  elements.vaultStatus.textContent = message;
  elements.vaultStatus.className = `status-message ${type}`.trim();
}

async function decryptVaultKey(unlockKey, vault) {
  const wrapped = vault.encrypted_vault_key;
  if (wrapped.crypto_version !== VAULT_KEY_WRAP_CRYPTO_VERSION) {
    throw new Error("Unsupported vault key wrap.");
  }
  const metadata = await aesGcmDecrypt(unlockKey, wrapped, null);
  if (
    metadata.version !== VAULT_KEY_WRAP_CRYPTO_VERSION ||
    metadata.vault_id !== vault.vault_id ||
    metadata.crypto_profile_id !== VAULT_CRYPTO_PROFILE_ID
  ) {
    throw new Error("Vault key metadata did not match the server vault.");
  }
  const vaultKey = base64UrlToBytes(metadata.vault_key);
  if (vaultKey.length !== 32) {
    throw new Error("Vault key has an invalid length.");
  }
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

async function unlockVaultWithKey(unlockKey) {
  beginStep("vault-unlock");
  setStatus("Unlocking vault...");
  const vaultList = await jsonFetch("/v1/vaults");
  const [serverVault] = vaultList.vaults || [];
  if (!serverVault) {
    throw new Error("No vault is available for this account.");
  }
  const { vaultKey, keyId } = await decryptVaultKey(unlockKey, serverVault);
  const integrityKey = await deriveVaultIntegrityKey(vaultKey, serverVault.vault_id);
  state.vault = {
    vaultId: serverVault.vault_id,
    keyId,
    vaultKey,
    integrityKey,
    headSeq: 0,
    headHash: serverVault.genesis_head_hash || serverVault.head_hash,
    serverHeadSeq: Number(serverVault.head_seq),
    serverHeadHash: serverVault.head_hash,
    items: new Map(),
  };
  clearPendingUnlockKey();
  finishStep("vault-unlock");
  elements.vaultPanel.hidden = false;
  setVaultBusy(false);
  await syncVault();
}

async function unlockVaultFromPendingKey() {
  if (!state.pendingUnlockKey) {
    return;
  }
  const unlockKey = state.pendingUnlockKey;
  state.pendingUnlockKey = null;
  try {
    await unlockVaultWithKey(unlockKey);
  } finally {
    wipe(unlockKey);
  }
}

async function envelopeHash(envelope) {
  return base64Url(await sha256(canonicalBytes(envelope)));
}

async function buildEncryptedItemChange({ operation, plaintext, itemId, baseRevisionSeq }) {
  const vault = requireUnlockedVault();
  const revisionId = crypto.randomUUID();
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

async function verifyAndApplyChange(change) {
  const vault = requireUnlockedVault();
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

  if (
    baseHeadSeq !== vault.headSeq ||
    baseHeadHash !== vault.headHash ||
    previousHeadHash !== vault.headHash
  ) {
    throw new Error("Sync chain does not extend the local vault checkpoint.");
  }
  if (envelope?.crypto_version !== ITEM_ENVELOPE_CRYPTO_VERSION || envelope?.aead !== ITEM_ENVELOPE_AEAD) {
    throw new Error("Unsupported item envelope.");
  }
  const actualEnvelopeHash = await envelopeHash(envelope);
  if (actualEnvelopeHash !== change.envelope_hash) {
    throw new Error("Encrypted item envelope hash mismatch.");
  }
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
  if (expectedChangeMac !== change.change_mac) {
    throw new Error("Vault change MAC mismatch.");
  }
  const expectedHeadHash = await computeHeadHash(
    vault,
    headHashPayload({
      vaultId: vault.vaultId,
      headSeq,
      previousHeadHash,
      changeMac: expectedChangeMac,
    }),
  );
  if (expectedHeadHash !== change.head_hash) {
    throw new Error("Vault head hash mismatch.");
  }

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

async function syncVault() {
  const vault = requireUnlockedVault();
  beginStep("vault-sync");
  setVaultBusy(true, "Save");
  setVaultStatus("Syncing vault...");
  try {
    let hasMore = false;
    do {
      const query = new URLSearchParams({
        from_head_seq: String(vault.headSeq),
        from_head_hash: vault.headHash,
      });
      const response = await jsonFetch(`/v1/vaults/${vault.vaultId}/sync?${query.toString()}`);
      for (const change of response.changes || []) {
        await verifyAndApplyChange(change);
      }
      hasMore = Boolean(response.has_more);
      if (!hasMore) {
        const serverHeadSeq = Number(response.to_head?.seq);
        const serverHeadHash = String(response.to_head?.hash || "");
        if (serverHeadSeq !== vault.headSeq || serverHeadHash !== vault.headHash) {
          throw new Error("Server sync head does not match the verified local chain.");
        }
      }
    } while (hasMore);
    renderVaultItems();
    finishStep("vault-sync");
    setVaultStatus(`Unlocked. ${vault.items.size} item(s).`, "success");
  } catch (error) {
    failActiveStep();
    setVaultStatus(error.message, "error");
    throw error;
  } finally {
    setVaultBusy(false, state.selectedItemId ? "Update" : "Save");
  }
}

function currentItemPlaintext() {
  const title = elements.itemTitle.value.trim();
  if (!title) {
    throw new Error("Enter an item title.");
  }
  return {
    version: "item-plaintext-v1",
    title,
    url: elements.itemUrl.value.trim(),
    username: elements.itemUsername.value.trim(),
    password: elements.itemPassword.value,
    notes: elements.itemNotes.value,
    updated_at: new Date().toISOString(),
  };
}

async function saveVaultItem(event) {
  event.preventDefault();
  const vault = requireUnlockedVault();
  const selected = state.selectedItemId ? vault.items.get(state.selectedItemId) : null;
  const operation = selected ? "update" : "create";
  const itemId = selected?.itemId || crypto.randomUUID();
  const baseRevisionSeq = selected?.revisionSeq || 0;
  const plaintext = currentItemPlaintext();
  setVaultBusy(true, selected ? "Update" : "Save");
  setVaultStatus(selected ? "Updating encrypted item..." : "Creating encrypted item...");

  try {
    beginStep("vault-write");
    const change = await buildEncryptedItemChange({
      operation,
      plaintext,
      itemId,
      baseRevisionSeq,
    });
    const csrf = await jsonFetch("/v1/csrf");
    const body = {
      revision_id: change.revisionId,
      base_head_seq: change.baseHeadSeq,
      base_head_hash: change.baseHeadHash,
      new_head_hash: change.newHeadHash,
      change_mac: change.changeMac,
      envelope_hash: change.envelopeHash,
      encrypted_item_envelope: change.encryptedItemEnvelope,
    };
    const response =
      operation === "create"
        ? await jsonFetch(`/v1/vaults/${vault.vaultId}/items`, {
            method: "POST",
            csrfToken: csrf.csrf_token,
            body: {
              item_id: itemId,
              ...body,
            },
          })
        : await jsonFetch(`/v1/vaults/${vault.vaultId}/items/${itemId}/revisions`, {
            method: "POST",
            csrfToken: csrf.csrf_token,
            body: {
              operation,
              base_revision_seq: baseRevisionSeq,
              ...body,
            },
          });
    if (response.head_hash !== change.newHeadHash || Number(response.head_seq) !== change.headSeq) {
      throw new Error("Server returned an unexpected vault head.");
    }
    await verifyAndApplyChange({
      item_id: itemId,
      revision_id: change.revisionId,
      operation,
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
    });
    state.selectedItemId = itemId;
    renderVaultItems();
    selectVaultItem(itemId);
    finishStep("vault-write");
    setVaultStatus("Encrypted item saved.", "success");
  } catch (error) {
    failActiveStep();
    setVaultStatus(error.message, "error");
    setStatus(error.message, "error");
  } finally {
    setVaultBusy(false, state.selectedItemId ? "Update" : "Save");
  }
}

async function deleteVaultItem() {
  const vault = requireUnlockedVault();
  const selected = state.selectedItemId ? vault.items.get(state.selectedItemId) : null;
  if (!selected) {
    setVaultStatus("Select an item first.", "error");
    return;
  }
  setVaultBusy(true, "Delete");
  setVaultStatus("Deleting encrypted item...");
  try {
    beginStep("vault-write");
    const change = await buildEncryptedItemChange({
      operation: "delete",
      plaintext: {
        version: "item-plaintext-v1",
        deleted: true,
        deleted_at: new Date().toISOString(),
      },
      itemId: selected.itemId,
      baseRevisionSeq: selected.revisionSeq,
    });
    const csrf = await jsonFetch("/v1/csrf");
    const response = await jsonFetch(`/v1/vaults/${vault.vaultId}/items/${selected.itemId}/revisions`, {
      method: "POST",
      csrfToken: csrf.csrf_token,
      body: {
        revision_id: change.revisionId,
        operation: "delete",
        base_revision_seq: selected.revisionSeq,
        base_head_seq: change.baseHeadSeq,
        base_head_hash: change.baseHeadHash,
        new_head_hash: change.newHeadHash,
        change_mac: change.changeMac,
        envelope_hash: change.envelopeHash,
        encrypted_item_envelope: change.encryptedItemEnvelope,
      },
    });
    if (response.head_hash !== change.newHeadHash || Number(response.head_seq) !== change.headSeq) {
      throw new Error("Server returned an unexpected vault head.");
    }
    await verifyAndApplyChange({
      item_id: selected.itemId,
      revision_id: change.revisionId,
      operation: "delete",
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
    });
    resetVaultForm();
    finishStep("vault-write");
    setVaultStatus("Encrypted item deleted.", "success");
  } catch (error) {
    failActiveStep();
    setVaultStatus(error.message, "error");
    setStatus(error.message, "error");
  } finally {
    setVaultBusy(false, state.selectedItemId ? "Update" : "Save");
  }
}

function renderVaultItems() {
  elements.vaultItemList.replaceChildren();
  if (!state.vault) {
    return;
  }
  const items = [...state.vault.items.values()].sort((left, right) =>
    String(left.fields.title || "").localeCompare(String(right.fields.title || "")),
  );
  if (items.length === 0) {
    const empty = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.disabled = true;
    button.innerHTML = '<span class="vault-item-title">No items</span>';
    empty.append(button);
    elements.vaultItemList.append(empty);
    return;
  }
  for (const item of items) {
    const row = document.createElement("li");
    const button = document.createElement("button");
    const title = document.createElement("span");
    const subtitle = document.createElement("span");
    button.type = "button";
    button.setAttribute("aria-selected", String(item.itemId === state.selectedItemId));
    title.className = "vault-item-title";
    subtitle.className = "vault-item-subtitle";
    title.textContent = item.fields.title || "Untitled";
    subtitle.textContent = item.fields.username || item.fields.url || `revision ${item.revisionSeq}`;
    button.append(title, subtitle);
    button.addEventListener("click", () => selectVaultItem(item.itemId));
    row.append(button);
    elements.vaultItemList.append(row);
  }
}

function selectVaultItem(itemId) {
  const item = state.vault?.items.get(itemId);
  if (!item) {
    return;
  }
  state.selectedItemId = itemId;
  elements.itemTitle.value = item.fields.title || "";
  elements.itemUrl.value = item.fields.url || "";
  elements.itemUsername.value = item.fields.username || "";
  elements.itemPassword.value = item.fields.password || "";
  elements.itemNotes.value = item.fields.notes || "";
  elements.saveItemButton.textContent = "Update";
  elements.deleteItemButton.disabled = false;
  renderVaultItems();
}

async function startTotpEnrollment(sessionForDisplay) {
  beginStep("totp-start");
  setStatus("Starting TOTP enrollment...");
  const csrf = await jsonFetch("/v1/csrf");
  state.csrfToken = csrf.csrf_token;
  const totpEnrollment = await jsonFetch("/v1/mfa/totp/enroll/start", {
    method: "POST",
    csrfToken: state.csrfToken,
    body: {},
  });
  renderTotpEnrollment(totpEnrollment);
  finishStep("totp-start");
  renderSession(null, sessionForDisplay);
}

function validateRegistrationInputs() {
  const loginHandle = elements.loginHandle.value.trim();
  const masterPassword = elements.masterPassword.value;
  const confirmPassword = elements.confirmPassword.value;

  if (!loginHandle) {
    throw new Error("Enter a login handle.");
  }
  if (masterPassword.length < 12) {
    throw new Error("Use at least 12 characters for the master password.");
  }
  if (masterPassword !== confirmPassword) {
    throw new Error("Master passwords do not match.");
  }

  return { loginHandle, masterPassword };
}

async function submitRegistration(event) {
  event.preventDefault();
  resetFlow();
  resetOutputs();

  let input;
  try {
    ensureBrowserCrypto();
    input = validateRegistrationInputs();
  } catch (error) {
    setStatus(error.message, "error");
    return;
  }

  setRegistrationBusy(true);
  setStatus("Starting registration...");

  try {
    beginStep("register-start");
    const startResponse = await jsonFetch("/v1/auth/register/start", {
      method: "POST",
      body: {
        login_handle: input.loginHandle,
        auth_protocol: AUTH_PROTOCOL,
      },
    });
    validateRegisterStart(startResponse);
    finishStep("register-start");

    beginStep("derive-keys");
    setStatus("Deriving browser keys...");
    const { accountSecretDisplay, unlockKey, finishPayload } = await buildEncryptedRegistrationPayload(
      startResponse,
      input.masterPassword,
      input.loginHandle,
    );
    renderAccountSecret(accountSecretDisplay);
    clearPendingUnlockKey();
    state.pendingUnlockKey = unlockKey;
    state.pendingFinishPayload = finishPayload;
    finishStep("derive-keys");

    elements.masterPassword.value = "";
    elements.confirmPassword.value = "";
    setStatus("Save the account secret key before continuing.", "success");
  } catch (error) {
    failActiveStep();
    elements.masterPassword.value = "";
    elements.confirmPassword.value = "";
    setStatus(error.message, "error");
  } finally {
    setRegistrationBusy(false);
    refreshStatus();
  }
}

async function submitLogin(event) {
  event.preventDefault();
  resetFlow();
  resetOutputs();

  let input;
  try {
    ensureBrowserCrypto();
    input = validateLoginInputs();
  } catch (error) {
    setStatus(error.message, "error");
    return;
  }

  const clientNonce = randomBytes(32);
  setLoginBusy(true);
  setStatus("Starting sign in...");

  try {
    beginStep("login-start");
    const startResponse = await jsonFetch("/v1/auth/login/start", {
      method: "POST",
      body: {
        login_handle: input.loginHandle,
        auth_protocol: AUTH_PROTOCOL,
        client_nonce: base64Url(clientNonce),
      },
    });
    validateLoginStart(startResponse);
    finishStep("login-start");

    beginStep("login-proof");
    setStatus("Verifying account proof...");
    const { payload: finishPayload, unlockKey } = await buildLoginFinishPayload(
      startResponse,
      input,
      clientNonce,
    );
    clearPendingUnlockKey();
    state.pendingUnlockKey = unlockKey;
    const finishResponse = await jsonFetch("/v1/auth/login/finish", {
      method: "POST",
      body: finishPayload,
    });
    finishStep("login-proof");

    elements.loginMasterPassword.value = "";
    elements.accountSecretInput.value = "";
    if (finishResponse.result === "mfa_required") {
      state.loginMfaChallengeId = finishResponse.mfa_challenge_id;
      elements.loginMfaPanel.hidden = false;
      elements.loginTotpCode.focus();
      setStatus("Enter the TOTP code for this account.", "success");
      return;
    }

    if (finishResponse.result === "session_created") {
      await startTotpEnrollment(finishResponse.session);
      setStatus("TOTP enrollment ready for this account.", "success");
      return;
    }

    throw new Error("Unexpected login response.");
  } catch (error) {
    failActiveStep();
    clearPendingUnlockKey();
    elements.loginMasterPassword.value = "";
    elements.accountSecretInput.value = "";
    setStatus(error.message, "error");
  } finally {
    wipe(clientNonce, input?.accountSecretKey);
    setLoginBusy(false);
    refreshStatus();
  }
}

async function continueEnrollment() {
  if (!elements.secretSavedCheckbox.checked) {
    setStatus("Confirm that the account secret key is saved.", "error");
    return;
  }
  if (!state.pendingFinishPayload && !state.registrationFinished) {
    setStatus("Start registration first.", "error");
    return;
  }

  setSecretContinueBusy(true);

  try {
    beginStep("register-finish");
    let registerSession = state.pendingRegisterSession;
    if (!state.registrationFinished) {
      setStatus("Finishing account setup...");
      const registerFinish = await jsonFetch("/v1/auth/register/finish", {
        method: "POST",
        body: state.pendingFinishPayload,
      });
      state.registrationFinished = true;
      state.pendingFinishPayload = null;
      state.pendingRegisterSession = registerFinish.session;
      registerSession = registerFinish.session;
      finishStep("register-finish");
    } else {
      finishStep("register-finish");
    }

    await startTotpEnrollment(registerSession);
    elements.copySecretButton.disabled = false;
    elements.downloadSecretButton.disabled = false;
    elements.continueEnrollmentButton.disabled = true;
    elements.continueEnrollmentButton.textContent = "Enrollment started";
    setStatus("TOTP enrollment ready.", "success");
  } catch (error) {
    failActiveStep();
    setStatus(error.message, "error");
  } finally {
    if (!state.factorId) {
      setSecretContinueBusy(false);
      if (state.registrationFinished) {
        elements.continueEnrollmentButton.textContent = "Retry enrollment";
      }
    }
    refreshStatus();
  }
}

async function submitLoginTotp(event) {
  event.preventDefault();
  const code = elements.loginTotpCode.value.replace(/\s/g, "");
  let completed = false;
  if (!state.loginMfaChallengeId) {
    setStatus("Start sign in first.", "error");
    return;
  }
  if (!/^\d{6}$/.test(code)) {
    setStatus("Enter a 6-digit TOTP code.", "error");
    return;
  }

  setLoginTotpBusy(true);
  setStatus("Verifying TOTP code...");

  try {
    beginStep("login-mfa");
    const verification = await jsonFetch("/v1/auth/mfa/totp/verify", {
      method: "POST",
      body: {
        mfa_challenge_id: state.loginMfaChallengeId,
        code,
      },
    });
    state.loginMfaChallengeId = "";
    finishStep("login-mfa");

    beginStep("session");
    const session = await jsonFetch("/v1/session").catch(() => null);
    renderSession(session, verification.session);
    finishStep("session");
    await unlockVaultFromPendingKey();

    elements.loginTotpCode.value = "";
    elements.loginMfaPanel.hidden = true;
    completed = true;
    setStatus("Signed in.", "success");
  } catch (error) {
    failActiveStep();
    setStatus(error.message, "error");
  } finally {
    if (!completed) {
      setLoginTotpBusy(false);
    }
    refreshStatus();
  }
}

async function submitTotp(event) {
  event.preventDefault();
  const code = elements.totpCode.value.replace(/\s/g, "");
  let completed = false;
  if (!state.factorId || !state.csrfToken) {
    setStatus("TOTP enrollment is not active.", "error");
    return;
  }
  if (!/^\d{6}$/.test(code)) {
    setStatus("Enter a 6-digit TOTP code.", "error");
    return;
  }

  setTotpBusy(true);
  setStatus("Confirming TOTP code...");

  try {
    beginStep("totp-confirm");
    const confirmation = await jsonFetch("/v1/mfa/totp/enroll/confirm", {
      method: "POST",
      csrfToken: state.csrfToken,
      body: {
        factor_id: state.factorId,
        code,
      },
    });
    finishStep("totp-confirm");
    renderRecoveryCodes(confirmation.recovery_codes || []);

    beginStep("session");
    const session = await jsonFetch("/v1/session").catch(() => null);
    renderSession(session, confirmation.session);
    finishStep("session");
    await unlockVaultFromPendingKey();

    elements.totpCode.value = "";
    elements.totpCode.disabled = true;
    elements.confirmTotpButton.disabled = true;
    completed = true;
    setStatus("Registration complete.", "success");
  } catch (error) {
    failActiveStep();
    setStatus(error.message, "error");
  } finally {
    if (!completed) {
      setTotpBusy(false);
    }
    refreshStatus();
  }
}

elements.showRegisterButton.addEventListener("click", () => setMode("register"));
elements.showLoginButton.addEventListener("click", () => setMode("login"));
elements.registrationForm.addEventListener("submit", submitRegistration);
elements.loginForm.addEventListener("submit", submitLogin);
elements.copySecretButton.addEventListener("click", copyAccountSecret);
elements.downloadSecretButton.addEventListener("click", downloadAccountSecret);
elements.secretSavedCheckbox.addEventListener("change", updateSecretSavedState);
elements.continueEnrollmentButton.addEventListener("click", continueEnrollment);
elements.totpForm.addEventListener("submit", submitTotp);
elements.loginMfaForm.addEventListener("submit", submitLoginTotp);
elements.vaultItemForm.addEventListener("submit", saveVaultItem);
elements.syncVaultButton.addEventListener("click", () => {
  syncVault().catch((error) => setStatus(error.message, "error"));
});
elements.lockVaultButton.addEventListener("click", () => lockVault(true));
elements.newItemButton.addEventListener("click", resetVaultForm);
elements.deleteItemButton.addEventListener("click", deleteVaultItem);
resetFlow();
refreshStatus();
