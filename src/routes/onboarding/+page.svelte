<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy, onMount } from "svelte";
  import {
    commands,
    type PuppetMovementMode,
    type RemoteInput,
    type SceneConfiguration,
    type User,
  } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import CompleteStep from "./components/complete-step.svelte";
  import FriendStep from "./components/friend-step.svelte";
  import IdentityStep from "./components/identity-step.svelte";
  import MovementStep from "./components/movement-step.svelte";
  import ServerStep from "./components/server-step.svelte";
  import SkinStep from "./components/skin-step.svelte";
  import WelcomeStep from "./components/welcome-step.svelte";
  import { errorMessage, messages, type MessageCatalog } from "$lib/i18n";
  import {
    containsScheme,
    storedServerAddress,
    type ServerConnectionType,
  } from "$lib/server-endpoint";
  import { getVersion } from "@tauri-apps/api/app";

  type StepId =
    | "welcome"
    | "identity"
    | "skin"
    | "movement"
    | "server"
    | "friend"
    | "complete";

  const allSteps: Array<{
    id: StepId;
    title: (catalog: MessageCatalog) => string;
    navigationLabel: (catalog: MessageCatalog) => string;
  }> = [
    {
      id: "welcome",
      title: (catalog) => catalog.onboarding_welcome_title(),
      navigationLabel: (catalog) => catalog.onboarding_welcome_nav(),
    },
    {
      id: "identity",
      title: (catalog) => catalog.onboarding_identity_title(),
      navigationLabel: (catalog) => catalog.onboarding_identity_nav(),
    },
    {
      id: "skin",
      title: (catalog) => catalog.onboarding_skin_title(),
      navigationLabel: (catalog) => catalog.onboarding_skin_nav(),
    },
    {
      id: "movement",
      title: (catalog) => catalog.onboarding_movement_title(),
      navigationLabel: (catalog) => catalog.onboarding_movement_nav(),
    },
    {
      id: "server",
      title: (catalog) => catalog.onboarding_server_title(),
      navigationLabel: (catalog) => catalog.onboarding_server_nav(),
    },
    {
      id: "friend",
      title: (catalog) => catalog.onboarding_friend_title(),
      navigationLabel: (catalog) => catalog.onboarding_friend_nav(),
    },
    {
      id: "complete",
      title: (catalog) => catalog.onboarding_complete_title(),
      navigationLabel: (catalog) => catalog.onboarding_complete_nav(),
    },
  ];

  let profile = $state<User | null>(null);
  let configuration = $state<SceneConfiguration>({
    puppetScale: 1,
    puppetOpacity: 1,
    puppetMovementMode: "free",
    hideLocalPuppetWhenAlone: false,
  });
  let stepIndex = $state(0);
  let busy = $state(false);
  let error = $state("");

  let displayName = $state("");
  let skinMode = $state<"current" | "default" | "custom">("default");
  let skinFile = $state<File | null>(null);
  let skinPreviewUrl = $state<string | null>(null);
  let movementMode = $state<PuppetMovementMode>("free");
  let serverName = $state("");
  let serverAddress = $state("");
  let serverPort = $state("");
  let serverConnectionType = $state<ServerConnectionType>("https");
  let savedServerId = $state<string | null>(null);
  let friendId = $state("");
  let savedFriendId = $state<string | null>(null);

  let appVersion = $state("");

  onMount(() => {
    getVersion().then((v) => {
      appVersion = v;
    });
  });

  let steps = $derived(allSteps);
  let step = $derived(steps[stepIndex] ?? steps[0]);
  let isLastStep = $derived(stepIndex === steps.length - 1);

  $effect(() => {
    void getCurrentWindow().setTitle($messages.onboarding_window_title());
  });

  onMount(() => {
    let disposed = false;

    void (async () => {
      try {
        const [nextProfile, nextConfiguration] = await Promise.all([
          commands.getProfile(),
          commands.getSceneConfiguration(),
        ]);
        if (disposed) return;

        profile = nextProfile;
        configuration = nextConfiguration;
        displayName = nextProfile.displayNameConfigured
          ? nextProfile.displayName
          : "";
        skinMode = nextProfile.skinHash ? "current" : "default";
        movementMode = nextConfiguration.puppetMovementMode;

        const requestedStep = new URLSearchParams(window.location.search).get(
          "step",
        );
        if (requestedStep) {
          const requestedIndex = steps.findIndex(
            (candidate) => candidate.id === requestedStep,
          );
          if (requestedIndex >= 0) stepIndex = requestedIndex;
        }
      } catch (cause) {
        if (!disposed) error = errorMessage(cause);
      }
    })();

    return () => {
      disposed = true;
    };
  });

  onDestroy(() => {
    if (skinPreviewUrl) URL.revokeObjectURL(skinPreviewUrl);
  });

  function selectSkin(file: File | null) {
    if (skinPreviewUrl) URL.revokeObjectURL(skinPreviewUrl);
    skinFile = file;
    skinPreviewUrl = file ? URL.createObjectURL(file) : null;
    if (file) skinMode = "custom";
    error = "";
  }

  function remoteInput(): RemoteInput | null {
    if (!serverAddress.trim()) return null;
    if (containsScheme(serverAddress)) {
      throw new Error($messages.error_server_address_scheme());
    }
    const port = serverPort ? Number(serverPort) : null;
    if (
      port !== null &&
      (!Number.isInteger(port) || port < 1 || port > 65535)
    ) {
      throw new Error($messages.error_port_invalid());
    }
    return {
      address: storedServerAddress(serverAddress, serverConnectionType),
      name: serverName.trim() || null,
      port,
    };
  }

  async function saveCurrentStep() {
    if (step.id === "identity") {
      const nextName = displayName.trim();
      if (!nextName) throw new Error($messages.error_display_name_empty());
      profile = await commands.updateProfile(nextName, null);
      displayName = profile.displayName;
    } else if (step.id === "skin") {
      if (!profile) throw new Error($messages.error_profile_loading());
      if (skinMode === "default" && profile.skinHash) {
        profile = await commands.resetProfileSkin();
      } else if (skinMode === "custom" && skinFile) {
        const skinData = Array.from(
          new Uint8Array(await skinFile.arrayBuffer()),
        );
        profile = await commands.updateProfile(profile.displayName, skinData);
        selectSkin(null);
        skinMode = "current";
      } else if (skinMode === "custom") {
        throw new Error($messages.error_skin_required());
      }
    } else if (step.id === "movement") {
      configuration = await commands.updateSceneConfiguration({
        ...configuration,
        puppetMovementMode: movementMode,
      });
    } else if (step.id === "server") {
      const remote = remoteInput();
      if (remote) {
        if (savedServerId) {
          await commands.updateRemote(savedServerId, remote);
        } else {
          savedServerId = (await commands.createRemote(remote)).id;
        }
      }
    } else if (step.id === "friend") {
      const nextFriendId = friendId.trim();
      if (nextFriendId && nextFriendId !== savedFriendId) {
        const previousFriendId = savedFriendId;
        savedFriendId = (await commands.createFriend(nextFriendId)).id;
        if (previousFriendId) await commands.deleteFriend(previousFriendId);
      }
    }
  }

  async function next() {
    if (!step) return;
    busy = true;
    error = "";
    try {
      await saveCurrentStep();
      if (isLastStep) {
        await commands.completeOnboarding();
        await getCurrentWindow().close();
      } else {
        stepIndex += 1;
      }
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = false;
    }
  }

  function back() {
    if (stepIndex > 0 && !busy) {
      stepIndex -= 1;
      error = "";
    }
  }
</script>

<svelte:head><title>{$messages.onboarding_window_title()}</title></svelte:head>

<main class="flex h-full min-h-0 flex-col bg-base-200 text-base-content">
  <div class="grid min-h-0 flex-1 grid-cols-[11rem_minmax(0,1fr)]">
    <aside
      class="flex flex-col bg-linear-to-b from-primary to-primary/50 p-5 text-primary-content"
    >
      <div class="text-lg font-bold leading-tight">
        {$messages.onboarding_brand()}
      </div>
      <div class="mt-6 flex-1 space-y-2 text-[11px]">
        {#each steps as candidate, index (candidate.id)}
          <div
            class="flex items-center gap-2 opacity-60"
            class:font-bold={index === stepIndex}
            class:opacity-100={index <= stepIndex}
          >
            <span
              class="grid size-4 place-content-center border border-primary-content text-[9px]"
              class:bg-primary-content={index < stepIndex}
              class:text-primary={index < stepIndex}
            >
              {index < stepIndex ? "✓" : index + 1}
            </span>
            <span>{candidate.navigationLabel($messages)}</span>
          </div>
        {/each}
      </div>
      <p class="text-[10px] opacity-70">Friendolls v{appVersion}</p>
    </aside>

    <section class="flex min-h-0 flex-col bg-base-100">
      {#if step}
        <header class="bg-base-100 px-6 pt-4">
          <p class="font-bold text-2xl">{step.title($messages)}</p>
        </header>

        <div class="min-h-0 flex-1 overflow-y-auto p-6">
          {#if error}
            <div class="mb-3">
              <PanelMessage kind="error">{error}</PanelMessage>
            </div>
          {/if}

          {#if !profile}
            <div class="space-y-3" aria-label={$messages.onboarding_loading()}>
              <div class="skeleton h-5 w-2/3"></div>
              <div class="skeleton h-24 w-full"></div>
            </div>
          {:else if step.id === "welcome"}
            <WelcomeStep />
          {:else if step.id === "identity"}
            <IdentityStep bind:displayName {busy} />
          {:else if step.id === "skin"}
            <SkinStep
              {profile}
              {configuration}
              bind:skinMode
              {skinPreviewUrl}
              {busy}
              onselect={selectSkin}
            />
          {:else if step.id === "movement"}
            <MovementStep bind:movementMode {busy} />
          {:else if step.id === "server"}
            <ServerStep
              bind:serverName
              bind:serverAddress
              bind:serverPort
              bind:serverConnectionType
              {busy}
            />
          {:else if step.id === "friend"}
            <FriendStep bind:friendId {busy} />
          {:else if step.id === "complete"}
            <CompleteStep />
          {/if}
        </div>

        <footer
          class="flex shrink-0 items-center justify-between border-t border-base-300 bg-base-200 px-4 py-3"
        >
          <span class="text-[10px] text-base-content/60">
            {$messages.onboarding_step_count({
              current: stepIndex + 1,
              total: steps.length,
            })}
          </span>
          <div class="flex justify-end gap-2">
            <button
              class="btn min-w-20"
              type="button"
              disabled={busy || stepIndex === 0}
              onclick={back}>{$messages.onboarding_back()}</button
            >
            <button
              class="btn min-w-20"
              type="button"
              disabled={busy}
              onclick={next}
            >
              {busy
                ? $messages.onboarding_working()
                : isLastStep
                  ? $messages.onboarding_finish()
                  : $messages.onboarding_next()}
            </button>
          </div>
        </footer>
      {/if}
    </section>
  </div>
</main>
