import { check } from 'k6';
import http from 'k6/http';
import { baseUrl, runId, scenario, smokeThresholds } from '../lib/config.js';
import { jsonHeaders, loginHandle } from '../lib/data.js';

export const options = {
  scenarios: {
    register_start: scenario('register_start'),
  },
  thresholds: smokeThresholds,
};

export default function () {
  const payload = JSON.stringify({
    login_handle: loginHandle(runId),
    auth_protocol: 'derived-auth-v1',
  });

  const response = http.post(`${baseUrl}/v1/auth/register/start`, payload, {
    headers: jsonHeaders(),
  });

  check(response, {
    'register/start is 200': (result) => result.status === 200,
    'register/start has challenge id': (result) => Boolean(result.json('registration_id')),
    'register/start has no-store': (result) => String(result.headers['Cache-Control']).includes('no-store'),
  });
}
