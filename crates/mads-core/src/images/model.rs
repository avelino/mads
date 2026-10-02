use async_trait::async_trait;

use crate::google::AspectRatio;

/// What the image model is asked to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRequest {
    pub prompt: String,
    pub ratio: AspectRatio,
    /// PNG or JPEG of a real product the picture must show.
    pub reference: Option<Vec<u8>>,
}

/// One image generation backend. Implemented in `mads-providers`, faked in tests.
#[async_trait]
pub trait ImageModel: Send + Sync {
    /// `provider:model`, shown in events and the report.
    fn id(&self) -> String;
    /// PNG or JPEG bytes of any size; the image step crops and resizes them.
    async fn generate(&self, req: &ImageRequest) -> Result<Vec<u8>, String>;
}

/// Draws a flat picture without any network. The hidden `solid` provider and tests use it.
pub struct SolidImageModel;

#[async_trait]
impl ImageModel for SolidImageModel {
    fn id(&self) -> String {
        "solid".into()
    }

    async fn generate(&self, req: &ImageRequest) -> Result<Vec<u8>, String> {
        let shade = (req.prompt.len() % 200) as u8;
        Ok(super::solid_png(req.ratio, [shade, 90, 160]))
    }
}
