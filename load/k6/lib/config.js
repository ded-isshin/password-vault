export const baseUrl = (__ENV.BASE_URL || 'http://127.0.0.1:8080').replace(/\/+$/, '');
export const runId = sanitize(__ENV.RUN_ID || 'manual');
export const loadRate = parseInt(__ENV.LOAD_RATE || '2', 10);
export const loadDuration = __ENV.LOAD_DURATION || '15s';

export function scenario(name) {
  return {
    executor: 'constant-arrival-rate',
    rate: loadRate,
    timeUnit: '1s',
    duration: loadDuration,
    preAllocatedVUs: Math.max(4, loadRate * 2),
    maxVUs: Math.max(8, loadRate * 4),
    tags: { scenario: name },
  };
}

export const smokeThresholds = {
  checks: ['rate>0.99'],
  http_req_failed: ['rate<0.01'],
};

export const performanceThresholds = {
  checks: ['rate>0.99'],
  http_req_failed: ['rate<0.01'],
  http_req_duration: ['p(95)<300', 'p(99)<800'],
};

function sanitize(value) {
  return String(value).replace(/[^a-zA-Z0-9._-]/g, '-').slice(0, 64);
}
