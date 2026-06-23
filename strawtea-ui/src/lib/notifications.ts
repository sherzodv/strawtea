import {
  fetchPushPublicKey,
  savePushSubscription,
  type PushSubscriptionPayload
} from './api';

type PushSubscriptionJson = {
  endpoint?: string;
  keys?: {
    p256dh?: string;
    auth?: string;
  };
};

export function isPushNotificationSupported() {
  return (
    'Notification' in window &&
    'serviceWorker' in navigator &&
    'PushManager' in window
  );
}

export async function enableAssetPriceNotifications() {
  if (!isPushNotificationSupported()) {
    throw new Error('Push notifications are not supported in this browser');
  }

  const keyResponse = await fetchPushPublicKey();
  if (!keyResponse.enabled || !keyResponse.public_key) {
    throw new Error('Push notifications are not configured on the server');
  }

  const permission = await Notification.requestPermission();
  if (permission !== 'granted') {
    throw new Error('Notification permission was not granted');
  }

  const registration = await navigator.serviceWorker.ready;
  const existingSubscription = await registration.pushManager.getSubscription();
  const subscription =
    existingSubscription ??
    (await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(keyResponse.public_key)
    }));

  await savePushSubscription(subscriptionToPayload(subscription));
}

function subscriptionToPayload(subscription: PushSubscription): PushSubscriptionPayload {
  const json = subscription.toJSON() as PushSubscriptionJson;
  const endpoint = json.endpoint ?? subscription.endpoint;
  const p256dh = json.keys?.p256dh;
  const auth = json.keys?.auth;

  if (!endpoint || !p256dh || !auth) {
    throw new Error('Browser returned an incomplete push subscription');
  }

  return {
    endpoint,
    keys: {
      p256dh,
      auth
    }
  };
}

function urlBase64ToUint8Array(value: string) {
  const padding = '='.repeat((4 - (value.length % 4)) % 4);
  const base64 = `${value}${padding}`.replace(/-/g, '+').replace(/_/g, '/');
  const raw = window.atob(base64);
  const output = new Uint8Array(raw.length);

  for (let index = 0; index < raw.length; index += 1) {
    output[index] = raw.charCodeAt(index);
  }

  return output;
}
