#pragma once
#include "helper_names.h"
#include <rapidjson/document.h>
#include <stdexcept>

namespace gcad {
// Run after structural validation and instance isolation. Indices are temporary
// import identities, independent of optional/repeated material and mesh names.
inline AuthoredNames tagImportNames(rapidjson::Document& doc) {
    AuthoredNames result;
    auto tag = [&](const char* collection, const char* prefix, auto& names, bool mesh) {
        if (!doc.HasMember(collection) || !doc[collection].IsArray())
            throw std::runtime_error("missing import name collection");
        auto& objects = doc[collection];
        for (unsigned i = 0; i < objects.Size(); ++i) {
            auto& object = objects[i];
            if (!object.IsObject()) throw std::runtime_error("invalid named import object");
            std::string authored = std::string(collection) + "_" + std::to_string(i);
            if (object.HasMember("name")) {
                if (!object["name"].IsString()) throw std::runtime_error("invalid authored name");
                authored.assign(object["name"].GetString(), object["name"].GetStringLength());
            }
            if (authored.empty() || authored.size() >= AI_MAXLEN || authored.find('\0') != std::string::npos)
                throw std::runtime_error("authored name cannot be represented by Assimp");
            const std::string identity = std::string(prefix) + std::to_string(i);
            if (mesh) {
                if (!object.HasMember("primitives") || !object["primitives"].IsArray() || object["primitives"].Empty())
                    throw std::runtime_error("missing named mesh primitives");
                const unsigned count = object["primitives"].Size();
                for (unsigned p = 0; p < count; ++p)
                    names.emplace(identity + (count > 1 ? "-" + std::to_string(p) : ""), authored);
            } else names.emplace(identity, authored);
            object.RemoveMember("name");
            object.AddMember("name", rapidjson::Value(identity.c_str(), doc.GetAllocator()), doc.GetAllocator());
        }
    };
    tag("materials", "gcad_audit_material_", result.materials, false);
    tag("meshes", "gcad_source_mesh_", result.meshes, true);
    tag("nodes", "gcad_audit_node_", result.nodes, false);
    return result;
}
}
