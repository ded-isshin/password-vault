import { check } from 'k6';
import http from 'k6/http';
import { baseUrl, runId, scenario, smokeThresholds } from '../lib/config.js';
import { clientNonce, jsonHeaders, stableLoginHandle } from '../lib/data.js';

export const options = {
  scenarios: {
    login_start: scenario('login_start'),
  },
  thresholds: smokeThresholds,
};

export default function () {
  const handle = stableLoginHandle(runId);
  const payload = JSON.stringify({
    login_handle: handle,
    auth_protocol: 'derived-auth-v1',
    client_nonce: clientNonce(handle),
  });

  const response = http.post(`${baseUrl}/v1/auth/login/start`, payload, {
    headers: jsonHeaders(),
  });

  check(response, {
    'login/start is 200': (result) => result.status === 200,
    'login/start has challenge id': (result) => Boolean(result.json('login_challenge_id')),
    'login/start has combined nonce': (result) => Boolean(result.json('combined_nonce')),
    'login/start has no-store': (result) => String(result.headers['Cache-Control']).includes('no-store'),
  });
}
