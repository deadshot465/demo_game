#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum ImageFormat {
    GltfFormat(gltf::image::Format),
    VkFormat(ash::vk::Format),
    ColorType(image::ColorType),
}
