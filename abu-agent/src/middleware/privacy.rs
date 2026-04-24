use abu_base::chat::AssistantMessage;
use regex::Regex;
use std::convert::Infallible;

use super::{LlmOutMiddleware, MiddlewareFlow};

pub struct PrivacyScrubberMiddleware {
    phone_regex: Regex,
    email_regex: Regex,
}

impl PrivacyScrubberMiddleware {
    pub fn new() -> Self {
        Self {
            // TODO
            phone_regex: Regex::new(r"1[3-9]\d{9}").unwrap(),
            email_regex: Regex::new(r"[a-zA-Z0-9_-]+@[a-zA-Z0-9_-]+(\.[a-zA-Z0-9_-]+)+").unwrap(),
        }
    }
}

#[async_trait::async_trait]
impl LlmOutMiddleware for PrivacyScrubberMiddleware {
    type Error = Infallible;

    async fn intercept(&mut self, ai_message: &mut AssistantMessage) -> Result<MiddlewareFlow, Self::Error> {
        let scrubbed = self.phone_regex.replace_all(&ai_message.content, "***-****-****");
        let scrubbed = self.email_regex.replace_all(&scrubbed, "[EMAIL PROTECTED]");

        ai_message.content = scrubbed.to_string();

        Ok(MiddlewareFlow::Continue)
    }
}