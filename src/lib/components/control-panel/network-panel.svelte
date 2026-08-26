<script lang="ts">
  import { commands } from "$lib/bindings";
  import {
    connectionStatuses,
    connectionStatusesListenerError,
  } from "$lib/listeners/connection-status";
  import { remotes, remotesListenerError } from "$lib/listeners/remotes";
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
    $remotes.find((remote) => remote.id === selectedId) ?? null,
  );

  $effect(() => register("network", { apply, reset }, { dirty, busy }));
  $effect(() => {
    if (selectedId && !$remotes.some((remote) => remote.id === selectedId))
      selectedId = null;
  });
  onMount(() => register("network", { apply, reset }, { dirty, busy }));

  function statusFor(remoteId: string) {
    return (
      $connectionStatuses.find((status) => status.remoteId === remoteId)
        ?.state ?? "disconnected"
    );
  }

  async function openAddActionWindow() {
    busy = true;
    error = "";
    try {
      await commands.openActionWindow(
        "network",
        "Add Server",
        "/control-panel/add/network",
      );
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function openEditWindow(id: string) {
    busy = true;
    error = "";
    try {
      await commands.openActionWindow(
        `network-edit-${id}`,
        "Edit Server",
        `/control-panel/edit/network/${encodeURIComponent(id)}`,
      );
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  function reset() {
    mode = "browse";
    error = "";
  }

  async function apply() {
    if (mode === "browse") return true;
    if (mode === "remove" && !selected) {
      error = "The selected server no longer exists.";
      return false;
    }

    busy = true;
    error = "";
    try {
      if (selected) {
        await commands.deleteRemote(selected.id);
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
  {#if error || $remotesListenerError || $connectionStatusesListenerError}
    <PanelMessage kind="error"
      >{error ||
        $remotesListenerError ||
        $connectionStatusesListenerError}</PanelMessage
    >
  {/if}

  {#if mode === "remove" && selected}
    <PanelMessage kind="warning">
      Applying will remove <strong>{selected.name ?? selected.address}</strong>.
      Existing connections through this server may stop.
    </PanelMessage>
    <button class="btn w-fit" type="button" onclick={() => (mode = "browse")}
      >Keep server</button
    >
  {:else}
    <div
      class="flex min-h-0 flex-1 flex-col border border-base-300 bg-base-100"
    >
      <div
        class="grid grid-cols-[1fr_5.5rem] border-b border-base-300 bg-base-200 px-2 py-1 text-[10px] font-bold uppercase tracking-wide text-base-content/60"
      >
        <span>Server</span><span>Connection</span>
      </div>
      <div
        class="min-h-0 flex-1 overflow-y-auto"
        role="listbox"
        aria-label="Configured servers"
      >
        {#if $remotes.length === 0}
          <div
            class="grid h-full min-h-36 place-content-center px-8 text-center"
          >
            <p class="text-xs font-bold">No servers configured</p>
            <p class="mt-1 text-xs text-base-content/60">
              Add a server to connect with friends.
            </p>
          </div>
        {:else}
          {#each $remotes as remote (remote.id)}
            {@const state = statusFor(remote.id)}
            <button
              type="button"
              role="option"
              aria-selected={selectedId === remote.id}
              class="grid w-full grid-cols-[1fr_5.5rem] items-center border-b border-base-200 px-2 py-1.5 text-left text-xs hover:bg-base-200 aria-selected:bg-primary aria-selected:text-primary-content"
              onclick={() => (selectedId = remote.id)}
              ondblclick={() => openEditWindow(remote.id)}
            >
              <span class="min-w-0">
                <strong class="block truncate"
                  >{remote.name ?? remote.address}</strong
                >
                <span class="block truncate text-[10px] opacity-65"
                  >{remote.address}{remote.port !== null
                    ? `:${remote.port}`
                    : ""}</span
                >
              </span>
              <span class="flex items-center gap-1.5 capitalize">
                <span
                  class:status-success={state === "connected"}
                  class:status-warning={state === "connecting"}
                  class="status shadow-none"
                ></span>
                {state}
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
      <div class="flex flex-row gap-1.5">
        <button
          class="btn join-item"
          type="button"
          disabled={!selected || busy}
          onclick={() => selected && openEditWindow(selected.id)}>Edit…</button
        >
        <button
          class="btn join-item"
          type="button"
          disabled={!selected}
          onclick={() => (mode = "remove")}>Remove</button
        >
      </div>
    </div>
  {/if}
</div>
