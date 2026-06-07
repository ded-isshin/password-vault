const AUTH_PROTOCOL = "derived-auth-v1";
const KDF_PROFILE_ID = "pbkdf2-sha256-browser-v1";
const KDF_ITERATIONS = 600000;
const ACCOUNT_KEYSET_CRYPTO_VERSION = "account-keyset-v1";
const VAULT_KEY_WRAP_CRYPTO_VERSION = "vault-key-wrap-v1";
const VAULT_CRYPTO_PROFILE_ID = "vault-crypto-v1";
const SCRAM_PROFILE_ID = "pv-scram-sha-256-v1";

const encoder = new TextEncoder();

const state = {
  activeStep: "",
  csrfToken: "",
  factorId: "",
  pendingFinishPayload: null,
  pendingRegisterSession: null,
  registrationFinished: false,
  accountSecretDisplay: "",
};

const elements = {
  registrationForm: document.querySelector("#registrationForm"),
  loginHandle: document.querySelector("#loginHandle"),
  masterPassword: document.querySelector("#masterPassword"),
  confirmPassword: document.querySelector("#confirmPassword"),
  registerButton: document.querySelector("#registerButton"),
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
  recoveryPanel: document.querySelector("#recoveryPanel"),
  recoveryCodes: document.querySelector("#recoveryCodes"),
  sessionPanel: document.querySelector("#sessionPanel"),
  sessionList: document.querySelector("#sessionList"),
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

function resetFlow() {
  state.activeStep = "";
  state.csrfToken = "";
  state.factorId = "";
  state.pendingFinishPayload = null;
  state.pendingRegisterSession = null;
  state.registrationFinished = false;
  state.accountSecretDisplay = "";
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
  elements.recoveryPanel.hidden = true;
  elements.recoveryCodes.replaceChildren();
  elements.sessionPanel.hidden = true;
  elements.sessionList.replaceChildren();
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
  return `PVSK1-${groups.join("-")}`;
}

function jsonBytes(value) {
  return textBytes(JSON.stringify(value));
}

function normalizeLoginHandle(value) {
  return value.trim().toLowerCase();
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

function wipe(...items) {
  for (const item of items) {
    if (item instanceof Uint8Array) {
      item.fill(0);
    }
  }
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

async function aesGcmEncrypt(keyBytes, plaintext) {
  const key = await crypto.subtle.importKey("raw", keyBytes, { name: "AES-GCM" }, false, [
    "encrypt",
  ]);
  const nonce = randomBytes(12);
  const ciphertext = new Uint8Array(
    await crypto.subtle.encrypt(
      {
        name: "AES-GCM",
        iv: nonce,
        tagLength: 128,
      },
      key,
      jsonBytes(plaintext),
    ),
  );
  return {
    nonce: base64Url(nonce),
    ciphertext: base64Url(ciphertext),
  };
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
  wipe(accountSalt, verifierSalt, accountSecretKey, accountKey, vaultKey, authSecret, unlockKey);

  return {
    accountSecretDisplay,
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
    static_ui_flow: "registration_totp_v1",
  };
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
    const { accountSecretDisplay, finishPayload } = await buildEncryptedRegistrationPayload(
      startResponse,
      input.masterPassword,
      input.loginHandle,
    );
    renderAccountSecret(accountSecretDisplay);
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

    renderSession(null, registerSession);
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

elements.registrationForm.addEventListener("submit", submitRegistration);
elements.copySecretButton.addEventListener("click", copyAccountSecret);
elements.downloadSecretButton.addEventListener("click", downloadAccountSecret);
elements.secretSavedCheckbox.addEventListener("change", updateSecretSavedState);
elements.continueEnrollmentButton.addEventListener("click", continueEnrollment);
elements.totpForm.addEventListener("submit", submitTotp);
resetFlow();
refreshStatus();
