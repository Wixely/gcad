// Isolated feasibility fixture, not a gcad export backend.
#include <assimp/Exporter.hpp>
#include <assimp/Importer.hpp>
#include <assimp/metadata.h>
#include <assimp/material.h>
#include <assimp/scene.h>
#include <ufbx.h>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <set>
#include <stdexcept>
#include <string>
#include <cstring>

static void require(bool condition, const char *message) {
    if (!condition) throw std::runtime_error(message);
}

static const aiVector3D positions[] = {
    {-10, 10, 0}, {10, 10, 0}, {-10, 60, 0},
    {10, 60, 0}, {-10, 110, 0}, {10, 110, 0},
};

static aiNode *child(aiNode *parent, const char *name) {
    auto *node = new aiNode(name);
    node->mParent = parent;
    return node;
}

static std::unique_ptr<aiScene> fixture(bool skinned) {
    auto scene = std::make_unique<aiScene>();
    scene->mRootNode = new aiNode("Scene");
    auto *root = scene->mRootNode;
    root->mNumChildren = skinned ? 2 : 1;
    root->mChildren = new aiNode *[root->mNumChildren];
    auto *node = child(root, "Ribbon");
    root->mChildren[0] = node;
    node->mNumMeshes = 1;
    node->mMeshes = new unsigned[1]{0};
    scene->mNumMeshes = 1;
    scene->mMeshes = new aiMesh *[1]{new aiMesh};
    auto *mesh = scene->mMeshes[0];
    mesh->mName = aiString("RibbonMesh");
    mesh->mPrimitiveTypes = aiPrimitiveType_TRIANGLE;
    mesh->mNumVertices = 6;
    mesh->mVertices = new aiVector3D[6];
    mesh->mNormals = new aiVector3D[6];
    mesh->mTextureCoords[0] = new aiVector3D[6];
    mesh->mNumUVComponents[0] = 2;
    for (unsigned i = 0; i < 6; ++i) {
        mesh->mVertices[i] = positions[i];
        mesh->mNormals[i] = {0, 0, 1};
        mesh->mTextureCoords[0][i] = {float(i % 2), float(i / 2) / 2, 0};
    }
    mesh->mNumFaces = 4;
    mesh->mFaces = new aiFace[4];
    const unsigned indices[4][3] = {{0, 1, 2}, {1, 3, 2}, {2, 3, 4}, {3, 5, 4}};
    for (unsigned i = 0; i < 4; ++i) {
        mesh->mFaces[i].mNumIndices = 3;
        mesh->mFaces[i].mIndices = new unsigned[3]{indices[i][0], indices[i][1], indices[i][2]};
    }
    scene->mNumMaterials = 1;
    scene->mMaterials = new aiMaterial *[1]{new aiMaterial};
    aiString materialName("Checker"), texturePath("checker.bmp");
    scene->mMaterials[0]->AddProperty(&materialName, AI_MATKEY_NAME);
    scene->mMaterials[0]->AddProperty(&texturePath, AI_MATKEY_TEXTURE_DIFFUSE(0));
    aiColor3D white(1, 1, 1);
    scene->mMaterials[0]->AddProperty(&white, 1, AI_MATKEY_COLOR_DIFFUSE);
    if (!skinned) return scene;

    auto *base = child(root, "Base");
    root->mChildren[1] = base;
    base->mTransformation.b4 = 10;
    auto *tip = child(base, "Tip");
    tip->mTransformation.b4 = 50;
    base->mNumChildren = 1;
    base->mChildren = new aiNode *[1]{tip};
    mesh->mNumBones = 2;
    mesh->mBones = new aiBone *[2]{new aiBone, new aiBone};
    for (unsigned b = 0; b < 2; ++b) {
        auto *bone = mesh->mBones[b];
        bone->mName = aiString(b ? "Tip" : "Base");
        bone->mOffsetMatrix.b4 = b ? -60 : -10;
        bone->mNumWeights = 4;
        bone->mWeights = new aiVertexWeight[4];
        for (unsigned k = 0; k < 4; ++k) {
            unsigned vertex = k + b * 2;
            bone->mWeights[k] = aiVertexWeight(vertex, (vertex == 2 || vertex == 3) ? 0.5f : 1.0f);
        }
    }
    scene->mNumAnimations = 2;
    scene->mAnimations = new aiAnimation *[2];
    for (unsigned c = 0; c < 2; ++c) {
        auto *clip = new aiAnimation;
        scene->mAnimations[c] = clip;
        clip->mName = aiString(c ? "Twist" : "Bend");
        clip->mDuration = 1;
        clip->mTicksPerSecond = 1;
        clip->mNumChannels = 1;
        clip->mChannels = new aiNodeAnim *[1]{new aiNodeAnim};
        auto *track = clip->mChannels[0];
        track->mNodeName = aiString("Tip");
        track->mNumPositionKeys = 2;
        track->mPositionKeys = new aiVectorKey[2]{{0, {0, 50, 0}}, {1, {0, 50, 0}}};
        track->mNumScalingKeys = 2;
        track->mScalingKeys = new aiVectorKey[2]{{0, {1, 1, 1}}, {1, {1, 1, 1}}};
        track->mNumRotationKeys = 3;
        track->mRotationKeys = new aiQuatKey[3];
        for (unsigned k = 0; k < 3; ++k) {
            double angle = k * 3.141592653589793 / 4;
            aiQuaternion q(float(std::cos(angle / 2)), 0,
                c ? float(std::sin(angle / 2)) : 0,
                c ? 0 : float(std::sin(angle / 2)));
            track->mRotationKeys[k] = aiQuatKey(k * 0.5, q);
        }
    }
    return scene;
}

static void texture(const std::filesystem::path &path) {
    // Original 2x2 RGB checker BMP, two padded rows, no external image tooling.
    unsigned char bytes[70] = {'B', 'M', 70};
    bytes[10] = 54; bytes[14] = 40; bytes[18] = 2; bytes[22] = 2;
    bytes[26] = 1; bytes[28] = 24; bytes[34] = 16;
    for (unsigned offset : {54u, 65u}) {
        bytes[offset] = 255; bytes[offset + 1] = 255; bytes[offset + 2] = 255;
    }
    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char *>(bytes), sizeof(bytes));
    require(bool(out), "texture write failed");
}

using Scene = std::unique_ptr<ufbx_scene, decltype(&ufbx_free_scene)>;

static void verify(const std::filesystem::path &path, bool skinned) {
    ufbx_load_opts options{};
    options.evaluate_skinning = true;
    ufbx_error error{};
    Scene scene(ufbx_load_file(path.string().c_str(), &options, &error), ufbx_free_scene);
    if (!scene) throw std::runtime_error(error.description.data);
    require(scene->meshes.count == 1, "mesh count changed");
    auto *mesh = scene->meshes.data[0];
    require(mesh->num_vertices == 6 && mesh->num_faces == 4, "geometry counts changed");
    require(mesh->vertex_uv.exists && mesh->vertex_normal.exists, "UVs or normals missing");
    require(scene->materials.count == 1 && scene->textures.count >= 1, "material/texture missing");
    require(scene->materials.data[0]->fbx.diffuse_color.texture != nullptr, "diffuse texture is not connected");
    require(std::abs(scene->settings.unit_meters - 0.01) < 1e-9, "unexpected units");
    for (size_t corner = 0; corner < mesh->num_indices; ++corner) {
        auto index = mesh->vertex_indices.data[corner];
        require(index < 6, "vertex mapping changed");
        auto p = ufbx_get_vertex_vec3(&mesh->vertex_position, corner);
        auto uv = ufbx_get_vertex_vec2(&mesh->vertex_uv, corner);
        auto n = ufbx_get_vertex_vec3(&mesh->vertex_normal, corner);
        require(std::abs(p.x - positions[index].x) < 1e-6 &&
            std::abs(p.y - positions[index].y) < 1e-6 && std::abs(p.z) < 1e-6, "rest geometry changed");
        require(std::abs(uv.x - index % 2) < 1e-6 &&
            std::abs(uv.y - (index / 2) / 2.0) < 1e-6, "UV values changed");
        require(std::abs(n.x) < 1e-6 && std::abs(n.y) < 1e-6 && std::abs(n.z - 1) < 1e-6, "normal values changed");
    }
    bool relativeTexture = false;
    for (size_t i = 0; i < scene->textures.count; ++i) {
        auto *t = scene->textures.data[i];
        if (std::string(t->relative_filename.data, t->relative_filename.length) == "checker.bmp")
            relativeTexture = true;
    }
    require(relativeTexture, "portable texture reference missing");
    require(std::filesystem::exists(path.parent_path() / "checker.bmp"), "texture is not packaged");
    require(scene->anim_stacks.count == (skinned ? 2 : 0), "clip count changed");
    if (!skinned) {
        require(mesh->skin_deformers.count == 0, "static mesh acquired skinning");
        std::cout << "static: geometry, UVs, normals, material, texture path and cm units passed\n";
        return;
    }
    require(mesh->skin_deformers.count == 1, "skin missing");
    require(mesh->skin_deformers.data[0]->clusters.count == 2, "bone count changed");
    auto *base = ufbx_find_node(scene.get(), "Base");
    auto *tip = ufbx_find_node(scene.get(), "Tip");
    require(base && tip && tip->parent == base, "bone hierarchy changed");
    auto *skin = mesh->skin_deformers.data[0];
    require(skin->vertices.count == 6, "skin vertex count changed");
    for (size_t i = 0; i < 6; ++i) {
        auto v = skin->vertices.data[i];
        double total = 0, tipWeight = 0;
        for (size_t k = 0; k < v.num_weights; ++k) {
            auto w = skin->weights.data[v.weight_begin + k];
            total += w.weight;
            if (skin->clusters.data[w.cluster_index]->bone_node == tip) tipWeight += w.weight;
        }
        double expected = i < 2 ? 0 : i < 4 ? 0.5 : 1;
        require(std::abs(total - 1) < 1e-6 && std::abs(tipWeight - expected) < 1e-6, "skin weights changed");
    }
    double maximumError = 0;
    std::set<std::string> names;
    for (size_t clip = 0; clip < scene->anim_stacks.count; ++clip) {
        auto *stack = scene->anim_stacks.data[clip];
        std::string name(stack->name.data, stack->name.length);
        require(name == "Bend" || name == "Twist", "clip name changed");
        require(names.insert(name).second, "duplicate clip name");
        require(std::abs(stack->time_begin) < 1e-6 && std::abs(stack->time_end - 1) < 1e-6, "clip duration changed");
        for (unsigned sample = 0; sample <= 8; ++sample) {
            double time = sample / 8.0;
            double angle = time * 3.141592653589793 / 2;
            ufbx_evaluate_opts evalOptions{};
            evalOptions.evaluate_skinning = true;
            Scene evaluated(ufbx_evaluate_scene(scene.get(), stack->anim, time, &evalOptions, &error), ufbx_free_scene);
            require(bool(evaluated), "animation evaluation failed");
            auto *deformed = evaluated->meshes.data[0];
            require(!deformed->skinned_is_local, "expected world-space skin evaluation");
            for (size_t corner = 0; corner < deformed->num_indices; ++corner) {
                auto index = deformed->vertex_indices.data[corner];
                require(index < 6, "vertex mapping changed");
                auto original = positions[index];
                double weight = index < 2 ? 0 : index < 4 ? 0.5 : 1;
                double x = original.x, y = original.y - 60;
                double rx = name == "Bend" ? x * std::cos(angle) - y * std::sin(angle) : x * std::cos(angle);
                double ry = name == "Bend" ? x * std::sin(angle) + y * std::cos(angle) : y;
                double rz = name == "Bend" ? 0 : -x * std::sin(angle);
                double expected[3] = {x * (1 - weight) + rx * weight,
                    original.y * (1 - weight) + (ry + 60) * weight, rz * weight};
                auto actual = ufbx_get_vertex_vec3(&deformed->skinned_position, corner);
                double squared = 0;
                for (unsigned axis = 0; axis < 3; ++axis) {
                    double delta = actual.v[axis] - expected[axis];
                    squared += delta * delta;
                }
                maximumError = std::max(maximumError, std::sqrt(squared));
            }
        }
    }
    std::cout << "skinned: two clips, nine samples each; max position error = " << maximumError << " cm\n";
    require(maximumError <= 0.001, "sampled deformation exceeds 0.001 cm tolerance");
}

// Feasibility audit of the unmodified GLB -> Assimp -> FBX route. Missing
// features are observations, not silently accepted production conversions.
#include "material_extension_probe.h"
#include "material_provenance_probe.h"
#include "scene_provenance_probe.h"

static void auditMaps(const char *input, const std::filesystem::path &directory, bool declareMetres=false, bool customMaterial=false, int writerExtension=0, int materialVariant=0) {
    require(!std::filesystem::exists(directory),"audit output directory already exists");
    const auto provenance=writerExtension==3?readMaterialProvenance(input):std::string();
    Assimp::Importer importer;
    const auto *imported=importer.ReadFile(input,0);
    require(imported && imported->mNumMeshes==1 && imported->mNumMaterials>=1 && imported->mNumTextures==5,"expected authored five-image maps fixture");
    std::unique_ptr<aiScene> owned(importer.GetOrphanedScene());
    auto *source=owned.get();
    if(declareMetres) {
        // FBX UnitScaleFactor is centimetres per source unit; GLB uses metres.
        if(!source->mMetaData) source->mMetaData=new aiMetadata;
        for(auto key:{"UnitScaleFactor","OriginalUnitScaleFactor"})
            if(!source->mMetaData->Set(key,100.0)) source->mMetaData->Add(key,100.0);
    }
    auto *sourceMaterial=source->mMaterials[source->mMeshes[0]->mMaterialIndex];
    if(writerExtension==3)require(sourceMaterial->AddBinaryProperty(provenance.data(),unsigned(provenance.size()),"gcad.texture_source_inputs.json",0,0,aiPTI_Buffer)==AI_SUCCESS,"cannot attach source provenance");
    if(materialVariant)varyMaterialExtensionFixture(sourceMaterial,materialVariant);
    if(writerExtension) {
        int version=writerExtension;
        sourceMaterial->AddProperty(&version,1,"gcad.extension.version",0,0);
    }
    if(customMaterial) {
        float roughness=.75f;
        sourceMaterial->AddProperty(&roughness,1,"gcad_pbr_roughness",0,0);
        float recovered=0;
        require(sourceMaterial->Get("gcad_pbr_roughness",0,0,recovered)==AI_SUCCESS && recovered==roughness,"custom material property was not set");
    }
    for(auto type:{aiTextureType_BASE_COLOR,aiTextureType_METALNESS,aiTextureType_DIFFUSE_ROUGHNESS,aiTextureType_NORMALS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE})
        require(sourceMaterial->GetTextureCount(type)==1,"source PBR map missing before FBX writing");
    require(std::filesystem::create_directory(directory),"cannot create audit directory");
    auto path=directory/"converted.fbx";
    Assimp::Exporter exporter;
    if(exporter.Export(source,"fbx",path.string())!=AI_SUCCESS) throw std::runtime_error(exporter.GetErrorString());
    ufbx_load_opts options{}; ufbx_error error{};
    Scene scene(ufbx_load_file(path.string().c_str(),&options,&error),ufbx_free_scene);
    require(bool(scene),"independent converted FBX load failed");
    require(scene->meshes.count==1 && scene->meshes.data[0]->materials.count==1,"converted topology/material assignment changed");
    auto *material=scene->meshes.data[0]->materials.data[0];
    if(writerExtension) {
        require(ufbx_find_int(&material->props,"gcad|PbrVersion",0)==writerExtension,"writer extension absent");
        const char* names[]={"gcad|Roughness","gcad|Metallic","gcad|Occlusion"};
        const aiTextureType types[]={aiTextureType_DIFFUSE_ROUGHNESS,aiTextureType_METALNESS,aiTextureType_LIGHTMAP};
        const char* channels[]={"gcad|RoughnessChannel","gcad|MetallicChannel","gcad|OcclusionChannel"};
        const int expectedChannels[]={1,2,0};
        for(unsigned i=0;i<3;++i) {
            unsigned count=0;
            for(const auto& connection:material->textures) {
                if(std::strcmp(connection.material_prop.data,names[i]))continue;
                ++count;
                aiString path;require(sourceMaterial->GetTexture(types[i],0,&path)==AI_SUCCESS,"source extension map absent");
                auto* expected=source->GetEmbeddedTexture(path.C_Str());
                auto* texture=connection.texture;
                require(expected && expected->mHeight==0 && texture && texture->content.size==expected->mWidth &&
                    !std::memcmp(texture->content.data,expected->pcData,expected->mWidth),"extension map bytes differ");
            }
            require(count==1,"extension map connection absent or ambiguous");
            require(ufbx_find_int(&material->props,channels[i],-1)==expectedChannels[i],"packed map channel lost");
        }
        float roughness=0,metallic=0;
        require(sourceMaterial->Get(AI_MATKEY_ROUGHNESS_FACTOR,roughness)==AI_SUCCESS && sourceMaterial->Get(AI_MATKEY_METALLIC_FACTOR,metallic)==AI_SUCCESS,"source factors absent");
        require(ufbx_find_real(&material->props,"gcad|Roughness",-1)==roughness && ufbx_find_real(&material->props,"gcad|Metallic",-1)==metallic,"extension factors differ");
        if(writerExtension>=2) {
            verifyMaterialExtensionV2(source,sourceMaterial,material);
            std::cout<<rejectMaterialPropertyChanges(source,sourceMaterial,material)<<" changed imported-property controls rejected and restored\n";
        }
        if(writerExtension==3) {
            verifyMaterialProvenance(material,provenance);
            auto* prop=ufbx_find_prop(&material->props,"gcad|TextureSourceInputsJson");
            auto original=prop->value_str;
            --prop->value_str.length;
            bool rejected=false;try{verifyMaterialProvenance(material,provenance);}catch(const std::runtime_error&){rejected=true;}
            prop->value_str=original;require(rejected,"provenance truncation control passed");
            verifyMaterialProvenance(material,provenance);
            std::ofstream preserved(directory/"provenance.json",std::ios::binary);
            preserved.write(provenance.data(),provenance.size());require(bool(preserved),"cannot write provenance audit");
            std::cout<<"Source provenance JSON preserved exactly: "<<provenance.size()<<" bytes; truncation control rejected\n";
        }
        std::ofstream extensionReport(directory/"extension.json");
        extensionReport<<"{\"version\":"<<writerExtension<<",\"production_ready\":false,\"custom_map_connections\":"<<(writerExtension>=2?6:3)<<",\"packed_channels\":[1,2,0],\"roughness\":"<<roughness<<",\"metallic\":"<<metallic<<",\"consumer_adapter_required\":true}\n";
        std::cout<<"Writer extension v"<<writerExtension<<" preserves "<<(writerExtension>=2?6:3)<<" custom map connections, exact image bytes, packed channels and PBR factors"<<(writerExtension>=2?", alpha/sidedness/colour/normal/AO and six sampler records":"")<<"; consumer adapter still required\n";
    }
    auto *sourceNode=source->mRootNode->FindNode("plate");
    auto *targetNode=ufbx_find_node(scene.get(),"plate");
    require(sourceNode && targetNode && targetNode->mesh,"named audit mesh missing");
    aiMatrix4x4 sourceWorld;
    for(auto *node=sourceNode;node;node=node->mParent) sourceWorld=node->mTransformation*sourceWorld;
    double sourceLow[3]={INFINITY,INFINITY,INFINITY},sourceHigh[3]={-INFINITY,-INFINITY,-INFINITY};
    double targetLow[3]={INFINITY,INFINITY,INFINITY},targetHigh[3]={-INFINITY,-INFINITY,-INFINITY};
    for(unsigned i=0;i<source->mMeshes[0]->mNumVertices;++i) {
        auto p=sourceWorld*source->mMeshes[0]->mVertices[i];
        for(unsigned k=0;k<3;++k) {sourceLow[k]=std::min(sourceLow[k],double(p[k]));sourceHigh[k]=std::max(sourceHigh[k],double(p[k]));}
    }
    for(size_t i=0;i<targetNode->mesh->vertices.count;++i) {
        auto p=ufbx_transform_position(&targetNode->geometry_to_world,targetNode->mesh->vertices.data[i]);
        for(unsigned k=0;k<3;++k) {double v=p.v[k]*scene->settings.unit_meters;targetLow[k]=std::min(targetLow[k],v);targetHigh[k]=std::max(targetHigh[k],v);}
    }
    double spanRatio[3];
    for(unsigned k=0;k<3;++k) {require(sourceHigh[k]>sourceLow[k],"audit reference span is zero");spanRatio[k]=(targetHigh[k]-targetLow[k])/(sourceHigh[k]-sourceLow[k]);}
    float sourceRoughness=0,sourceMetallic=0;
    require(sourceMaterial->Get(AI_MATKEY_ROUGHNESS_FACTOR,sourceRoughness)==AI_SUCCESS && sourceMaterial->Get(AI_MATKEY_METALLIC_FACTOR,sourceMetallic)==AI_SUCCESS,"source PBR factors missing");
    unsigned embedded=0,matching=0;
    for(size_t i=0;i<scene->textures.count;++i) {
        auto *texture=scene->textures.data[i];
        if(!texture->content.size) continue;
        ++embedded;
        for(unsigned j=0;j<source->mNumTextures;++j) {
            auto *image=source->mTextures[j];
            if(image->mHeight==0 && image->mWidth==texture->content.size &&
               std::memcmp(image->pcData,texture->content.data,image->mWidth)==0) {++matching;break;}
        }
    }
    auto connected=[](const ufbx_material_map &map) {return map.texture!=nullptr && map.texture_enabled;};
    bool maps[]={connected(material->pbr.base_color),connected(material->pbr.roughness),connected(material->pbr.metalness),connected(material->pbr.normal_map),connected(material->pbr.ambient_occlusion),connected(material->pbr.emission_color)};
    const char *names[]={"base_colour","roughness","metallic","normal","occlusion","emissive"};
    std::ofstream report(directory/"report.json");
    require(bool(report),"cannot write conversion report");
    bool customSurvived=ufbx_find_prop(&material->props,"gcad_pbr_roughness")!=nullptr;
    report<<std::boolalpha<<"{\n  \"version\": 1,\n  \"route\": \""<<(customMaterial?"assimp-material-property-api":declareMetres?"assimp-glb-to-fbx-metre-metadata":"unmodified-assimp-glb-to-fbx")<<"\",\n  \"production_ready\": false,\n  \"custom_material_property_supplied\": "<<customMaterial<<",\n  \"custom_material_property_preserved\": "<<customSurvived<<",\n  \"source_images\": "<<source->mNumTextures
          <<",\n  \"fbx_material_objects\": "<<scene->materials.count<<",\n  \"fbx_texture_objects\": "<<scene->textures.count<<",\n  \"embedded_textures\": "<<embedded<<",\n  \"byte_identical_to_source\": "<<matching
          <<",\n  \"declared_unit_metres\": "<<scene->settings.unit_meters<<",\n  \"connected_pbr_maps\": {";
    for(unsigned i=0;i<6;++i) report<<(i?", ":"")<<"\""<<names[i]<<"\": "<<maps[i];
    report<<"},\n  \"roughness_factor_present\": "<<material->pbr.roughness.has_value
          <<",\n  \"metallic_factor_present\": "<<material->pbr.metalness.has_value
          <<",\n  \"source_roughness\": "<<sourceRoughness<<",\n  \"target_roughness\": "<<material->pbr.roughness.value_real
          <<",\n  \"source_metallic\": "<<sourceMetallic<<",\n  \"target_metallic\": "<<material->pbr.metalness.value_real
          <<",\n  \"world_span_ratio_metres\": ["<<spanRatio[0]<<", "<<spanRatio[1]<<", "<<spanRatio[2]<<"]\n}\n";
    require(bool(report),"conversion report write failed");
    if(declareMetres) for(double ratio:spanRatio) require(std::abs(ratio-1)<1e-6,"metre metadata did not preserve world scale");
    std::cout<<(declareMetres?"Metre-metadata":"Raw")<<" FBX map audit: "<<embedded<<" embedded textures, "<<matching<<" byte-identical source payloads; connected maps:";
    for(unsigned i=0;i<6;++i) std::cout<<" "<<names[i]<<"="<<maps[i];
    std::cout<<"; declared metre scale "<<scene->settings.unit_meters<<"; world span ratios "<<spanRatio[0]<<", "<<spanRatio[1]<<", "<<spanRatio[2]<<"\n";
}

#include "skin_conversion_probe.h"
#include "unity_pose_reference.h"

int main(int argc, char **argv) {
    try {
        if(argc==4 && std::string(argv[1])=="--unity-pose-reference") {writeUnityPoseReference(argv[2],argv[3]);return 0;}
        if(argc==3 && std::string(argv[1])=="--audit-prepared-scene") {auditPreparedScene(argv[2]);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-scene-instance-skins") {auditSceneProvenance(argv[2],argv[3],true,true);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-scene-merged-skin") {auditSceneProvenance(argv[2],argv[3],true);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-scene-provenance-v3") {auditSceneProvenance(argv[2],argv[3]);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-skin") {auditSkin(argv[2],argv[3],false);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-skin-metres") {auditSkin(argv[2],argv[3],true);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps") {auditMaps(argv[2],argv[3]);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-metres") {auditMaps(argv[2],argv[3],true);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-api") {auditMaps(argv[2],argv[3],true,true);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-extension") {auditMaps(argv[2],argv[3],true,false,true);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-extension-v2") {auditMaps(argv[2],argv[3],true,false,2);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-extension-v3") {auditMaps(argv[2],argv[3],true,false,3);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-extension-v2-blend") {auditMaps(argv[2],argv[3],true,false,2,1);return 0;}
        if(argc==4 && std::string(argv[1])=="--audit-maps-extension-v2-opaque") {auditMaps(argv[2],argv[3],true,false,2,2);return 0;}
        require(argc == 2 || (argc == 3 && std::string(argv[2]) == "--verify"), "usage: gcad_fbx_probe OUTPUT_DIRECTORY [--verify]");
        std::filesystem::path directory(argv[1]);
        const bool verifyOnly = argc == 3;
        if (!verifyOnly) {
            std::filesystem::create_directories(directory);
            texture(directory / "checker.bmp");
        }
        for (bool skinned : {false, true}) {
            auto path = directory / (skinned ? "skinned.fbx" : "static.fbx");
            if (!verifyOnly) {
                auto scene = fixture(skinned);
                Assimp::Exporter exporter;
                if (exporter.Export(scene.get(), "fbx", path.string()) != AI_SUCCESS)
                    throw std::runtime_error(exporter.GetErrorString());
            }
            verify(path, skinned);
        }
        return 0;
    } catch (const std::exception &e) {
        std::cerr << "FBX feasibility failed: " << e.what() << '\n';
        return 1;
    }
}
