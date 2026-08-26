<script>
  import FriendsList from "$lib/components/debug/friends-list.svelte";
  import FriendStatuses from "$lib/components/debug/friend-statuses.svelte";
  import ConnectionStatus from "$lib/components/debug/connection-status.svelte";
  import Profile from "$lib/components/debug/profile.svelte";
  import RemotesList from "$lib/components/debug/remotes-list.svelte";
  import LiveMetadata from "$lib/components/debug/live-metadata.svelte";
</script>

<main>
  <div>
    <Profile />
  </div>
  <div>
    <FriendsList />
  </div>
  <div>
    <FriendStatuses />
  </div>
  <div>
    <RemotesList />
  </div>
  <div>
    <ConnectionStatus />
  </div>
  <div>
    <LiveMetadata />
  </div>
</main>
