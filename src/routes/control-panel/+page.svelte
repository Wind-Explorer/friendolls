<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import FriendsPanel from "$lib/components/control-panel/friends-panel.svelte";
  import GeneralPanel from "$lib/components/control-panel/general-panel.svelte";
  import NetworkPanel from "$lib/components/control-panel/network-panel.svelte";
  import ScenePanel from "$lib/components/control-panel/scene-panel.svelte";
  import type {
    PanelActions,
    PanelState,
  } from "$lib/components/control-panel/types";
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";

  const tabs = [
    { id: "scene", label: "Scene" },
    { id: "general", label: "Account" },
    { id: "friends", label: "Friends" },
    { id: "network", label: "Network" },
  ] as const;
  type TabId = (typeof tabs)[number]["id"];

  let activeTab = $state<TabId>("scene");
  let actions: Partial<Record<TabId, PanelActions>> = {};
  let panelStates = $state<Record<TabId, PanelState>>({
    general: { dirty: false, busy: false },
    scene: { dirty: false, busy: false },
    friends: { dirty: false, busy: false },
    network: { dirty: false, busy: false },
  });
  let anyDirty = $derived(tabs.some((tab) => panelStates[tab.id].dirty));
  let anyBusy = $derived(tabs.some((tab) => panelStates[tab.id].busy));
  let appVersion = $state("");

  onMount(() => {
    getVersion().then((v) => {
      appVersion = v;
    });
  });

  function register(
    name: string,
    nextActions: PanelActions,
    state: PanelState,
  ) {
    const id = name as TabId;
    actions[id] = nextActions;
    if (
      panelStates[id].dirty !== state.dirty ||
      panelStates[id].busy !== state.busy
    ) {
      panelStates[id] = state;
    }
  }

  async function applyAll() {
    for (const tab of tabs) {
      if (!panelStates[tab.id].dirty) continue;
      if (!(await actions[tab.id]?.apply())) {
        activeTab = tab.id;
        return false;
      }
    }
    return true;
  }

  async function hideWindow() {
    try {
      await getCurrentWindow().hide();
    } catch (error) {
      console.error("failed to hide control panel", error);
    }
  }

  async function confirm() {
    if (await applyAll()) await hideWindow();
  }

  function cancel() {
    tabs.forEach((tab) => actions[tab.id]?.reset());
    void hideWindow();
  }
</script>

<svelte:head><title>Friendolls Properties</title></svelte:head>

<main
  class="relative flex h-full min-h-0 flex-col overflow-hidden bg-base-100 p-2 text-base-content"
>
  <div class="absolute right-0 top-0">
    <p class="text-xs text-base-content/50 p-3">v{appVersion}</p>
  </div>
  <div class="tabs tabs-lift tabs-sm min-h-0 flex-1">
    {#each tabs as tab}
      <input
        id={`${tab.id}-tab`}
        type="radio"
        name="control_panel_tabs"
        class="tab text-xs"
        aria-label={tab.label}
        value={tab.id}
        bind:group={activeTab}
      />
      <div
        class="tab-content h-[calc(100%-1.3rem)] w-full overflow-hidden border border-base-300 bg-base-100 p-3"
        role="tabpanel"
        aria-labelledby={`${tab.id}-tab`}
      >
        <div class="h-full">
          {#if tab.id === "general"}
            <GeneralPanel {register} />
          {:else if tab.id === "scene"}
            <ScenePanel {register} />
          {:else if tab.id === "friends"}
            <FriendsPanel {register} />
          {:else if tab.id === "network"}
            <NetworkPanel {register} />
          {:else}
            <p>You're not supposed to see this 👀</p>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <div class="flex shrink-0 justify-end gap-1.5 pt-2">
    <button
      class="btn min-w-16"
      type="button"
      disabled={anyBusy}
      onclick={confirm}>OK</button
    >
    <button
      class="btn min-w-16"
      type="button"
      disabled={anyBusy}
      onclick={cancel}>Cancel</button
    >
    <button
      class="btn min-w-16"
      type="button"
      disabled={!anyDirty || anyBusy}
      onclick={applyAll}>{anyBusy ? "Applying…" : "Apply"}</button
    >
  </div>
</main>
