<script lang="ts">
  import { friends } from "$lib/listeners/friends";
  import {
    liveMetadata,
    liveMetadataListenerError,
  } from "$lib/listeners/live-metadata";
  import { onMount } from "svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";

  let { register }: { register: RegisterPanel } = $props();

  const actions = { apply: async () => true, reset: () => undefined };
  onMount(() => register("activity", actions, { dirty: false, busy: false }));

  function userIds() {
    return [
      ...new Set([
        $liveMetadata.localId,
        ...Object.keys($liveMetadata.cursorPositions),
        ...$liveMetadata.foregroundApps.keys(),
      ]),
    ].filter(Boolean);
  }

  function nameFor(id: string) {
    if (id === $liveMetadata.localId) return "You";
    return (
      $friends.find((friend) => friend.id === id)?.displayName ??
      "Unknown friend"
    );
  }

  function compactId(id: string) {
    return id.length > 18 ? `${id.slice(0, 9)}…${id.slice(-6)}` : id;
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
  {#if $liveMetadataListenerError}
    <PanelMessage kind="error">{$liveMetadataListenerError}</PanelMessage>
  {:else if userIds().length === 0}
    <PanelMessage
      >Waiting for activity data. This usually appears after Wyd has started
      monitoring.</PanelMessage
    >
  {/if}

  <div
    class="min-h-0 flex-1 overflow-y-auto border border-base-300 bg-base-100"
  >
    {#each userIds() as userId (userId)}
      {@const cursor = $liveMetadata.cursorPositions[userId]}
      {@const app = $liveMetadata.foregroundApps.get(userId)}
      <article class="border-b border-base-300 p-2 last:border-b-0">
        <div class="flex items-center gap-2">
          {#if app?.ico}
            <img
              class="pixelated size-8 border border-base-300 bg-base-200 p-0.5"
              src={`data:image/png;base64,${app.ico}`}
              alt=""
              width="32"
              height="32"
            />
          {:else}
            <div
              class="grid size-8 shrink-0 place-content-center border border-base-300 bg-base-200 text-sm"
              aria-hidden="true"
            >
              ▣
            </div>
          {/if}
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-1.5">
              <strong class="truncate text-xs">{nameFor(userId)}</strong>
              {#if userId === $liveMetadata.localId}<span class="badge badge-xs"
                  >Local</span
                >{/if}
            </div>
            <p
              class="truncate font-mono text-[9px] text-base-content/55"
              title={userId}
            >
              {compactId(userId)}
            </p>
          </div>
        </div>

        <dl
          class="mt-2 grid grid-cols-[5rem_1fr] gap-x-2 gap-y-0.5 text-[10px]"
        >
          <dt class="text-base-content/60">Foreground app</dt>
          <dd class="truncate">
            {app?.local ?? app?.unlocal ?? "Not available"}
          </dd>
          <dt class="text-base-content/60">Cursor</dt>
          <dd>
            {cursor
              ? `${Math.round(cursor.raw.x)}, ${Math.round(cursor.raw.y)} px`
              : "Not available"}
          </dd>
          <dt class="text-base-content/60">Mapped</dt>
          <dd>
            {cursor
              ? `${cursor.mapped.x.toFixed(3)}, ${cursor.mapped.y.toFixed(3)}`
              : "Not available"}
          </dd>
        </dl>
      </article>
    {/each}
  </div>
</div>
