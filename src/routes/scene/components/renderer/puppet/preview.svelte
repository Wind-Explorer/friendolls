<script lang="ts">
  import { messages } from "$lib/i18n";
  import { onDestroy, onMount } from "svelte";
  import * as THREE from "three";
  import { PuppetVisual, SOUTH_WEST_ROTATION_Y } from "./visual";

  let {
    userId,
    skinHash,
    skinSource = null,
    scale,
    opacity,
  }: {
    userId: string;
    skinHash: string | null;
    skinSource?: string | null;
    scale: number;
    opacity: number;
  } = $props();

  let container = $state<HTMLDivElement | null>(null);
  let renderer: THREE.WebGLRenderer | null = null;
  let visual: PuppetVisual | null = null;
  let clock: THREE.Timer | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let animationFrameId = 0;

  function animate() {
    if (!renderer || !visual || !clock) return;
    animationFrameId = requestAnimationFrame(animate);

    clock.update();
    visual.setAppearance({ skinHash, skinSource, scale, opacity });
    visual.walkInPlace(SOUTH_WEST_ROTATION_Y, clock.getElapsed());
    renderer.render(scene, camera);
  }

  function resizePreview() {
    if (!container || !renderer) return;
    const { clientWidth: width, clientHeight: height } = container;
    if (width === 0 || height === 0) return;

    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setSize(width, height);
    camera.left = width / -2;
    camera.right = width / 2;
    camera.top = height / 2;
    camera.bottom = height / -2;
    camera.updateProjectionMatrix();
  }

  const scene = new THREE.Scene();
  scene.background = null;
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.1, 1_000);
  camera.position.set(200, 134, 200);
  camera.lookAt(0, 34, 0);
  camera.updateProjectionMatrix();

  scene.add(new THREE.AmbientLight("white", 0.8));
  const directionalLight = new THREE.DirectionalLight("white", 2);
  directionalLight.position.set(200, 150, 150);
  scene.add(directionalLight);

  onMount(() => {
    if (!container) return;

    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setSize(1, 1);
    container.appendChild(renderer.domElement);
    resizePreview();

    resizeObserver = new ResizeObserver(resizePreview);
    resizeObserver.observe(container);

    visual = new PuppetVisual(userId);
    scene.add(visual.root);

    clock = new THREE.Timer();
    clock.connect(document);
    animate();
  });

  onDestroy(() => {
    cancelAnimationFrame(animationFrameId);
    resizeObserver?.disconnect();
    clock?.dispose();
    visual?.dispose();
    renderer?.dispose();
    renderer?.domElement.remove();
  });
</script>

<div
  class="aspect-square size-full overflow-hidden"
  aria-label={$messages.scene_loading_preview()}
  role="img"
  bind:this={container}
></div>
