import encoding from 'k6/encoding';

export function loginHandle(runId) {
  return `loadtest+${runId}-${__VU}-${__ITER}@loadtest.invalid`;
}

export function stableLoginHandle(runId) {
  return `loadtest+${runId}-stable-${__VU % 8}@loadtest.invalid`;
}

export function clientNonce(label) {
  const raw = fixedLength(`${label}:${__VU}:${__ITER}`, 32);
  return encoding.b64encode(raw, 'rawurl');
}

export function jsonHeaders() {
  return {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  };
}

function fixedLength(value, length) {
  let output = String(value);
  while (output.length < length) {
    output += '.';
  }
  return output.slice(0, length);
}
