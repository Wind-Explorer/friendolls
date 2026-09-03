<script lang="ts">
  import {
    commands,
    type AppMeta,
    type InteractionContent,
  } from "$lib/bindings";
  import { friendName, friends } from "$lib/listeners/friends";
  import { profile } from "$lib/listeners/profile";
  import { errorMessage, messages } from "$lib/i18n";

  type Props = {
    titleId: string;
    userId: string;
    isLocal: boolean;
    foregroundApp?: AppMeta;
    onModeChange: (active: boolean) => void;
    onDismiss: () => void;
    onSent: () => void;
  };

  let {
    titleId,
    userId,
    isLocal,
    foregroundApp,
    onModeChange,
    onDismiss,
    onSent,
  }: Props = $props();
  let mode = $state<"message" | "image" | null>(null);
  let message = $state("");
  let error = $state("");
  let sending = $state(false);
  let username = $derived(
    $profile?.id === userId
      ? $profile.displayName
      : friendName(
          $friends.find((friend) => friend.id === userId),
          $messages.common_unknown_user(),
        ),
  );

  function selectMode(nextMode: "message" | "image") {
    mode = mode === nextMode ? null : nextMode;
    onModeChange(mode !== null);
    error = "";
  }

  async function sendContent(content: InteractionContent) {
    sending = true;
    error = "";
    try {
      await commands.sendInteraction(userId, content);
      message = "";
      mode = null;
      onSent();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      sending = false;
    }
  }

  async function sendMessage() {
    const text = message.trim();
    if (text) await sendContent({ type: "text", text });
  }

  async function sendImage() {
    await runImageSend(() => commands.pickAndSendImage(userId));
  }

  async function pasteImage(event: ClipboardEvent) {
    if (mode !== "image" || sending) return;
    const file = Array.from(event.clipboardData?.items ?? [])
      .find((item) => item.type.startsWith("image/"))
      ?.getAsFile();
    if (!file) return;

    event.preventDefault();
    await runImageSend(async () => {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      await commands.sendImageBytes(userId, bytes);
      return true;
    });
  }

  async function runImageSend(send: () => Promise<boolean>) {
    if (sending) return;
    sending = true;
    error = "";
    try {
      const sent = await send();
      if (sent) {
        mode = null;
        onSent();
      }
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      sending = false;
    }
  }
</script>

<article class="w-68 overflow-hidden text-base-content" onpaste={pasteImage}>
  <header
    class="ml-2 flex w-max items-center gap-1 border border-b-0 border-base-300 bg-base-200 px-2 py-0.5"
  >
    <p id={titleId} class="max-w-52 truncate text-xs">{username}</p>
  </header>

  <div
    class="relative flex flex-col gap-3 border border-base-200 bg-base-100 p-2 text-sm shadow-lg"
  >
    {#if mode !== null}
      <button
        class="absolute right-1 top-1 grid size-6 place-items-center text-lg leading-none btn btn-square btn-ghost"
        aria-label={$messages.scene_close_interaction()}
        onclick={onDismiss}
      >
        ×
      </button>
    {/if}

    <section>
      <h3 class="text-xs text-base-content/50">{$messages.scene_currently_enjoying()}</h3>
      <div class="mt-1 flex min-w-0 items-center gap-2">
        {#if foregroundApp?.ico}
          <img
            src={`data:image/png;base64,${foregroundApp.ico}`}
            alt=""
            class="size-6 shrink-0 object-contain"
          />
        {/if}
        <div class="min-w-0">
          <p class="truncate text-sm">
            {foregroundApp?.local ??
              foregroundApp?.unlocal ??
              $messages.scene_waiting_data()}
          </p>
          {#if foregroundApp?.local && foregroundApp.unlocal}
            <p class="truncate text-xs text-base-content/50">
              {foregroundApp.unlocal}
            </p>
          {/if}
        </div>
      </div>
    </section>

    {#if !isLocal}
      <section class="border-t border-base-200 pt-2 w-full">
        <div class="grid grid-cols-3 gap-1">
          <button
            type="button"
            class:!bg-primary={mode === "message"}
            class:!text-primary-content={mode === "message"}
            class="btn btn-ghost btn-xs"
            aria-pressed={mode === "message"}
            onclick={() => selectMode("message")}
          >
            {$messages.scene_message()}
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs"
            disabled={sending}
            onclick={() => sendContent({ type: "wave" })}
          >
            {$messages.scene_wave()}
          </button>
          <button
            type="button"
            class:!bg-primary={mode === "image"}
            class:!text-primary-content={mode === "image"}
            class="btn btn-ghost btn-xs"
            aria-pressed={mode === "image"}
            onclick={() => selectMode("image")}
          >
            {$messages.scene_image()}
          </button>
        </div>

        {#if mode === "message"}
          <form
            class="mt-2 grid gap-1 border-l-2 border-primary bg-base-200/55 p-2"
            onsubmit={(event) => {
              event.preventDefault();
              void sendMessage();
            }}
          >
            <textarea
              bind:value={message}
              maxlength="500"
              rows="3"
              class="textarea textarea-xs w-full resize-none bg-base-100"
              placeholder={$messages.scene_write_to({ username })}
              aria-label={$messages.scene_message_user({ username })}></textarea>
            <div class="flex items-center justify-between gap-2">
              <span class="text-[0.6rem] text-base-content/45">
                {message.length}/500
              </span>
              <button
                type="submit"
                class="btn btn-primary btn-xs"
                disabled={sending || !message.trim()}
              >
                {sending ? $messages.scene_sending() : $messages.scene_send()}
              </button>
            </div>
          </form>
        {:else if mode === "image"}
          <div
            class="mt-2 grid min-h-24 place-items-center border border-primary/60 bg-primary/5 p-2 text-center"
          >
            <div>
              <p class="text-xs">{$messages.scene_choose_or_paste()}</p>
              <p class="mt-0.5 text-[0.6rem] text-base-content/45">
                {$messages.scene_compression_note()}
              </p>
              <button
                type="button"
                class="btn btn-primary btn-xs mt-2"
                disabled={sending}
                onclick={sendImage}
              >
                {sending
                  ? $messages.scene_compressing()
                  : $messages.scene_choose_image()}
              </button>
            </div>
          </div>
        {/if}

        {#if error}
          <p class="mt-2 text-xs text-error" role="alert">{error}</p>
        {/if}
      </section>
    {/if}
  </div>
</article>
