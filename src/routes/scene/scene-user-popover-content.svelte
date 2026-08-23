<script lang="ts">
  import type { AppMeta, CursorPositions } from "$lib/bindings";
  import { friends } from "$lib/listeners/friends";
  import { profile } from "$lib/listeners/profile";

  type Props = {
    titleId: string;
    userId: string;
    isLocal: boolean;
    cursor: CursorPositions;
    foregroundApp?: AppMeta;
  };

  let { titleId, userId, isLocal, cursor, foregroundApp }: Props = $props();
  let username = $derived(
    $profile?.id === userId
      ? $profile.displayName
      : ($friends.find((friend) => friend.id === userId)?.displayName ??
          "Unknown user"),
  );

  function compactId(id: string) {
    return id.length > 18 ? `${id.slice(0, 9)}…${id.slice(-7)}` : id;
  }
</script>

<article class="w-68 overflow-hidden text-neutral-100">
  <header
    class="flex w-max items-center ml-2 gap-1 px-2 py-0.5 bg-neutral-800 border border-neutral-500 border-b-0"
  >
    <div class="min-w-0 flex-1">
      <p id={titleId} class="truncate text-xs">{username}</p>
    </div>
  </header>

  <div
    class="grid gap-3 p-2 text-sm bg-neutral-900 border border-neutral-500 shadow-lg"
  >
    <section>
      <h3 class="text-xs font-medium text-neutral-400">Currently enjoying</h3>
      <div class="mt-1 flex min-w-0 items-center gap-2">
        {#if foregroundApp?.ico}
          <img
            src={`data:image/png;base64,${foregroundApp.ico}`}
            alt=""
            class="size-6 shrink-0 object-contain"
          />
        {/if}
        <div class="min-w-0">
          <p class="truncate text-xs">
            {foregroundApp?.local ??
              foregroundApp?.unlocal ??
              "Waiting for data"}
          </p>
          {#if foregroundApp?.local && foregroundApp.unlocal}
            <p class="truncate text-xs text-neutral-400">
              {foregroundApp.unlocal}
            </p>
          {/if}
        </div>
      </div>
    </section>
  </div>
</article>
