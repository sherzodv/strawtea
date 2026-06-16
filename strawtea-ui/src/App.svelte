<script lang="ts">
  import { onMount } from 'svelte';
  import { LogOut } from '@lucide/svelte';
  import HomePage from './features/home/HomePage.svelte';
  import InvestlogPage from './features/investlog/InvestlogPage.svelte';
  import LoginPage from './features/auth/LoginPage.svelte';
  import SpendsPage from './features/spends/SpendsPage.svelte';
  import { auth, initAuth, signOut } from './lib/auth';
  import { route, startRouter } from './lib/router';

  onMount(() => {
    initAuth();
    startRouter();
  });

  $: isReady = $auth.status !== 'loading';
  $: isSignedIn = $auth.status === 'signed-in';
</script>

{#if !isReady}
  <main class="stea-shell stea-center">
    <p class="stea-muted">Loading</p>
  </main>
{:else if !isSignedIn}
  <LoginPage />
{:else}
  <main class="stea-shell">
    <header class="stea-topbar">
      <div>
        <p class="stea-eyebrow">Strawtea</p>
        <p class="stea-text">{$auth.user?.email}</p>
      </div>
      <button class="stea-icon-btn" type="button" aria-label="Sign out" on:click={signOut}>
        <LogOut size={20} />
      </button>
    </header>

    {#if $route.path === '/'}
      <HomePage />
    {:else if $route.path === '/investlog'}
      <InvestlogPage />
    {:else if $route.path === '/spends'}
      <SpendsPage />
    {:else}
      <section class="stea-empty">
        <h1>Not found</h1>
        <a href="/" on:click|preventDefault={() => route.navigate('/')}>Back home</a>
      </section>
    {/if}
  </main>
{/if}
