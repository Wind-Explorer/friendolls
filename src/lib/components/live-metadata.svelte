<script lang="ts">
  import { onMount } from "svelte";
  import {
    commands,
    events,
    type AppMeta,
    type CursorPositions,
  } from "$lib/bindings";

  let localId = "";
  let cursorPositions: Partial<Record<string, CursorPositions>> = {};
  let foregroundApps = new Map<string, AppMeta>();
  let pendingLocalForegroundApp: AppMeta | null = null;
  let error = "";

  function updateForegroundApp(userId: string, meta: AppMeta) {
    foregroundApps = new Map(foregroundApps).set(userId, meta);
  }

  function liveUserIds() {
    return [
      ...new Set([
        localId,
        ...Object.keys(cursorPositions),
        ...foregroundApps.keys(),
      ]),
    ].filter(Boolean);
  }

  function compactId(id: string) {
    return id.length > 16 ? `${id.slice(0, 8)}…${id.slice(-6)}` : id;
  }

  onMount(() => {
    let disposed = false;
    let stopListeners: (() => void)[] = [];

    async function initialize() {
      const listeners = await Promise.all([
        events.cursorPositionChanged.listen((event) => {
          if (!disposed) cursorPositions = event.payload.positions;
        }),
        events.foregroundAppChanged.listen((event) => {
          if (disposed) return;
          if (localId) {
            updateForegroundApp(localId, event.payload.meta);
          } else {
            pendingLocalForegroundApp = event.payload.meta;
          }
        }),
        events.friendForegroundAppChanged.listen((event) => {
          if (!disposed) {
            updateForegroundApp(event.payload.friendId, event.payload.meta);
          }
        }),
      ]);

      if (disposed) {
        listeners.forEach((stop) => stop());
        return;
      }
      stopListeners = listeners;

      localId = await commands.getPublicKey();
      if (disposed) return;
      if (pendingLocalForegroundApp) {
        updateForegroundApp(localId, pendingLocalForegroundApp);
        pendingLocalForegroundApp = null;
      }
    }

    initialize().catch((err) => {
      if (!disposed) error = String(err);
    });

    return () => {
      disposed = true;
      stopListeners.forEach((stop) => stop());
    };
  });
</script>

<section>
  <h1>Live metadata</h1>

  {#if error}
    <p>{error}</p>
  {:else if liveUserIds().length === 0}
    <p>Waiting for live metadata...</p>
  {:else}
    {#each liveUserIds() as userId (userId)}
      {@const cursor = cursorPositions[userId]}
      {@const foregroundApp = foregroundApps.get(userId)}
      <article>
        <h2>{userId === localId ? "Local user" : "Remote user"}</h2>
        <p title={userId}>ID: {compactId(userId)}</p>

        <h3>Cursor</h3>
        {#if cursor}
          <p>Raw: {cursor.raw.x}, {cursor.raw.y}</p>
          <p>Mapped: {cursor.mapped.x}, {cursor.mapped.y}</p>
        {:else}
          <p>Waiting for cursor data...</p>
        {/if}

        <h3>Foreground app</h3>
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
      </article>
    {/each}
  {/if}
</section>
