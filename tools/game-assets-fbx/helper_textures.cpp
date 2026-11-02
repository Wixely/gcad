#include "helper_textures.h"
#include <assimp/material.h>
#include <assimp/cexport.h>
#include <algorithm>
#include <cstring>
#include <map>
#include <memory>
#include <stdexcept>
#define STB_IMAGE_IMPLEMENTATION
#define STB_IMAGE_STATIC
#define STBI_ONLY_PNG
#define STBI_NO_STDIO
#define STBI_NO_HDR
#define STBI_NO_LINEAR
#include <stb_image.h>

namespace gcad {
void SceneDeleter::operator()(aiScene* scene) const { aiFreeScene(scene); }

PreparedScene prepareTextures(const aiScene& source) {
    auto plan = planTextures(source);
    aiScene* copy = nullptr;
    aiCopyScene(&source, &copy);
    PreparedScene result{std::unique_ptr<aiScene, SceneDeleter>(copy), std::move(plan)};
    if (!copy) throw std::runtime_error("cannot copy export scene");
    for (const auto& binding : result.textures.bindings) {
        aiString path(result.textures.files[binding.file].relative_path);
        if (copy->mMaterials[binding.material]->AddProperty(&path, _AI_MATKEY_TEXTURE_BASE,
            binding.semantic, binding.slot) != AI_SUCCESS)
            throw std::runtime_error("cannot assign external texture reference");
    }
    for (unsigned i = 0; i < copy->mNumTextures; ++i) delete copy->mTextures[i];
    delete[] copy->mTextures;
    copy->mTextures = nullptr;
    copy->mNumTextures = 0;
    return result;
}

TexturePlan planTextures(const aiScene& scene) {
    TexturePlan plan;
    std::map<unsigned, unsigned> files;
    size_t encoded = 0, decoded = 0;
    if (scene.mNumTextures > 1024) throw std::runtime_error("too many source textures");
    for (unsigned m = 0; m < scene.mNumMaterials; ++m) {
        const auto& material = *scene.mMaterials[m];
        for (unsigned p = 0; p < material.mNumProperties; ++p) {
            const auto& property = *material.mProperties[p];
            if (property.mKey != aiString(_AI_MATKEY_TEXTURE_BASE)) continue;
            aiString ref;
            if (material.Get(_AI_MATKEY_TEXTURE_BASE, property.mSemantic, property.mIndex, ref) != AI_SUCCESS)
                throw std::runtime_error("invalid texture reference");
            // Canonical GLB references only: no filesystem reads or authored names
            // are allowed to determine the output path.
            std::string name(ref.C_Str(), ref.length);
            if (name.size() < 2 || name[0] != '*') throw std::runtime_error("external texture reference unsupported");
            unsigned index = 0;
            for (size_t i = 1; i < name.size(); ++i) {
                if (name[i] < '0' || name[i] > '9' || index > 1024)
                    throw std::runtime_error("invalid embedded texture reference");
                index = index * 10 + unsigned(name[i] - '0');
            }
            if (name != "*" + std::to_string(index) || index >= scene.mNumTextures)
                throw std::runtime_error("invalid embedded texture index");
            auto found = files.find(index);
            if (found == files.end()) {
                const auto& texture = *scene.mTextures[index];
                if (texture.mHeight || !texture.pcData || texture.mWidth < 8 || texture.mWidth > 16 * 1024 * 1024)
                    throw std::runtime_error("expected bounded encoded PNG texture");
                const auto* bytes = reinterpret_cast<const unsigned char*>(texture.pcData);
                static const unsigned char signature[] = {137,80,78,71,13,10,26,10};
                if (std::memcmp(bytes, signature, sizeof(signature))) throw std::runtime_error("texture is not PNG");
                if (texture.mWidth > 64 * 1024 * 1024 - encoded) throw std::runtime_error("encoded texture budget exceeded");
                int w = 0, h = 0, channels = 0;
                if (!stbi_info_from_memory(bytes, int(texture.mWidth), &w, &h, &channels) ||
                    w <= 0 || h <= 0 || w > 8192 || h > 8192)
                    throw std::runtime_error("invalid PNG dimensions");
                const size_t charge = size_t(w) * size_t(h) * 8;
                if (charge > 128 * 1024 * 1024 - decoded) throw std::runtime_error("decoded texture budget exceeded");
                // Decode at the source precision for validation only. Copy the
                // original encoded bytes so color/alpha/16-bit data is unchanged.
                const bool wide = stbi_is_16_bit_from_memory(bytes, int(texture.mWidth)) != 0;
                std::unique_ptr<void, decltype(&stbi_image_free)> pixels(
                    wide ? static_cast<void*>(stbi_load_16_from_memory(bytes, int(texture.mWidth), &w, &h, &channels, 4))
                         : static_cast<void*>(stbi_load_from_memory(bytes, int(texture.mWidth), &w, &h, &channels, 4)),
                    stbi_image_free);
                if (!pixels) throw std::runtime_error("PNG pixel decode failed");
                encoded += texture.mWidth;
                decoded += charge;
                const unsigned id = unsigned(plan.files.size());
                plan.files.push_back({index, "textures/image_" + std::to_string(index) + ".png",
                    std::vector<unsigned char>(bytes, bytes + texture.mWidth)});
                found = files.emplace(index, id).first;
            }
            plan.bindings.push_back({m, property.mSemantic, property.mIndex, found->second});
        }
    }
    return plan;
}
}
