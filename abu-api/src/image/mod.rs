use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::ApiRequest;

#[derive(Debug, Clone, Serialize, Builder)]
#[builder(pattern = "mutable")]
#[builder(setter(into, strip_option))]
pub struct ImageRequest {
    /// A text description of the desired image(s). The maximum length is 4000 characters for dall-e-3.
    prompt: String,
    /// The model to use for image generation. Only support Dall-e-3
    model: String,
    /// The number of images to generate. Must be between 1 and 10. For dall-e-3, only n=1 is supported.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<usize>,
    /// The quality of the image that will be generated. hd creates images with finer details and greater consistency across the image. This param is only supported for dall-e-3.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<ImageQuality>,
    /// The format in which the generated images are returned. Must be one of url or b64_json.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ImageResponseFormat>,
    /// The size of the generated images. Must be one of 1024x1024, 1792x1024, or 1024x1792 for dall-e-3 models.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<ImageSize>,
    /// The style of the generated images. Must be one of vivid or natural. Vivid causes the model to lean towards generating hyper-real and dramatic images. Natural causes the model to produce more natural, less hyper-real looking images. This param is only supported for dall-e-3.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<ImageStyle>,
    /// A unique identifier representing your end-user, which can help OpenAI to monitor and detect abuse.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageQuality {
    #[default]
    Standard,
    Hd,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseFormat {
    #[default]
    Url,
    B64Json,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum ImageSize {
    #[serde(rename = "1024x1024")]
    #[default]
    Large,
    #[serde(rename = "1792x1024")]
    LargeWide,
    #[serde(rename = "1024x1792")]
    LargeTall,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStyle {
    #[default]
    Vivid,
    Natural,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageResponse {
    pub created: u64,
    pub data: Vec<ImageObject>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageObject {
    /// The base64-encoded JSON of the generated image, if response_format is b64_json
    #[serde(default)]
    pub b64_json: Option<String>,
    /// The URL of the generated image, if response_format is url (default).
    pub url: Option<String>,
    /// The prompt that was used to generate the image, if there was any revision to the prompt.
    #[serde(default)]
    pub revised_prompt: String,
}

impl ApiRequest for ImageRequest {
    type Response = ImageResponse;
    fn url(base_url: &str) -> String {
        format!("{}/images/generations", base_url)
    }
}

impl ImageRequest {
    pub fn new(model: impl Into<String>, prompt: impl Into<String>) -> Self {
        ImageRequestBuilder::default()
            .prompt(prompt)
            .model(model)
            .build()
            .unwrap()
    }
}
