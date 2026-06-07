const AUTH_PROTOCOL = "derived-auth-v1";

const state = {
  mode: "register",
};

const elements = {
  registerTab: document.querySelector("#registerTab"),
  loginTab: document.querySelector("#loginTab"),
  form: document.querySelector("#challengeForm"),
  title: document.querySelector("#formTitle"),
  copy: document.querySelector("#formCopy"),
  loginHandle: document.querySelector("#loginHandle"),
  submitButton: document.querySelector("#submitButton"),
  statusMessage: document.querySelector("#statusMessage"),
  resultPanel: document.querySelector("#resultPanel"),
  resultMode: document.querySelector("#resultMode"),
  resultList: document.querySelector("#resultList"),
  healthStatus: document.querySelector("#healthStatus"),
  readyStatus: document.querySelector("#readyStatus"),
};

const copy = {
  register: {
    tab: "register/start",
    title: "Create your vault",
    body: "Start a server challenge and receive the KDF profile needed for client-side key derivation.",
  },
  login: {
    tab: "login/start",
    title: "Unlock your vault",
    body: "Request a login challenge. The finish step and vault session are not implemented yet.",
  },
};

function setMode(mode) {
  state.mode = mode;
  const isRegister = mode === "register";
  elements.registerTab.classList.toggle("active", isRegister);
  elements.loginTab.classList.toggle("active", !isRegister);
  elements.registerTab.setAttribute("aria-selected", String(isRegister));
  elements.loginTab.setAttribute("aria-selected", String(!isRegister));
  elements.title.textContent = copy[mode].title;
  elements.copy.textContent = copy[mode].body;
  elements.resultPanel.hidden = true;
  setStatus("Your master password is never sent by this preview screen.");
}

function setStatus(message, type = "") {
  elements.statusMessage.textContent = message;
  elements.statusMessage.className = `status-message ${type}`.trim();
}

function setBusy(isBusy) {
  elements.submitButton.disabled = isBusy;
  elements.loginHandle.disabled = isBusy;
  elements.submitButton.textContent = isBusy ? "Contacting server..." : "Continue";
}

function base64Url(bytes) {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

function clientNonce() {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  return base64Url(bytes);
}

async function jsonFetch(path, options = {}) {
  const response = await fetch(path, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...(options.headers || {}),
    },
  });
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
    const health = await fetch("/healthz").then((response) => response.json());
    elements.healthStatus.textContent = health.status === "ok" ? "Online" : "Unknown";
  } catch {
    elements.healthStatus.textContent = "Unavailable";
  }

  try {
    const ready = await fetch("/readyz").then((response) => response.json());
    elements.readyStatus.textContent = ready.status === "ready" ? "Ready" : "Not ready";
  } catch {
    elements.readyStatus.textContent = "Unavailable";
  }
}

function renderResult(mode, response) {
  const rows = [
    ["protocol", response.auth_protocol],
    ["kdf", JSON.stringify(response.kdf_profile)],
    ["account salt", response.account_salt],
    ["verifier profile", response.auth_verifier_profile],
    ["verifier salt", response.auth_verifier_salt],
    ["iterations", String(response.auth_verifier_iterations)],
    ["expires", response.expires_at],
  ];

  if (mode === "register") {
    rows.unshift(["registration id", response.registration_id]);
  } else {
    rows.unshift(["login challenge id", response.login_challenge_id]);
    rows.push(["server nonce", response.server_nonce]);
    rows.push(["combined nonce", response.combined_nonce]);
  }

  elements.resultMode.textContent = copy[mode].tab;
  elements.resultList.replaceChildren();
  for (const [key, value] of rows) {
    const row = document.createElement("div");
    const term = document.createElement("dt");
    const detail = document.createElement("dd");
    term.textContent = key;
    detail.textContent = value;
    row.append(term, detail);
    elements.resultList.append(row);
  }
  elements.resultPanel.hidden = false;
}

async function submitChallenge(event) {
  event.preventDefault();
  const loginHandle = elements.loginHandle.value.trim();
  if (!loginHandle) {
    setStatus("Enter a login handle first.", "error");
    return;
  }

  setBusy(true);
  elements.resultPanel.hidden = true;
  setStatus("Establishing a secure challenge...");

  try {
    const payload = {
      login_handle: loginHandle,
      auth_protocol: AUTH_PROTOCOL,
    };
    const path =
      state.mode === "register" ? "/v1/auth/register/start" : "/v1/auth/login/start";

    if (state.mode === "login") {
      payload.client_nonce = clientNonce();
    }

    const response = await jsonFetch(path, {
      method: "POST",
      body: JSON.stringify(payload),
    });
    renderResult(state.mode, response);
    setStatus("Secure challenge established. Finish/session support is coming next.", "success");
  } catch (error) {
    setStatus(error.message, "error");
  } finally {
    setBusy(false);
    refreshStatus();
  }
}

elements.registerTab.addEventListener("click", () => setMode("register"));
elements.loginTab.addEventListener("click", () => setMode("login"));
elements.form.addEventListener("submit", submitChallenge);
refreshStatus();
