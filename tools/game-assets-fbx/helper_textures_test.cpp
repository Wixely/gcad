#include "helper_textures.h"
#include "helper_names.h"
#include "helper_import_names.h"
#include <assimp/material.h>
#include <assimp/Exporter.hpp>
#include <ufbx.h>
#include <cstring>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>

static void check(bool ok) { if (!ok) throw std::runtime_error("texture preparation assertion failed"); }
int main(int argc, char** argv) {
    try {
        check(argc == 2);
        std::ifstream input(argv[1], std::ios::binary);
        check(bool(input));
        std::vector<unsigned char> png((std::istreambuf_iterator<char>(input)), {});
        check(!png.empty());
        aiScene scene;
        scene.mRootNode = new aiNode("Authored root");
        scene.mRootNode->mNumChildren = 1;
        scene.mRootNode->mChildren = new aiNode*[1]{new aiNode("Authored mesh node")};
        auto& node = *scene.mRootNode->mChildren[0];
        node.mParent = scene.mRootNode;
        node.mNumMeshes = 1; node.mMeshes = new unsigned[1]{0};
        scene.mNumMeshes = 1; scene.mMeshes = new aiMesh*[1]{new aiMesh};
        auto& mesh = *scene.mMeshes[0];
        mesh.mName = aiString("Authored geometry");
        mesh.mPrimitiveTypes = aiPrimitiveType_TRIANGLE;
        mesh.mNumVertices = 3;
        mesh.mVertices = new aiVector3D[3]{{0,0,0}, {1,0,0}, {0,1,0}};
        mesh.mNormals = new aiVector3D[3]{{0,0,1}, {0,0,1}, {0,0,1}};
        mesh.mTextureCoords[0] = new aiVector3D[3]{{0,0,0}, {1,0,0}, {0,1,0}};
        mesh.mNumUVComponents[0] = 2;
        mesh.mNumFaces = 1; mesh.mFaces = new aiFace[1];
        mesh.mFaces[0].mNumIndices = 3; mesh.mFaces[0].mIndices = new unsigned[3]{0,1,2};
        scene.mNumTextures = 1; scene.mTextures = new aiTexture*[1]{new aiTexture};
        auto& image = *scene.mTextures[0];
        image.mWidth = unsigned(png.size());
        image.pcData = new aiTexel[(png.size() + sizeof(aiTexel)-1)/sizeof(aiTexel)]{};
        std::memcpy(image.pcData, png.data(), png.size());
        image.mFilename = aiString("../../authored-image.png");
        scene.mNumMaterials = 1; scene.mMaterials = new aiMaterial*[1]{new aiMaterial};
        auto& material = *scene.mMaterials[0];
        aiString name("Authored material"), ref("*0");
        material.AddProperty(&name, AI_MATKEY_NAME);
        material.AddProperty(&ref, AI_MATKEY_TEXTURE_DIFFUSE(0));
        material.AddProperty(&ref, AI_MATKEY_TEXTURE_NORMALS(0));
        auto plan = gcad::planTextures(scene);
        check(plan.files.size() == 1 && plan.bindings.size() == 2);
        check(plan.files[0].png == png && plan.files[0].relative_path == "textures/image_0.png");
        check(plan.bindings[0].file == plan.bindings[1].file);
        aiString unchanged;
        check(material.Get(AI_MATKEY_NAME, unchanged) == AI_SUCCESS && unchanged == name);
        check(material.Get(AI_MATKEY_TEXTURE_DIFFUSE(0), unchanged) == AI_SUCCESS && unchanged == ref);
        for (const char* bad : {"../escape.png", "*0junk", "*01", "*99999999999999999999", "*1"}) {
            aiString value(bad); material.AddProperty(&value, AI_MATKEY_TEXTURE_DIFFUSE(0));
            bool rejected = false;
            try { gcad::planTextures(scene); } catch (const std::runtime_error&) { rejected = true; }
            check(rejected);
        }
        material.AddProperty(&ref, AI_MATKEY_TEXTURE_DIFFUSE(0));
        auto full = image.mWidth; image.mWidth = 24;
        bool rejected = false;
        try { gcad::planTextures(scene); } catch (const std::runtime_error&) { rejected = true; }
        check(rejected); image.mWidth = full;
        check(gcad::planTextures(scene).files[0].png == png);
        auto prepared = gcad::prepareTextures(scene);
        check(prepared.scene->mNumTextures == 0 && scene.mNumTextures == 1);
        check(prepared.textures.files[0].png == png);
        check(material.Get(AI_MATKEY_TEXTURE_DIFFUSE(0), unchanged) == AI_SUCCESS && unchanged == ref);
        check(prepared.scene->mRootNode->mChildren[0]->mName == node.mName);
        check(prepared.scene->mMeshes[0]->mName == mesh.mName);
        // Exercise the temporary import identities, including linked references.
        auto& privateNode = *prepared.scene->mRootNode->mChildren[0];
        privateNode.mName = aiString("gcad_audit_node_0");
        auto& privateMesh = *prepared.scene->mMeshes[0];
        privateMesh.mName = aiString("gcad_audit_mesh_0");
        privateMesh.mNumBones = 1; privateMesh.mBones = new aiBone*[1]{new aiBone};
        privateMesh.mBones[0]->mName = privateNode.mName;
        prepared.scene->mNumAnimations = 1;
        prepared.scene->mAnimations = new aiAnimation*[1]{new aiAnimation};
        auto& animation = *prepared.scene->mAnimations[0];
        animation.mNumChannels = 1; animation.mChannels = new aiNodeAnim*[1]{new aiNodeAnim};
        animation.mChannels[0]->mNodeName = privateNode.mName;
        aiString temporary("gcad_audit_material_0");
        prepared.scene->mMaterials[0]->AddProperty(&temporary, AI_MATKEY_NAME);
        rapidjson::Document document;
        document.Parse(R"({"nodes":[{"name":"Authored mesh node"}],"meshes":[{"name":"Authored geometry","primitives":[{}]}],"materials":[{"name":"Authored material"}]})");
        auto names = gcad::tagImportNames(document);
        check(names.nodes.at("gcad_audit_node_0") == "Authored mesh node");
        check(names.materials.at("gcad_audit_material_0") == "Authored material");
        privateMesh.mName = aiString("gcad_source_mesh_0");
        names.nodes["gcad_audit_node_0"] = "Authored root";
        bool collision = false;
        try { gcad::restoreNames(*prepared.scene, names); } catch (const std::runtime_error&) { collision = true; }
        check(collision && privateNode.mName == aiString("gcad_audit_node_0"));
        names.nodes["gcad_audit_node_0"] = std::string(AI_MAXLEN, 'x');
        bool tooLong = false;
        try { gcad::restoreNames(*prepared.scene, names); } catch (const std::runtime_error&) { tooLong = true; }
        check(tooLong && privateNode.mName == aiString("gcad_audit_node_0"));
        names.nodes["gcad_audit_node_0"] = "Authored mesh node";
        animation.mChannels[0]->mNodeName = aiString("missing target");
        bool unresolved = false;
        try { gcad::restoreNames(*prepared.scene, names); } catch (const std::runtime_error&) { unresolved = true; }
        check(unresolved && privateNode.mName == aiString("gcad_audit_node_0"));
        animation.mChannels[0]->mNodeName = privateNode.mName;
        gcad::restoreNames(*prepared.scene, names);
        check(privateNode.mName == node.mName && privateMesh.mName == mesh.mName);
        check(privateMesh.mBones[0]->mName == node.mName);
        check(animation.mChannels[0]->mNodeName == node.mName);
        // The synthetic references above test restoration only; the texture
        // readback fixture remains static. Rig conversion has separate audits.
        delete privateMesh.mBones[0]; delete[] privateMesh.mBones;
        privateMesh.mBones = nullptr; privateMesh.mNumBones = 0;
        delete prepared.scene->mAnimations[0]; delete[] prepared.scene->mAnimations;
        prepared.scene->mAnimations = nullptr; prepared.scene->mNumAnimations = 0;
        Assimp::Exporter exporter;
        const auto* blob = exporter.ExportToBlob(prepared.scene.get(), "fbx");
        if (!blob) throw std::runtime_error(exporter.GetErrorString());
        ufbx_load_opts options{}; ufbx_error error{};
        std::unique_ptr<ufbx_scene, decltype(&ufbx_free_scene)> readback(
            ufbx_load_memory(blob->data, blob->size, &options, &error), ufbx_free_scene);
        check(bool(readback));
        auto text = [](ufbx_string value) { return std::string(value.data, value.length); };
        check(ufbx_find_node(readback.get(), "Authored mesh node") != nullptr);
        check(readback->materials.count == 1 && text(readback->materials[0]->name) == "Authored material");
        const auto& maps = readback->materials[0]->fbx;
        check(maps.diffuse_color.texture && maps.normal_map.texture);
        check(text(maps.diffuse_color.texture->relative_filename) == "textures/image_0.png");
        check(text(maps.normal_map.texture->relative_filename) == "textures/image_0.png");
        check(readback->textures.count > 0);
        for (auto* texture : readback->textures) {
            check(text(texture->relative_filename) == "textures/image_0.png");
            check(texture->content.size == 0);
        }
        for (auto* video : readback->videos) check(video->content.size == 0);
        check(readback->meshes.count == 1 && readback->meshes[0]->num_triangles == 1);
        check(readback->meshes[0]->materials.count == 1 &&
            readback->meshes[0]->materials[0] == readback->materials[0]);
        std::cout << "Independent FBX readback: authored node/material names, relative PNG references and no embedded images passed\n";
        std::cout << "Texture preparation: shared bindings, exact PNG bytes, safe paths, unchanged names, malformed references and truncated PNG passed\n";
        return 0;
    } catch (const std::exception& e) { std::cerr << e.what() << '\n'; return 1; }
}
