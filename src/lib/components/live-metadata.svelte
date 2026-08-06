<script lang="ts">
  import { onMount } from "svelte";
  import {
    events,
    type AppMeta,
    type CursorPositions,
  } from "$lib/bindings";

  let cursor: CursorPositions | null = null;
  let foregroundApp: AppMeta | null = null;

  onMount(() => {
    let disposed = false;
    let stopCursor: (() => void) | undefined;
    let stopForegroundApp: (() => void) | undefined;

    async function initialize() {
      const [cursorUnlisten, foregroundAppUnlisten] = await Promise.all([
        events.cursorPositionChanged.listen((event) => {
          cursor = event.payload.positions;
        }),
        events.foregroundAppChanged.listen((event) => {
          foregroundApp = event.payload.meta;
        }),
      ]);

      if (disposed) {
        cursorUnlisten();
        foregroundAppUnlisten();
        return;
      }

      stopCursor = cursorUnlisten;
      stopForegroundApp = foregroundAppUnlisten;
    }

    initialize();

    return () => {
      disposed = true;
      stopCursor?.();
      stopForegroundApp?.();
    };
  });
</script>

<section>
  <h1>Live metadata</h1>

  <h2>Cursor</h2>
  {#if cursor}
    <p>Raw: {cursor.raw.x}, {cursor.raw.y}</p>
    <p>Mapped: {cursor.mapped.x}, {cursor.mapped.y}</p>
  {:else}
    <p>Waiting for cursor data...</p>
  {/if}

  <h2>Foreground app</h2>
  {#if foregroundApp}
    {#if foregroundApp.ico}
      <img
        src={`data:image/png;base64,${foregroundApp.ico}`}
        alt=""
        width="64"
        height="64"
      />
    {/if}
    <p>Localized name: {foregroundApp.local ?? "Unavailable"}</p>
    <p>Executable name: {foregroundApp.unlocal ?? "Unavailable"}</p>
  {:else}
    <p>Waiting for foreground app data...</p>
  {/if}
</section>
