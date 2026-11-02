#pragma once
#include <assimp/scene.h>
#include <map>
#include <string>

namespace gcad {
struct AuthoredNames {
    std::map<std::string, std::string> nodes, meshes, materials;
};
// Apply after import validation and before export, on an owned scene copy.
// Node identities must remain unique because Assimp resolves animation and bone
// targets by name. The gcad source scene already requires unique node names.
void restoreNames(aiScene& scene, const AuthoredNames& names);
}
