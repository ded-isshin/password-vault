import { check } from 'k6';
import http from 'k6/http';
import { baseUrl, metricsBaseUrl, runId, scenario, smokeThresholds } from '../lib/config.js';
import { clientNonce, jsonHeaders, loginHandle, stableLoginHandle } from '../lib/data.js';

export const options = {
  scenarios: {
    smoke: scenario('smoke'),
  },
  thresholds: smokeThresholds,
};

export default function () {
  const health = http.get(`${baseUrl}/healthz`);
  check(health, {
    'healthz is 200': (response) => response.status === 200,
  });

  const registerPayload = JSON.stringify({
    login_handle: loginHandle(runId),
    auth_protocol: 'derived-auth-v1',
  });
  const register = http.post(`${baseUrl}/v1/auth/register/start`, registerPayload, {
    headers: jsonHeaders(),
  });
  check(register, {
    'register/start is 200': (response) => response.status === 200,
    'register/start has challenge id': (response) => Boolean(response.json('registration_id')),
  });

  const login = stableLoginHandle(runId);
  const loginPayload = JSON.stringify({
    login_handle: login,
    auth_protocol: 'derived-auth-v1',
    client_nonce: clientNonce(login),
  });
  const loginStart = http.post(`${baseUrl}/v1/auth/login/start`, loginPayload, {
    headers: jsonHeaders(),
  });
  check(loginStart, {
    'login/start is 200': (response) => response.status === 200,
    'login/start has combined nonce': (response) => Boolean(response.json('combined_nonce')),
  });

  const metrics = http.get(`${metricsBaseUrl}/metrics`);
  check(metrics, {
    'metrics is 200': (response) => response.status === 200,
    'metrics expose http counters': (response) => response.body.includes('axum_http_requests_total'),
  });
}
