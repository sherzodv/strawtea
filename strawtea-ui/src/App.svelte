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

  let isAccountModalOpen = false;

  $: isReady = $auth.status !== 'loading';
  $: isSignedIn = $auth.status === 'signed-in';
  $: pageTitle = titleForPath($route.path);
  $: pageEyebrow = $route.path === '/' ? 'Strawtea' : 'Strawtea';
  $: avatarText = ($auth.user?.email ?? 'S').trim().slice(0, 1).toUpperCase();

  function titleForPath(path: string) {
    if (path === '/investlog') {
      return 'Investlog';
    }

    if (path === '/spends') {
      return 'Spends';
    }

    if (path === '/stocks') {
      return 'Market lookup';
    }

    if (path === '/') {
      return 'Home';
    }

    return 'Not found';
  }
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
        <p class="stea-eyebrow">{pageEyebrow}</p>
        <h1 class="stea-heading">{pageTitle}</h1>
      </div>
      <button class="stea-avatar-btn" type="button" aria-label="Account" on:click={() => (isAccountModalOpen = true)}>
        {avatarText}
      </button>
    </header>

    {#if isAccountModalOpen}
      <div class="stea-modal-backdrop" role="presentation" on:pointerdown={() => (isAccountModalOpen = false)}>
        <div class="stea-account-modal" role="dialog" aria-modal="true" aria-labelledby="account-modal-title" tabindex="-1" on:pointerdown|stopPropagation>
          <div class="stea-modal-header">
            <div>
              <p class="stea-eyebrow">Strawtea</p>
              <h2 id="account-modal-title" class="stea-heading-sm">Account</h2>
            </div>
            <button class="stea-icon-btn" type="button" aria-label="Close" on:click={() => (isAccountModalOpen = false)}>×</button>
          </div>
          <dl class="stea-account-details">
            <div>
              <dt>App</dt>
              <dd>Strawtea</dd>
            </div>
            <div>
              <dt>Email</dt>
              <dd>{$auth.user?.email}</dd>
            </div>
          </dl>
          <div class="stea-modal-actions">
            <button class="stea-btn-secondary stea-btn-fit" type="button" on:click={signOut}>
              <LogOut size={18} />
              Exit
            </button>
          </div>
        </div>
      </div>
    {/if}

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
