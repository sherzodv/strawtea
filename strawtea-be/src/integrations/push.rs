use std::sync::Arc;

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use web_push::{
    ContentEncoding, IsahcWebPushClient, SubscriptionInfo, VapidSignatureBuilder, WebPushClient,
    WebPushMessageBuilder,
};

#[derive(Clone)]
pub struct PushNotifications {
    public_key: String,
    private_key: Arc<String>,
    subject: Arc<String>,
    client: Arc<IsahcWebPushClient>,
}

#[derive(Debug, Serialize)]
pub struct PriceAlertPayload<'a> {
    pub title: &'a str,
    pub body: String,
    pub url: &'a str,
    pub ticker: &'a str,
    pub threshold_percent: i32,
    pub percent_change: f64,
    pub current_price: i64,
    pub avg_buy_price: i64,
}

impl PushNotifications {
    pub fn new(private_key: String, subject: String) -> Result<Self> {
        let vapid_builder = VapidSignatureBuilder::from_pem_no_sub(private_key.as_bytes())?;
        let public_key = URL_SAFE_NO_PAD.encode(vapid_builder.get_public_key());

        Ok(Self {
            public_key,
            private_key: Arc::new(private_key),
            subject: Arc::new(subject),
            client: Arc::new(IsahcWebPushClient::new()?),
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub async fn send_price_alert(
        &self,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        payload: &PriceAlertPayload<'_>,
    ) -> Result<()> {
        let subscription_info = SubscriptionInfo::new(endpoint, p256dh, auth);
        let mut vapid_signature =
            VapidSignatureBuilder::from_pem(self.private_key.as_bytes(), &subscription_info)?;
        vapid_signature.add_claim("sub", self.subject.as_str());

        let content = serde_json::to_vec(payload)?;
        let mut builder = WebPushMessageBuilder::new(&subscription_info);
        builder.set_payload(ContentEncoding::Aes128Gcm, &content);
        builder.set_vapid_signature(vapid_signature.build()?);

        self.client.send(builder.build()?).await?;

        Ok(())
    }
}
