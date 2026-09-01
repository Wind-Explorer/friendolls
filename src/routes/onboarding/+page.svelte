<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onDestroy, onMount } from "svelte";
  import {
    commands,
    events,
    type OnboardingStatus,
    type PuppetMovementMode,
    type RemoteInput,
    type SceneConfiguration,
    type User,
  } from "$lib/bindings";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import AccessibilityStep from "./components/accessibility-step.svelte";
  import CompleteStep from "./components/complete-step.svelte";
  import FriendStep from "./components/friend-step.svelte";
  import IdentityStep from "./components/identity-step.svelte";
  import MovementStep from "./components/movement-step.svelte";
  import ServerStep from "./components/server-step.svelte";
  import SkinStep from "./components/skin-step.svelte";
  import WelcomeStep from "./components/welcome-step.svelte";

  type StepId =
    | "welcome"
    | "identity"
    | "skin"
    | "movement"
    | "server"
    | "friend"
    | "accessibility"
    | "complete";

  const allSteps: Array<{
    id: StepId;
    title: string;
  }> = [
    {
      id: "welcome",
      title: "Welcome to Friendolls!",
    },
    {
      id: "identity",
      title: "Choose your Display Name",
    },
    {
      id: "skin",
      title: "Choose your Puppet's Skin",
    },
    {
      id: "movement",
      title: "Choose your Movement Mode",
    },
    {
      id: "server",
      title: "Add a server",
    },
    {
      id: "friend",
      title: "Add a friend",
    },
    {
      id: "accessibility",
      title: "Allow cursor access",
    },
    {
      id: "complete",
      title: "Setup is complete",
    },
  ];

  let status = $state<OnboardingStatus | null>(null);
  let profile = $state<User | null>(null);
  let configuration = $state<SceneConfiguration>({
    puppetScale: 1,
    puppetOpacity: 1,
    puppetMovementMode: "free",
  });
  let remediation = $state(false);
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
  let savedServerId = $state<string | null>(null);
  let friendId = $state("");
  let savedFriendId = $state<string | null>(null);

  let steps = $derived(
    remediation
      ? allSteps.filter((candidate) => candidate.id === "accessibility")
      : allSteps.filter(
          (candidate) =>
            candidate.id !== "accessibility" ||
            status?.requiresAccessibilityPermission,
        ),
  );
  let step = $derived(steps[stepIndex] ?? steps[0]);
  let permissionGranted = $derived(
    status?.macosAccessibilityPermissionGranted ?? false,
  );
  let isLastStep = $derived(stepIndex === steps.length - 1);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;

    void (async () => {
      try {
        unlisten = await events.onboardingStatus.listen((event) => {
          status = event.payload;
        });
        const [nextStatus, nextProfile, nextConfiguration] = await Promise.all([
          commands.getOnboardingStatus(),
          commands.getProfile(),
          commands.getSceneConfiguration(),
        ]);
        if (disposed) return;

        status = nextStatus;
        profile = nextProfile;
        configuration = nextConfiguration;
        displayName =
          nextProfile.displayName === "Anonymous"
            ? ""
            : nextProfile.displayName;
        skinMode = nextProfile.skinHash ? "current" : "default";
        movementMode = nextConfiguration.puppetMovementMode;

        const requestedStep = new URLSearchParams(window.location.search).get(
          "step",
        );
        remediation =
          requestedStep === "accessibility" && nextStatus.onboardingDone;
        if (!remediation && requestedStep) {
          const requestedIndex = steps.findIndex(
            (candidate) => candidate.id === requestedStep,
          );
          if (requestedIndex >= 0) stepIndex = requestedIndex;
        }
      } catch (cause) {
        if (!disposed) error = String(cause);
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
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
    const port = serverPort ? Number(serverPort) : null;
    if (
      port !== null &&
      (!Number.isInteger(port) || port < 1 || port > 65535)
    ) {
      throw new Error("Port must be a whole number from 1 to 65535.");
    }
    return {
      address: serverAddress.trim(),
      name: serverName.trim() || null,
      port,
    };
  }

  async function saveCurrentStep() {
    if (step.id === "identity") {
      const nextName = displayName.trim();
      if (!nextName) throw new Error("Display name cannot be empty.");
      profile = await commands.updateProfile(nextName, null);
      displayName = profile.displayName;
    } else if (step.id === "skin") {
      if (!profile) throw new Error("Your profile is still loading.");
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
        throw new Error("Choose a 64×64 PNG skin before continuing.");
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
    } else if (step.id === "accessibility" && !permissionGranted) {
      throw new Error(
        "Turn on Friendolls in macOS Accessibility settings before continuing.",
      );
    }
  }

  async function next() {
    if (!step) return;
    busy = true;
    error = "";
    try {
      await saveCurrentStep();
      if (remediation) {
        await getCurrentWindow().close();
      } else if (isLastStep) {
        await commands.completeOnboarding();
        await getCurrentWindow().close();
      } else {
        stepIndex += 1;
      }
    } catch (cause) {
      error = String(cause);
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

  async function requestAccessibilityPermission() {
    busy = true;
    error = "";
    try {
      const granted = await commands.requestAccessibilityPermission();
      if (!granted) {
        error =
          "macOS opened Accessibility settings. Enable Friendolls there; this page will confirm automatically.";
      }
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:head><title>Friendolls Setup</title></svelte:head>

<main class="flex h-full min-h-0 flex-col bg-base-200 text-base-content">
  <div class="grid min-h-0 flex-1 grid-cols-[11rem_minmax(0,1fr)]">
    <aside
      class="flex flex-col bg-linear-to-b from-primary to-primary/50 p-5 text-primary-content"
    >
      <div class="text-lg font-bold leading-tight">Friendolls<br />Setup</div>
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
            <span>{candidate.title.replace("Choose your ", "")}</span>
          </div>
        {/each}
      </div>
      <p class="text-[10px] opacity-70">Friendolls 0.1</p>
    </aside>

    <section class="flex min-h-0 flex-col bg-base-100">
      {#if step}
        <header class="bg-base-100 px-6 pt-4">
          <p class="font-bold text-2xl">{step.title}</p>
          <!-- <p class="text-xs text-base-content/65">{step.description}</p> -->
        </header>

        <div class="min-h-0 flex-1 overflow-y-auto p-6">
          {#if error}
            <div class="mb-3">
              <PanelMessage
                kind={step.id === "accessibility" && !permissionGranted
                  ? "warning"
                  : "error"}>{error}</PanelMessage
              >
            </div>
          {/if}

          {#if !status || !profile}
            <div class="space-y-3" aria-label="Loading setup">
              <div class="skeleton h-5 w-2/3"></div>
              <div class="skeleton h-24 w-full"></div>
            </div>
          {:else if step.id === "welcome"}
            <WelcomeStep
              requiresAccessibilityPermission={status.requiresAccessibilityPermission}
            />
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
              {busy}
            />
          {:else if step.id === "friend"}
            <FriendStep bind:friendId {busy} />
          {:else if step.id === "accessibility"}
            <AccessibilityStep
              {permissionGranted}
              {busy}
              onrequest={requestAccessibilityPermission}
            />
          {:else if step.id === "complete"}
            <CompleteStep />
          {/if}
        </div>

        <footer
          class="flex shrink-0 items-center justify-between border-t border-base-300 bg-base-200 px-4 py-3"
        >
          <span class="text-[10px] text-base-content/60">
            {remediation
              ? "Permission repair"
              : `Step ${stepIndex + 1} of ${steps.length}`}
          </span>
          <div class="flex justify-end gap-2">
            {#if !remediation}
              <button
                class="btn min-w-20"
                type="button"
                disabled={busy || stepIndex === 0}
                onclick={back}>&lt; Back</button
              >
            {/if}
            <button
              class="btn min-w-20"
              type="button"
              disabled={busy ||
                (step.id === "accessibility" && !permissionGranted)}
              onclick={next}
            >
              {busy
                ? "Working…"
                : remediation
                  ? "Done"
                  : isLastStep
                    ? "Finish"
                    : "Next >"}
            </button>
          </div>
        </footer>
      {/if}
    </section>
  </div>
</main>
