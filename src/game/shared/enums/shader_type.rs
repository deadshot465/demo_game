#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub enum ShaderType {
    BasicShader,
    BasicShaderForMesh,
    BasicShaderWithoutTexture,
    AnimatedModel,
    Terrain,
}

impl ToString for ShaderType {
    fn to_string(&self) -> String {
        match self {
            ShaderType::BasicShader => "BasicShader".to_string(),
            ShaderType::BasicShaderForMesh => "BasicShaderForMesh".to_string(),
            ShaderType::BasicShaderWithoutTexture => "BasicShaderWithoutTexture".to_string(),
            ShaderType::AnimatedModel => "AnimatedModel".to_string(),
            ShaderType::Terrain => "Terrain".to_string(),
        }
    }
}
