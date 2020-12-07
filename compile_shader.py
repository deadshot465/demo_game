import os
import shutil

file_names = {
    'basicShader.vert': 'vert.spv',
    'basicShader_animated.vert': 'basicShader_animated.spv',
    'basicShader_noTexture.frag': 'basicShader_noTexture.spv',
    'basicShader.frag': 'frag.spv',
    'terrain.vert': 'terrain_vert.spv',
    'terrain.frag': 'terrain_frag.spv',
    'water.frag': 'water_frag.spv',
    'instance.vert': 'instance_vert.spv',
    'instance.frag': 'instance_frag.spv',
    'ui.vert': 'ui_vert.spv',
    'ui.frag': 'ui_frag.spv'
}

# Compile shaders
os.chdir('./shaders')
for x, y in file_names.items():
    os.system('glslangValidator -V {} -o {}'.format(x, y))

# Create the folder in case the folder doesn't exist
if not os.path.exists('./target'):
    os.makedirs('./target')
if not os.path.exists('./target/debug'):
    os.makedirs('./target/debug')
if not os.path.exists('./target/debug/shaders'):
    os.makedirs('./target/debug/shaders')
if not os.path.exists('./target/debug/models'):
    os.makedirs('./target/debug/models')
if not os.path.exists('./target/debug/resource'):
    os.makedirs('./target/debug/resource')
if not os.path.exists('./target/debug/textures'):
    os.makedirs('./target/debug/textures')

if not os.path.exists('./target/release'):
    os.makedirs('./target/release')
if not os.path.exists('./target/release/shaders'):
    os.makedirs('./target/release/shaders')
if not os.path.exists('./target/release/models'):
    os.makedirs('./target/release/models')
if not os.path.exists('./target/release/resource'):
    os.makedirs('./target/release/resource')
if not os.path.exists('./target/release/textures'):
    os.makedirs('./target/release/textures')

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

# Copy resource to debug and release folder
shutil.rmtree('./target/debug/resource')
shutil.copytree('./resource', './target/debug/resource')
shutil.rmtree('./target/release/resource')
shutil.copytree('./resource', './target/release/resource')

shutil.rmtree('./target/debug/textures')
shutil.copytree('./textures', './target/debug/textures')
shutil.rmtree('./target/release/textures')
shutil.copytree('./textures', './target/release/textures')