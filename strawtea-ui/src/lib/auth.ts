import { createClient, type Session, type SupabaseClient } from '@supabase/supabase-js';
import { writable } from 'svelte/store';

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL;
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY;
export const hasAuthConfig = Boolean(supabaseUrl && supabaseAnonKey);

export const supabase: SupabaseClient | null = hasAuthConfig
  ? createClient(supabaseUrl, supabaseAnonKey)
  : null;

type AuthState =
  | { status: 'loading'; session: null; user: null }
  | { status: 'signed-out'; session: null; user: null }
  | { status: 'signed-in'; session: Session; user: { email: string } };

export const auth = writable<AuthState>({
  status: 'loading',
  session: null,
  user: null
});

export function initAuth() {
  if (!supabase) {
    auth.set({ status: 'signed-out', session: null, user: null });
    return;
  }

  supabase.auth.getSession().then(({ data }) => {
    setSession(data.session);
  });

  supabase.auth.onAuthStateChange((_event, session) => {
    setSession(session);
  });
}

export async function signInWithGoogle() {
  if (!supabase) {
    throw new Error('Supabase env vars are not configured');
  }

  await supabase.auth.signInWithOAuth({
    provider: 'google',
    options: {
      redirectTo: window.location.origin
    }
  });
}

export async function signOut() {
  if (!supabase) {
    auth.set({ status: 'signed-out', session: null, user: null });
    return;
  }

  await supabase.auth.signOut();
}

export async function accessToken(): Promise<string> {
  if (!supabase) {
    throw new Error('Supabase env vars are not configured');
  }

  const { data } = await supabase.auth.getSession();
  const token = data.session?.access_token;

  if (!token) {
    throw new Error('Not signed in');
  }

  return token;
}

function setSession(session: Session | null) {
  if (!session) {
    auth.set({ status: 'signed-out', session: null, user: null });
    return;
  }

  auth.set({
    status: 'signed-in',
    session,
    user: {
      email: session.user.email ?? 'Signed in'
    }
  });
}
