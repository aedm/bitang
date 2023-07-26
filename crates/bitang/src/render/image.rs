use std::cell::Cell;
use vulkano::image::{AttachmentImage, ImageAccess, ImmutableImage, StorageImage};

enum ImageInner {
    Immutable(ImmutableImage),
    SingleLevelAttachment(Option<AttachmentImage>),
}

pub enum ImageFormat {
    Rgba16F,
    Depth32F,
    Rgba8U,
}

pub enum ImageSizeRule {
    Fixed(u32, u32),
    CanvasRelative(f32),
    At4k(u32, u32),
}

pub struct Image {
    pub id: String,
    pub size_rule: ImageSizeRule,

    inner: ImageInner,
    size: Cell<Option<(u32, u32)>>,
}

impl Image {
    pub fn new_immutable(id: &str, source: ImmutableImage) -> Self {
        let dim = source.dimensions().width_height();
        Self {
            id: id.to_owned(),
            inner: ImageInner::Immutable(source),
            size_rule: ImageSizeRule::Fixed(dim[0], dim[1]),
            size: Cell::new(Some((dim[0], dim[1]))),
        }
    }

    pub fn new_attachment(id: &str, format: ImageFormat, size_rule: ImageSizeRule) -> Self {
        Self {
            id: id.to_owned(),
            inner: ImageInner::SingleLevelAttachment(None),
            size_rule: size,
            size: Cell::new(None),
        }
    }

    // pub fn get_view(&self) -> ImageView {
    //     match &self.inner {
    //         ImageInner::Immutable(image) => ImageView::new(image.clone()).unwrap(),
    //         ImageInner::SingleLevelAttachment(image) => {
    //             ImageView::new(image.clone()).unwrap()
    //         }
    //     }
    // }
}

struct ManagedTexture {
    image: StorageImage,
}
