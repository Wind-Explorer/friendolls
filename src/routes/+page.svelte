<script>
  import FriendsList from "$lib/components/friends-list.svelte";
  import ConnectionStatus from "$lib/components/connection-status.svelte";
  import Profile from "$lib/components/profile.svelte";
  import RemotesList from "$lib/components/remotes-list.svelte";
</script>

<main>
  <div>
    <Profile />
  </div>
  <div>
    <FriendsList />
  </div>
  <div>
    <RemotesList />
  </div>
  <div>
    <ConnectionStatus />
  </div>
</main>
