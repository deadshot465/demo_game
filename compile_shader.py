import os
import shutil

file_names = {
    'basicShader.vert': 'vert.spv',
    'basicShader_mesh.vert': 'basicShader_mesh.spv',
    'basicShader_noTexture.frag': 'basicShader_noTexture.spv',
    'basicShader.frag': 'frag.spv',
    'terrain.vert': 'terrain.spv'
}

# Compile shaders
os.chdir('./shaders')
for x, y in file_names.items():
    os.system('glslangValidator -V {} -o {}'.format(x, y))

# Copy shaders to debug folder
os.chdir('../')
for x, y in file_names.items():
    shutil.copyfile('./shaders/' + y, './target/debug/shaders/' + y)
shutil.copyfile('./.env', './target/debug/.env')
shutil.rmtree('./target/debug/models')
shutil.copytree('./models', './target/debug/models')

# Copy shaders to release folder
for x, y in file_names.items():
    shutil.copyfile('./shaders/' + y, './target/release/shaders/' + y)
shutil.copyfile('./.env', './target/release/.env')
shutil.rmtree('./target/release/models')
shutil.copytree('./models', './target/release/models')
