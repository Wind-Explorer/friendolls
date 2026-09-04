import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from 'three';
import { PuppetVisual } from '../src/routes/scene/components/renderer/puppet/visual';
import { SceneScheduler } from '../src/routes/scene/components/renderer/scheduler';

test('scheduler coalesces wakeups, sleeps, resets time, and cancels on suspension/disposal', () => {
  const originalRequest = globalThis.requestAnimationFrame;
  const originalCancel = globalThis.cancelAnimationFrame;
  const pending = new Map<number, FrameRequestCallback>();
  let id = 0;
  globalThis.requestAnimationFrame = (callback) => {
    pending.set(++id, callback);
    return id;
  };
  globalThis.cancelAnimationFrame = (frameId) => { pending.delete(frameId); };
  let active = true;
  const deltas: number[] = [];
  const scheduler = new SceneScheduler((delta) => {
    deltas.push(delta);
    return active;
  });
  const tick = (timestamp: number) => {
    assert.equal(pending.size, 1);
    const [frameId, callback] = [...pending][0];
    pending.delete(frameId);
    callback(timestamp);
  };
  try {
    scheduler.invalidate();
    scheduler.invalidate();
    tick(100);
    active = false;
    tick(116);
    assert.equal(pending.size, 0);
    scheduler.invalidate();
    tick(100_000);
    assert.deepEqual(deltas, [0, 0.016, 0]);
    scheduler.invalidate();
    scheduler.setSuspended(true);
    scheduler.invalidate();
    assert.equal(pending.size, 0);
    scheduler.setSuspended(false);
    tick(200_000);
    assert.equal(deltas.at(-1), 0);
    scheduler.invalidate();
    scheduler.dispose();
    scheduler.invalidate();
    assert.equal(pending.size, 0);
  } finally {
    scheduler.dispose();
    globalThis.requestAnimationFrame = originalRequest;
    globalThis.cancelAnimationFrame = originalCancel;
  }
});

test('movement and idle turn finish at an exact stable pose', () => {
  const visual = new PuppetVisual('test');
  const origin = new THREE.Vector3();
  const destination = new THREE.Vector3(100, 0, 100);
  try {
    assert.equal(visual.updateMotion(origin, false, 0, 0), false);
    for (let i = 1; i <= 20; i++) {
      assert.equal(visual.updateMotion(destination, true, 1 / 60, i / 60), true);
    }
    let active = true;
    let frames = 0;
    while (active && frames < 600) {
      active = visual.updateMotion(destination, false, 1 / 60, (20 + ++frames) / 60);
    }
    assert.ok(frames > 1 && frames < 600);
    assert.deepEqual(visual.root.position, destination);
    const rotation = visual.root.rotation.y;
    assert.equal(visual.updateMotion(destination, false, 1 / 60, 20), false);
    assert.equal(visual.root.rotation.y, rotation);
  } finally {
    visual.dispose();
  }
});

test('limb relaxation has the same speed at 30 and 120 FPS and finishes', () => {
  const slow = new PuppetVisual('slow');
  const fast = new PuppetVisual('fast');
  try {
    slow.walkInPlace(0, 0.1);
    fast.walkInPlace(0, 0.1);
    for (let i = 0; i < 6; i++) slow.pause(1 / 30);
    for (let i = 0; i < 24; i++) fast.pause(1 / 120);
    slow.root.children.forEach((limb, index) => {
      assert.ok(Math.abs(limb.rotation.x - fast.root.children[index].rotation.x) < 1e-10);
    });
    for (let i = 0; i < 120; i++) fast.pause(1 / 120);
    assert.equal(fast.pause(0), false);
    assert.ok(fast.root.children.every((limb) => limb.rotation.x === 0));
  } finally {
    slow.dispose();
    fast.dispose();
  }
});
