<script lang="ts">
  import { commands } from "$lib/bindings";
  import {
    friendStatusesListenerError,
    onlineFriendIds,
  } from "$lib/listeners/friend-statuses";
  import { friends, friendsListenerError } from "$lib/listeners/friends";
  import { onMount } from "svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";

  let { register }: { register: RegisterPanel } = $props();

  let selectedId = $state<string | null>(null);
  let mode = $state<"browse" | "remove">("browse");
  let busy = $state(false);
  let error = $state("");

  let dirty = $derived(mode !== "browse");
  let selected = $derived(
    $friends.find((friend) => friend.id === selectedId) ?? null,
  );

  $effect(() => register("friends", { apply, reset }, { dirty, busy }));
  $effect(() => {
    if (selectedId && !$friends.some((friend) => friend.id === selectedId))
      selectedId = null;
  });
  onMount(() => register("friends", { apply, reset }, { dirty, busy }));

  function reset() {
    mode = "browse";
    error = "";
  }

  async function openAddActionWindow() {
    busy = true;
    error = "";
    try {
      await commands.openActionWindow(
        "friend",
        "Add Friend",
        "/control-panel/add/friend",
      );
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function apply() {
    if (mode === "browse") return true;
    if (mode === "remove" && !selected) {
      error = "The selected friend no longer exists.";
      return false;
    }

    busy = true;
    error = "";
    try {
      if (selected) {
        await commands.deleteFriend(selected.id);
        selectedId = null;
      }
      reset();
      return true;
    } catch (cause) {
      error = String(cause);
      return false;
    } finally {
      busy = false;
    }
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
  {#if error || $friendsListenerError || $friendStatusesListenerError}
    <PanelMessage kind="error"
      >{error ||
        $friendsListenerError ||
        $friendStatusesListenerError}</PanelMessage
    >
  {/if}

  {#if mode === "remove" && selected}
    <PanelMessage kind="warning">
      Applying will remove <strong>{selected.displayName}</strong>. They will
      immediately go offline and stop receiving your activity.
    </PanelMessage>
    <button class="btn w-fit" type="button" onclick={() => (mode = "browse")}
      >Keep friend</button
    >
  {:else}
    <div
      class="flex min-h-0 flex-1 flex-col border border-base-300 bg-base-100"
    >
      <div
        class="grid grid-cols-[1fr_4.5rem] border-b border-base-300 bg-base-200 px-2 py-1 text-[10px] font-bold uppercase tracking-wide text-base-content/60"
      >
        <span>Name</span><span>Status</span>
      </div>
      <div
        class="min-h-0 flex-1 overflow-y-auto"
        role="listbox"
        aria-label="Friends"
      >
        {#if $friends.length === 0}
          <div
            class="grid h-full min-h-36 place-content-center px-8 text-center"
          >
            <p class="text-xs font-bold">No friends yet</p>
            <p class="mt-1 text-xs text-base-content/60">
              Add someone using their public key.
            </p>
          </div>
        {:else}
          {#each $friends as friend (friend.id)}
            {@const online = $onlineFriendIds.has(friend.id)}
            <button
              type="button"
              role="option"
              aria-selected={selectedId === friend.id}
              class="grid w-full grid-cols-[1fr_4.5rem] items-center border-b border-base-200 px-2 py-1.5 text-left text-xs hover:bg-base-200 aria-selected:bg-primary aria-selected:text-primary-content"
              onclick={() => (selectedId = friend.id)}
              ondblclick={() => (selectedId = friend.id)}
            >
              <span class="min-w-0">
                <strong class="block truncate">{friend.displayName}</strong>
                <span class="block truncate font-mono text-[9px] opacity-65"
                  >{friend.id}</span
                >
              </span>
              <span class="flex items-center gap-1.5">
                <span class:status-success={online} class="status shadow-none"
                ></span>
                {online ? "Online" : "Offline"}
              </span>
            </button>
          {/each}
        {/if}
      </div>
    </div>

    <div class="flex justify-between gap-2">
      <button
        class="btn"
        type="button"
        disabled={busy}
        onclick={openAddActionWindow}>Add…</button
      >
      <button
        class="btn"
        type="button"
        disabled={!selected}
        onclick={() => (mode = "remove")}>Remove</button
      >
    </div>
  {/if}
</div>
