#include "helper_names.h"
#include <assimp/material.h>
#include <set>
#include <vector>
#include <stdexcept>

namespace gcad {
void restoreNames(aiScene& scene, const AuthoredNames& names) {
    auto mapped = [](const aiString& current, const auto& table) {
        auto found = table.find(std::string(current.C_Str(), current.length));
        if (found == table.end()) return current;
        const auto& name = found->second;
        if (name.empty() || name.size() >= AI_MAXLEN || name.find('\0') != std::string::npos)
            throw std::runtime_error("authored name cannot be represented by Assimp");
        return aiString(name);
    };
    std::vector<std::pair<aiString*, aiString>> edits;
    std::set<std::string> original, restored;
    std::vector<aiNode*> pending;
    if (scene.mRootNode) pending.push_back(scene.mRootNode);
    while (!pending.empty()) {
        auto* node = pending.back(); pending.pop_back();
        auto name = mapped(node->mName, names.nodes);
        if (!original.insert(node->mName.C_Str()).second || !restored.insert(name.C_Str()).second)
            throw std::runtime_error("ambiguous node identity during name restoration");
        edits.emplace_back(&node->mName, name);
        for (unsigned c = 0; c < node->mNumChildren; ++c) pending.push_back(node->mChildren[c]);
    }
    auto target = [&](aiString& name) {
        if (!original.count(name.C_Str())) throw std::runtime_error("unresolved bone or animation target");
        edits.emplace_back(&name, mapped(name, names.nodes));
    };
    for (unsigned m = 0; m < scene.mNumMeshes; ++m) {
        auto& mesh = *scene.mMeshes[m];
        edits.emplace_back(&mesh.mName, mapped(mesh.mName, names.meshes));
        for (unsigned b = 0; b < mesh.mNumBones; ++b) target(mesh.mBones[b]->mName);
    }
    for (unsigned a = 0; a < scene.mNumAnimations; ++a) {
        auto& animation = *scene.mAnimations[a];
        // The canonical gcad pipeline has node animation, not morph channels.
        if (animation.mNumMeshChannels || animation.mNumMorphMeshChannels)
            throw std::runtime_error("mesh animation name restoration unsupported");
        for (unsigned c = 0; c < animation.mNumChannels; ++c) target(animation.mChannels[c]->mNodeName);
    }
    std::vector<std::pair<aiMaterial*, aiString>> materials;
    for (unsigned m = 0; m < scene.mNumMaterials; ++m) {
        aiString name;
        auto* material = scene.mMaterials[m];
        if (material->Get(AI_MATKEY_NAME, name) == AI_SUCCESS)
            materials.emplace_back(material, mapped(name, names.materials));
    }
    // All references and collisions are checked before changing any identity.
    for (auto& edit : edits) *edit.first = edit.second;
    for (auto& edit : materials)
        if (edit.first->AddProperty(&edit.second, AI_MATKEY_NAME) != AI_SUCCESS)
            throw std::runtime_error("cannot restore material name");
}
}
