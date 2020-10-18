#[derive(Eq, PartialEq, Hash, Copy, Clone, Debug)]
pub enum ShaderType {
    BasicShader,
    BasicShaderWithoutTexture,
    AnimatedModel,
    Terrain,
    Water,
}

impl ToString for ShaderType {
    fn to_string(&self) -> String {
        match self {
            ShaderType::BasicShader => "BasicShader".to_string(),
            ShaderType::BasicShaderWithoutTexture => "BasicShaderWithoutTexture".to_string(),
            ShaderType::AnimatedModel => "AnimatedModel".to_string(),
            ShaderType::Terrain => "Terrain".to_string(),
            ShaderType::Water => "Water".to_string(),
        }
    }
}

impl ShaderType {
    pub fn get_all_shader_types() -> Vec<ShaderType> {
        vec![
            ShaderType::BasicShader,
            ShaderType::BasicShaderWithoutTexture,
            ShaderType::AnimatedModel,
            ShaderType::Terrain,
            ShaderType::Water,
        ]
    }

    pub fn get_all_shader_type_pairs() -> Vec<(ShaderType, String)> {
        let shader_types = ShaderType::get_all_shader_types();
        let shader_type_names = shader_types
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        shader_types
            .iter()
            .zip(shader_type_names.iter())
            .map(|p| (*p.0, p.1.clone()))
            .collect::<Vec<_>>()
    }
}
