#pragma once
#include <assimp/scene.h>
#include <string>
#include <vector>
#include <memory>

namespace gcad {
struct TextureFile {
    unsigned source;
    std::string relative_path;
    std::vector<unsigned char> png;
};
struct TextureBinding {
    unsigned material, semantic, slot, file;
};
struct TexturePlan {
    std::vector<TextureFile> files;
    std::vector<TextureBinding> bindings;
};
// Pure preparation for a private export scene. Does not write files or alter names,
// material properties or embedded images. The publisher owns staging and cleanup.
TexturePlan planTextures(const aiScene& scene);
struct SceneDeleter { void operator()(aiScene* scene) const; };
struct PreparedScene {
    std::unique_ptr<aiScene, SceneDeleter> scene;
    TexturePlan textures;
};
// Owns a deep copy. References point to the accompanying PNG manifest and no
// embedded image remains for the FBX writer to accidentally embed or alias.
PreparedScene prepareTextures(const aiScene& source);
}
