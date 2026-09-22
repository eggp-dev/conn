import assert from 'node:assert/strict';
import { test } from 'node:test';
import { failureKey } from '../../src/lib/collaboration/api.ts';

test('a lost response explains uncertainty even when its message resembles a disconnect', () => {
  const failure = Object.assign(new Error('Connection disconnected after sending the request'), {code:'outcome_unknown'});
  assert.equal(failureKey(failure), 'decision.outcomeUnknown');
});
test('native string errors and pending outcomes use the same uncertainty message', () => {
  assert.equal(failureKey('outcome_unknown'), 'decision.outcomeUnknown');
  assert.equal(failureKey(Object.assign(new Error('Still executing'), {code:'outcome_pending'})), 'decision.outcomeUnknown');
});
test('ordinary failures retain their existing specific or retryable guidance', () => {
  assert.equal(failureKey(new Error('sharing_changed')), 'sharing.changed');
  assert.equal(failureKey(new Error('input_pending')), 'sharing.pendingInput');
  assert.equal(failureKey(new Error('Temporary failure')), 'decision.failed');
});
