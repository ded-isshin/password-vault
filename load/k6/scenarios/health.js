import { check } from 'k6';
import http from 'k6/http';
import { baseUrl, metricsBaseUrl, scenario, smokeThresholds } from '../lib/config.js';

export const options = {
  scenarios: {
    health: scenario('health'),
  },
  thresholds: smokeThresholds,
};

export default function () {
  const health = http.get(`${baseUrl}/healthz`);
  check(health, {
    'healthz is 200': (response) => response.status === 200,
  });

  const ready = http.get(`${baseUrl}/readyz`);
  check(ready, {
    'readyz is 200': (response) => response.status === 200,
  });

  const metrics = http.get(`${metricsBaseUrl}/metrics`);
  check(metrics, {
    'metrics is 200': (response) => response.status === 200,
    'metrics expose http counters': (response) => response.body.includes('axum_http_requests_total'),
  });
}
