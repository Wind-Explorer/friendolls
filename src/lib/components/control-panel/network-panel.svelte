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
  import { errorMessage, messages } from "$lib/i18n";

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
        $messages.action_add_server_title(),
        "/control-panel/add/network",
      );
    } catch (cause) {
      error = errorMessage(cause);
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
        $messages.action_edit_server_title(),
        `/control-panel/edit/network/${encodeURIComponent(id)}`,
      );
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = false;
    }
  }

  async function moveRemote(id: string, offset: -1 | 1) {
    const index = $remotes.findIndex((remote) => remote.id === id);
    const target = index + offset;
    if (index < 0 || target < 0 || target >= $remotes.length) return;

    const ordered = [...$remotes];
    [ordered[index], ordered[target]] = [ordered[target], ordered[index]];
    busy = true;
    error = "";
    try {
      await commands.reorderRemotes(ordered.map((remote) => remote.id));
    } catch (cause) {
      error = errorMessage(cause);
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
      error = $messages.error_server_missing();
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
      error = errorMessage(cause);
      return false;
    } finally {
      busy = false;
    }
  }
</script>

<div class="flex h-full min-h-0 flex-col gap-2">
  {#if error || $remotesListenerError || $connectionStatusesListenerError}
    <PanelMessage kind="error"
      >{errorMessage(
        error || $remotesListenerError || $connectionStatusesListenerError,
      )}</PanelMessage
    >
  {/if}

  {#if mode === "remove" && selected}
    <PanelMessage kind="warning">
      {$messages.network_remove_prefix()}<strong
        >{selected.name ?? selected.address}</strong
      >{$messages.network_existing_connections_stop()}
    </PanelMessage>
    <button class="btn w-fit" type="button" onclick={() => (mode = "browse")}
      >{$messages.network_keep()}</button
    >
  {:else}
    <div
      class="flex min-h-0 flex-1 flex-col border border-base-300 bg-base-100"
    >
      <div
        class="grid grid-cols-[1fr_5.5rem_3.5rem] border-b border-base-300 bg-base-200 px-2 py-1 text-[10px] font-bold uppercase tracking-wide text-base-content/60"
      >
        <span>{$messages.network_server()}</span><span
          >{$messages.network_connection()}</span
        ><span>{$messages.network_priority()}</span>
      </div>
      <div
        class="min-h-0 flex-1 overflow-y-auto"
        role="group"
        aria-label={$messages.network_configured_servers()}
      >
        {#if $remotes.length === 0}
          <div
            class="grid h-full min-h-36 place-content-center px-8 text-center"
          >
            <p class="text-xs font-bold">{$messages.network_no_servers()}</p>
            <p class="mt-1 text-xs text-base-content/60">
              {$messages.network_empty_help()}
            </p>
          </div>
        {:else}
          {#each $remotes as remote, index (remote.id)}
            {@const state = statusFor(remote.id)}
            <div
              class="grid w-full grid-cols-[1fr_5.5rem_3.5rem] items-center border-b border-base-200 px-2 py-1 text-xs hover:bg-base-200"
              class:bg-primary={selectedId === remote.id}
              class:text-primary-content={selectedId === remote.id}
            >
              <button
                type="button"
                class="min-w-0 py-0.5 text-left"
                aria-pressed={selectedId === remote.id}
                onclick={() => (selectedId = remote.id)}
                ondblclick={() => openEditWindow(remote.id)}
              >
                <strong class="block truncate"
                  >{remote.name ?? remote.address}</strong
                >
                <span class="block truncate text-[10px] opacity-65"
                  >{remote.address}{remote.port !== null
                    ? `:${remote.port}`
                    : ""}</span
                >
              </button>
              <span class="flex items-center gap-1.5 capitalize">
                <span
                  class:status-success={state === "connected"}
                  class:status-warning={state === "connecting"}
                  class="status shadow-none"
                ></span>
                {state === "connected"
                  ? $messages.network_connected()
                  : state === "connecting"
                    ? $messages.network_connecting()
                    : $messages.network_disconnected()}
              </span>
              <span class="flex justify-end gap-0.5">
                <button
                  class="btn btn-xs px-1"
                  type="button"
                  aria-label={`${$messages.network_move_up()}: ${remote.name ?? remote.address}`}
                  title={$messages.network_move_up()}
                  disabled={busy || index === 0}
                  onclick={() => moveRemote(remote.id, -1)}>↑</button
                >
                <button
                  class="btn btn-xs px-1"
                  type="button"
                  aria-label={`${$messages.network_move_down()}: ${remote.name ?? remote.address}`}
                  title={$messages.network_move_down()}
                  disabled={busy || index === $remotes.length - 1}
                  onclick={() => moveRemote(remote.id, 1)}>↓</button
                >
              </span>
            </div>
          {/each}
        {/if}
      </div>
    </div>

    <div class="flex justify-between gap-2">
      <button
        class="btn"
        type="button"
        disabled={busy}
        onclick={openAddActionWindow}>{$messages.common_add()}</button
      >
      <div class="flex flex-row gap-1.5">
        <button
          class="btn join-item"
          type="button"
          disabled={!selected || busy}
          onclick={() => selected && openEditWindow(selected.id)}>{$messages.common_edit()}</button
        >
        <button
          class="btn join-item"
          type="button"
          disabled={!selected}
          onclick={() => (mode = "remove")}>{$messages.common_remove()}</button
        >
      </div>
    </div>
  {/if}
</div>
