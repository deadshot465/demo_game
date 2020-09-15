import io
import os
import shutil

os.chdir('./shaders')
os.system('glslangValidator -V basicShader.vert')
os.system('glslangValidator -V basicShader_mesh.vert -o basicShader_mesh.spv')
os.system('glslangValidator -V basicShader_noTexture.frag -o basicShader_noTexture.spv')
os.system('glslangValidator -V basicShader.frag')
os.chdir('../')
shutil.copyfile('./shaders/vert.spv', './target/debug/shaders/vert.spv')
shutil.copyfile('./shaders/basicShader_mesh.spv', './target/debug/shaders/basicShader_mesh.spv')
shutil.copyfile('./shaders/basicShader_noTexture.spv', './target/debug/shaders/basicShader_noTexture.spv')
shutil.copyfile('./shaders/frag.spv', './target/debug/shaders/frag.spv')

if os.path.isdir('cmake-build-debug'):
    shutil.copyfile('./shaders/vert.spv', 'cmake-build-debug/GLVK/VK/Shaders/vert.spv')
    shutil.copyfile('./shaders/basicShader_mesh.spv', 'cmake-build-debug/GLVK/VK/Shaders/basicShader_mesh.spv')
    shutil.copyfile('./shaders/basicShader_noTexture.spv', 'cmake-build-debug/GLVK/VK/Shaders/basicShader_noTexture.spv')
    shutil.copyfile('./shaders/frag.spv', 'cmake-build-debug/GLVK/VK/Shaders/frag.spv')