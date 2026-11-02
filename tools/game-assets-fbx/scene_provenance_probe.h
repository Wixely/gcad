#include <map>
#include <functional>
#include "scene_skin_probe.h"
#include "scene_animation_probe.h"
#include "scene_units_probe.h"
#include "scene_instances_probe.h"
#include "scene_material_probe.h"
#include "scene_graph_probe.h"
#include "scene_buffer_probe.h"
#include "scene_rig_contract_probe.h"
#include "scene_primitive_probe.h"
#include "scene_values_probe.h"
#include "scene_image_probe.h"
#include "scene_texture_probe.h"
#include "scene_factor_probe.h"
#include "helper_import_names.h"

static size_t verifySceneHierarchy(const aiScene* source,const ufbx_scene* converted) {
    auto tagged=[](const std::string& name){return name.rfind("gcad_audit_node_",0)==0;};
    std::map<std::string,const ufbx_node*> outputs;
    for(auto* node:converted->nodes){std::string name(node->name.data,node->name.length);
        if(tagged(name))require(outputs.emplace(name,node).second,"duplicate hierarchy node identity");}
    size_t count=0;
    std::function<void(const aiNode*)> visit=[&](const aiNode* node){
        if(tagged(node->mName.C_Str())) {
            auto found=outputs.find(node->mName.C_Str());require(found!=outputs.end(),"hierarchy node missing");auto* actual=found->second;++count;
            std::string expectedParent=node->mParent?node->mParent->mName.C_Str():"";
            std::string actualParent=actual->parent?std::string(actual->parent->name.data,actual->parent->name.length):"";
            require(tagged(expectedParent)?actualParent==expectedParent:!tagged(actualParent),"scene parent association changed");
            require((node->mNumMeshes!=0)==(actual->mesh!=nullptr),"scene group/mesh role changed");
        }
        for(unsigned i=0;i<node->mNumChildren;++i)visit(node->mChildren[i]);
    };
    visit(source->mRootNode);require(count==outputs.size(),"unexpected hierarchy nodes");return count;
}

static size_t verifySceneFaceMaterials(const aiScene* source,const ufbx_scene* converted,
    const std::vector<unsigned>& primitiveMaterials,const std::vector<std::string>& records,bool restored=false) {
    size_t faces=0,meshNodes=0;
    std::function<void(const aiNode*)> visit=[&](const aiNode* node) {
        if(node->mNumMeshes) {
            const ufbx_node* output=nullptr;
            for(auto* candidate:converted->nodes)if(std::string(candidate->name.data,candidate->name.length)==node->mName.C_Str()) {
                require(!output,"duplicate output node identity");output=candidate;
            }
            require(output && output->mesh,"mesh node attachment changed");++meshNodes;
            auto* mesh=output->mesh;size_t face=0;
            std::set<std::string> expectedBones,actualBones;
            for(unsigned p=0;p<node->mNumMeshes;++p) {
                const unsigned index=node->mMeshes[p];auto* primitive=source->mMeshes[index];
                const auto id=primitiveMaterials[index];
                const auto weights=sourceSceneWeights(primitive);
                for(unsigned b=0;b<primitive->mNumBones;++b)expectedBones.insert(primitive->mBones[b]->mName.C_Str());
                for(unsigned f=0;f<primitive->mNumFaces;++f,++face) {
                    require(face<mesh->faces.count && face<mesh->face_material.count,"merged face count changed");
                    auto slot=mesh->face_material.data[face];
                    require(slot<output->materials.count,"face material index invalid");
                    auto* material=output->materials.data[slot];
                    aiString sourceName;
                    require(source->mMaterials[primitive->mMaterialIndex]->Get(AI_MATKEY_NAME,sourceName)==AI_SUCCESS,"source face material name missing");
                    const std::string expectedName=restored?sourceName.C_Str():"gcad_audit_material_"+std::to_string(id);
                    require(std::string(material->name.data,material->name.length)==expectedName,"face material assignment changed");
                    require(ufbx_find_int(&material->props,"gcad|PbrVersion",0)==3,"writer extension absent");
                    verifyMaterialProvenance(material,records[id]);
                    const auto actual=mesh->faces.data[face];const auto& expected=primitive->mFaces[f];
                    require(actual.num_indices==expected.mNumIndices,"face corner count changed");
                    for(unsigned c=0;c<expected.mNumIndices;++c) {
                        const size_t corner=size_t(actual.index_begin)+c;
                        require(corner<mesh->vertex_position.indices.count,"face corner out of bounds");
                        auto vertex=mesh->vertex_position.indices.data[corner];
                        require(vertex<mesh->vertex_position.values.count,"face vertex out of bounds");
                        auto a=mesh->vertex_position.values.data[vertex];auto e=primitive->mVertices[expected.mIndices[c]];
                        require(a.x==e.x && a.y==e.y && a.z==e.z,"face position or winding changed");
                        auto vector=[&](const ufbx_vertex_vec3& attribute,const aiVector3D& expected,const char* error) {
                            require(attribute.exists && corner<attribute.indices.count,error);
                            auto index=attribute.indices.data[corner];require(index<attribute.values.count,error);
                            auto value=attribute.values.data[index];
                            require(std::isfinite(value.x) && std::isfinite(value.y) && std::isfinite(value.z) &&
                                std::abs(value.x-expected.x)<1e-6 && std::abs(value.y-expected.y)<1e-6 && std::abs(value.z-expected.z)<1e-6,error);
                        };
                        if(primitive->HasNormals())vector(mesh->vertex_normal,primitive->mNormals[expected.mIndices[c]],"face normal changed");
                        if(primitive->HasTextureCoords(0)) {
                            require(mesh->vertex_uv.exists && corner<mesh->vertex_uv.indices.count,"face UV missing");
                            auto uvIndex=mesh->vertex_uv.indices.data[corner];require(uvIndex<mesh->vertex_uv.values.count,"face UV invalid");
                            auto uv=mesh->vertex_uv.values.data[uvIndex];auto input=primitive->mTextureCoords[0][expected.mIndices[c]];
                            require(std::isfinite(uv.x) && std::isfinite(uv.y) && std::abs(uv.x-input.x)<1e-6 && std::abs(uv.y-input.y)<1e-6,"face UV changed");
                        }
                        if(source->mMaterials[primitive->mMaterialIndex]->GetTextureCount(aiTextureType_NORMALS) || (primitive->HasTangentsAndBitangents() && (mesh->vertex_tangent.exists || mesh->vertex_bitangent.exists))) {
                            require(primitive->HasTangentsAndBitangents(),"normal-mapped source tangent frame missing");
                            vector(mesh->vertex_tangent,primitive->mTangents[expected.mIndices[c]],"normal-mapped tangent changed or missing");
                            vector(mesh->vertex_bitangent,primitive->mBitangents[expected.mIndices[c]],"normal-mapped bitangent changed or missing");
                        }
                        verifySceneCornerSkin(weights[expected.mIndices[c]],e,mesh,corner);
                    }
                }
            }
            require(face==mesh->faces.count,"extra merged faces");faces+=face;
            for(auto* skin:mesh->skin_deformers)for(auto* cluster:skin->clusters) {
                require(cluster->bone_node,"skin bone missing");
                actualBones.insert(std::string(cluster->bone_node->name.data,cluster->bone_node->name.length));
            }
            require(actualBones==expectedBones,"merged skin bone set changed");
        }
        for(unsigned c=0;c<node->mNumChildren;++c)visit(node->mChildren[c]);
    };
    visit(source->mRootNode);
    size_t outputMeshNodes=0;for(auto* node:converted->nodes)if(node->mesh)++outputMeshNodes;
    require(outputMeshNodes==meshNodes,"extra mesh node attachments");return faces;
}
#include "helper_scene_import.h"

struct ConvertedPackage {
    std::vector<char> fbx;
    gcad::TexturePlan textures;
};
// Shared conversion and independent readback, without filesystem publication.
static ConvertedPackage convertPreparedScene(const char* input,bool auditControls=false) {
    auto imported=importScene(input,true,true);
    auto prepared=prepareScene(imported);
    const auto* source=prepared.scene.get();
    Assimp::Exporter exporter;Assimp::ExportProperties properties;
    properties.SetPropertyBool("GCAD_FBX_MERGE_SKINS",true);
    if(auditControls) {
        aiBone* conflict=nullptr;
        std::function<void(const aiNode*)> find=[&](const aiNode* node) {
            std::set<std::string> joints;
            for(unsigned p=0;p<node->mNumMeshes && !conflict;++p) {
                auto* mesh=source->mMeshes[node->mMeshes[p]];
                for(unsigned b=0;b<mesh->mNumBones;++b)
                    if(!joints.insert(mesh->mBones[b]->mName.C_Str()).second) {conflict=mesh->mBones[b];break;}
            }
            for(unsigned c=0;c<node->mNumChildren && !conflict;++c)find(node->mChildren[c]);
        };
        find(source->mRootNode);
        if(conflict) {
            auto saved=conflict->mOffsetMatrix;conflict->mOffsetMatrix.a4+=.125f;
            Assimp::Exporter control;
            auto* result=control.ExportToBlob(source,"fbx",0,&properties);
            const std::string error=control.GetErrorString();conflict->mOffsetMatrix=saved;
            require(!result && error.find("conflicting inverse binds")!=std::string::npos,
                "conflicting merged-joint binds were accepted");
            std::cout<<"Conflicting merged-joint bind control rejected and restored\n";
        }
    }
    const auto* blob=exporter.ExportToBlob(source,"fbx",0,&properties);
    require(blob,"prepared scene export failed");
    ufbx_load_opts options{};ufbx_error error{};
    Scene converted(ufbx_load_memory(blob->data,blob->size,&options,&error),ufbx_free_scene);
    require(bool(converted),"prepared scene independent readback failed");
    for(auto* mesh:converted->meshes)for(auto* skin:mesh->skin_deformers) {
        std::set<const ufbx_node*> joints;
        for(auto* cluster:skin->clusters)require(joints.insert(cluster->bone_node).second,
            "duplicate joint clusters in prepared FBX");
    }
    std::set<std::string> paths,seen;
    for(const auto& file:prepared.textures.files)paths.insert(file.relative_path);
    for(auto* texture:converted->textures) {
        require(!texture->content.size,"prepared scene still embeds image content");
        std::string path(texture->relative_filename.data,texture->relative_filename.length);
        require(paths.count(path),"prepared texture path outside manifest");seen.insert(path);
    }
    require(seen==paths,"prepared texture missing from FBX");
    for(auto* video:converted->videos)require(!video->content.size,"prepared video embeds image content");
    std::function<void(const aiNode*)> names=[&](const aiNode* node) {
        if(node!=source->mRootNode) {
            auto* output=ufbx_find_node(converted.get(),node->mName.C_Str());
            require(output,"authored node name missing after preparation");
            require(bool(node->mNumMeshes)==bool(output->mesh),"authored node mesh role changed");
            if(node->mParent!=source->mRootNode)
                require(output->parent && std::string(output->parent->name.data,output->parent->name.length)==node->mParent->mName.C_Str(),"authored parent changed");
        }
        for(unsigned c=0;c<node->mNumChildren;++c)names(node->mChildren[c]);
    };
    names(source->mRootNode);
    auto verifyFaces=[&]() {
        return verifySceneFaceMaterials(source,converted.get(),imported.primitiveMaterials,imported.records,true);
    };
    const auto faces=verifyFaces();
    unsigned controls=0;
    if(auditControls) {
        auto reject=[&]() {
            bool rejected=false;
            try {verifyFaces();}catch(const std::runtime_error&){rejected=true;}
            require(rejected,"restored-scene geometry/material corruption accepted");++controls;
        };
        for(auto* mesh:converted->meshes) {
            if(mesh->vertex_position.values.count) {
                auto saved=mesh->vertex_position.values[0];mesh->vertex_position.values[0].x+=.125;
                reject();mesh->vertex_position.values[0]=saved;
            }
            if(mesh->vertex_uv.exists && mesh->vertex_uv.values.count) {
                auto saved=mesh->vertex_uv.values[0];mesh->vertex_uv.values[0].x+=.125;
                reject();mesh->vertex_uv.values[0]=saved;
            }
            if(mesh->vertex_normal.exists && mesh->vertex_normal.values.count) {
                auto saved=mesh->vertex_normal.values[0];mesh->vertex_normal.values[0].z+=.125;
                reject();mesh->vertex_normal.values[0]=saved;
            }
            if(mesh->vertex_tangent.exists && mesh->vertex_tangent.values.count) {
                auto saved=mesh->vertex_tangent.values[0];mesh->vertex_tangent.values[0].x+=.125;
                reject();mesh->vertex_tangent.values[0]=saved;
            }
            if(mesh->materials.count>1 && mesh->face_material.count) {
                auto saved=mesh->face_material[0];mesh->face_material[0]=(saved+1)%unsigned(mesh->materials.count);
                reject();mesh->face_material[0]=saved;
            }
        }
        verifyFaces();
    }
    std::cout<<"Restored scene: "<<faces<<" ordered faces, material assignments, corner attributes and weights/binds; "
        <<controls<<" corruption controls rejected\n";
    verifySceneMaterialContracts(source,converted.get(),true);
    verifyScenePhysicalUnits(source,converted.get());
    verifySceneAnimation(source,converted.get(),true,true);
    require(!blob->next,"unexpected secondary FBX export blob");
    const auto* bytes=static_cast<const char*>(blob->data);
    return {std::vector<char>(bytes,bytes+blob->size),std::move(prepared.textures)};
}
static void auditPreparedScene(const char* input) {
    auto converted=convertPreparedScene(input,true);
    std::cout<<"PASS: combined import, authored name restoration, "<<converted.textures.files.size()
        <<" external PNG references, physical units and skin/animation playback (in-memory only)\n";
}

// Audit-only identities make material/mesh reordering observable without relying
// on user names, which may be absent or repeated. No production converter API.
static void auditSceneProvenance(const char* input,const std::filesystem::path& directory,bool mergeSkins=false,bool isolateInstances=false) {
    require(!std::filesystem::exists(directory),"audit output directory already exists");
    auto imported=importScene(input,mergeSkins,isolateInstances);
    auto& doc=imported.doc;
    auto& tagged=imported.tagged;
    auto& records=imported.records;
    auto& primitiveMaterials=imported.primitiveMaterials;
    auto* source=imported.importer->GetScene();
    aiVertexWeight* alteredWeight=nullptr;
    for(unsigned m=0;m<source->mNumMeshes && !alteredWeight;++m)for(unsigned b=0;b<source->mMeshes[m]->mNumBones && !alteredWeight;++b) {
        auto* bone=source->mMeshes[m]->mBones[b];
        for(unsigned w=0;w<bone->mNumWeights;++w)if(bone->mWeights[w].mWeight>0){alteredWeight=&bone->mWeights[w];break;}
    }
    if(alteredWeight) {
        auto saved=alteredWeight->mWeight;alteredWeight->mWeight+=.125f;
        bool rejected=false;try{verifySourceValues(doc,input,source);}catch(const std::runtime_error& e){rejected=std::string(e.what())=="imported weight differs from source";}
        alteredWeight->mWeight=saved;require(rejected,"imported source-weight control accepted or wrong refusal");
        verifySourceValues(doc,input,source);
        std::cout<<"Imported raw-weight control rejected and restored\n";
    }
    if(isolateInstances) {
        aiBone* bone=nullptr;
        for(unsigned m=0;m<source->mNumMeshes && !bone;++m)if(source->mMeshes[m]->mNumBones)bone=source->mMeshes[m]->mBones[0];
        if(bone) {
            auto reject=[&]() {
                bool rejected=false;try{verifyImportedSkinIdentities(doc,source,tagged);}catch(const std::runtime_error&){rejected=true;}
                require(rejected,"source skin identity/bind control accepted");
            };
            auto bind=bone->mOffsetMatrix;bone->mOffsetMatrix.a4+=.25f;reject();bone->mOffsetMatrix=bind;
            auto name=bone->mName;bone->mName=aiString("gcad_unmatched_audit_bone");reject();bone->mName=name;
            verifyImportedSkinIdentities(doc,source,tagged);
            std::cout<<"2 imported source-skin controls rejected and restored\n";
        }
    }
    require(std::filesystem::create_directory(directory),"cannot create audit directory");
    auto path=directory/"converted.fbx";Assimp::Exporter exporter;
    Assimp::ExportProperties properties;properties.SetPropertyBool("GCAD_FBX_MERGE_SKINS",mergeSkins);
    if(exporter.Export(source,"fbx",path.string(),0,&properties)!=AI_SUCCESS)throw std::runtime_error(std::string("scene FBX export failed: ")+exporter.GetErrorString());
    ufbx_load_opts options{};ufbx_error error{};
    Scene converted(ufbx_load_file(path.string().c_str(),&options,&error),ufbx_free_scene);
    require(bool(converted),"independent scene FBX load failed");
    auto hierarchyNodes=verifySceneHierarchy(source,converted.get());
    bool parentControl=false;
    for(auto* node:converted->nodes)if(node->parent && std::string(node->parent->name.data,node->parent->name.length).rfind("gcad_audit_node_",0)==0) {
        auto* parent=node->parent;node->parent=converted->root_node;
        bool rejected=false;try{verifySceneHierarchy(source,converted.get());}catch(const std::runtime_error&){rejected=true;}
        node->parent=parent;require(rejected,"hierarchy parent control accepted");parentControl=true;break;
    }
    verifySceneHierarchy(source,converted.get());
    std::cout<<"Scene hierarchy: "<<hierarchyNodes<<" authored nodes; "<<(parentControl?1:0)<<" parent controls rejected and restored\n";
    size_t curveKeys=0;for(auto* curve:converted->anim_curves)curveKeys+=curve->keyframes.count;
    std::cout<<"Exported animation curve keys: "<<curveKeys<<"\n";
    if(mergeSkins)for(auto* mesh:converted->meshes)require(mesh->skin_deformers.count<=1,"merged-skin writer extension absent");
    for(auto* mesh:converted->meshes) {
        std::cout<<"Skin layout: "<<mesh->num_vertices<<" vertices, "<<mesh->skin_deformers.count<<" deformers\n";
        for(auto* skin:mesh->skin_deformers) {
            size_t weighted=0;for(const auto& vertex:skin->vertices)if(vertex.num_weights)++weighted;
            std::cout<<"  "<<skin->clusters.count<<" clusters, "<<weighted<<" weighted vertices\n";
        }
    }
    if(mergeSkins && converted->meshes.count && converted->meshes.data[0]->vertex_tangent.exists) {
        aiMesh* partial=nullptr;
        std::function<void(const aiNode*)> findPartial=[&](const aiNode* node) {
            if(!partial && node->mNumMeshes>1) {
                bool complete=true;
                for(unsigned i=0;i<node->mNumMeshes;++i)complete=complete && source->mMeshes[node->mMeshes[i]]->HasTangentsAndBitangents();
                if(complete)partial=source->mMeshes[node->mMeshes[0]];
            }
            for(unsigned i=0;i<node->mNumChildren;++i)findPartial(node->mChildren[i]);
        };
        findPartial(source->mRootNode);
        if(partial) {
            auto* saved=partial->mTangents;partial->mTangents=nullptr;
            Assimp::Exporter control;
            auto* blob=control.ExportToBlob(source,"fbx",0,&properties);
            partial->mTangents=saved;
            require(!blob && std::string(control.GetErrorString()).find("incomplete tangent frames")!=std::string::npos,
                "partial merged tangent-frame control accepted or failed for another reason");
            std::cout<<"Partial merged tangent-frame control rejected and restored\n";
        }
    }
    auto faces=verifySceneFaceMaterials(source,converted.get(),primitiveMaterials,records);
    verifySceneMaterialContracts(source,converted.get());
    if(mergeSkins) {
        verifyScenePhysicalUnits(source,converted.get());
        auto unit=converted->settings.unit_meters;converted->settings.unit_meters=.01;
        bool rejected=false;try{verifyScenePhysicalUnits(source,converted.get());}catch(const std::runtime_error&){rejected=true;}
        if(source->mNumAnimations) {
            bool animationRejected=false;try{verifySceneAnimation(source,converted.get(),false,true);}catch(const std::runtime_error&){animationRejected=true;}
            require(animationRejected,"animation physical-unit control accepted");
        }
        converted->settings.unit_meters=unit;require(rejected,"centimetre metadata control accepted");
        std::cout<<"Changed physical-unit control rejected and restored\n";
        for(auto* node:converted->nodes)if(node->mesh && ufbx_matrix_determinant(&node->geometry_to_world)<0) {
            auto saved=node->geometry_to_world;
            node->geometry_to_world.m00=-saved.m00;node->geometry_to_world.m10=-saved.m10;node->geometry_to_world.m20=-saved.m20;
            bool reflectionRejected=false;
            try{verifyScenePhysicalUnits(source,converted.get());}catch(const std::runtime_error& e){reflectionRejected=std::string(e.what())=="instance determinant changed";}
            node->geometry_to_world=saved;
            require(reflectionRejected,"removed reflection control accepted or rejected for another reason");
            std::cout<<"Removed instance reflection rejected and restored\n";
            break;
        }
    }
    unsigned controls=0;
    auto reject=[&]() {
        bool rejected=false;
        try{verifySceneFaceMaterials(source,converted.get(),primitiveMaterials,records);}catch(const std::runtime_error&){rejected=true;}
        require(rejected,"altered scene control was accepted");++controls;
    };
    for(auto* node:converted->nodes)if(node->mesh && node->materials.count>1) {
        auto* mesh=node->mesh;require(mesh->face_material.count>0,"missing face material data");
        auto saved=mesh->face_material.data[0];mesh->face_material.data[0]=(saved+1)%uint32_t(node->materials.count);
        reject();mesh->face_material.data[0]=saved;break;
    }
    auto* mesh=converted->meshes.data[0];
    require(mesh->vertex_position.values.count && mesh->faces.count,"empty audit mesh");
    auto position=mesh->vertex_position.values.data[0];mesh->vertex_position.values.data[0].x+=1;
    reject();mesh->vertex_position.values.data[0]=position;
    auto face=mesh->faces.data[0];require(face.num_indices>=3,"audit controls need polygon faces");
    std::swap(mesh->vertex_position.indices.data[face.index_begin],mesh->vertex_position.indices.data[face.index_begin+1]);
    reject();std::swap(mesh->vertex_position.indices.data[face.index_begin],mesh->vertex_position.indices.data[face.index_begin+1]);
    auto* node=mesh->instances.data[0];auto* attached=node->mesh;node->mesh=nullptr;
    reject();node->mesh=attached;
    auto* property=ufbx_find_prop(&node->materials.data[0]->props,"gcad|TextureSourceInputsJson");
    require(property && property->value_str.length,"audit provenance missing");
    auto lengthBefore=property->value_str.length;--property->value_str.length;
    reject();property->value_str.length=lengthBefore;
    ufbx_mesh* skinned=nullptr;
    for(auto* candidate:converted->meshes)if(candidate->skin_deformers.count){skinned=candidate;break;}
    if(skinned) {
        auto* skin=skinned->skin_deformers.data[0];
        require(skin->weights.count && skin->clusters.count>=1,"skin controls require weighted bones");
        auto& weight=skin->weights.data[0];auto savedWeight=weight.weight;weight.weight+=.125;
        reject();weight.weight=savedWeight;
        auto* cluster=skin->clusters.data[weight.cluster_index];
        auto savedBind=cluster->geometry_to_bone;cluster->geometry_to_bone.m03+=.01;
        reject();cluster->geometry_to_bone=savedBind;
        auto* savedBone=cluster->bone_node;
        cluster->bone_node=skin->clusters.count>1?skin->clusters.data[(weight.cluster_index+1)%skin->clusters.count]->bone_node:converted->root_node;
        require(cluster->bone_node!=savedBone,"skin control replacement bone unavailable");
        reject();cluster->bone_node=savedBone;
        std::cout<<"3 skin weight/bind/identity controls rejected and restored\n";
    }
    unsigned shadingControls=0;
    auto rejectShading=[&]() {
        bool rejected=false;
        try{verifySceneFaceMaterials(source,converted.get(),primitiveMaterials,records);}catch(const std::runtime_error&){rejected=true;}
        require(rejected,"altered shading control was accepted");++shadingControls;
    };
    if(mesh->vertex_normal.exists && mesh->vertex_normal.values.count) {
        auto saved=mesh->vertex_normal.values.data[0];mesh->vertex_normal.values.data[0].x+=.25;
        rejectShading();mesh->vertex_normal.values.data[0]=saved;
    }
    if(mesh->vertex_uv.exists && mesh->vertex_uv.values.count) {
        auto saved=mesh->vertex_uv.values.data[0];mesh->vertex_uv.values.data[0].x+=.25;
        rejectShading();mesh->vertex_uv.values.data[0]=saved;
    }
    if(mesh->vertex_tangent.exists && mesh->vertex_bitangent.exists) {
        auto saved=mesh->vertex_tangent.values.data[0];mesh->vertex_tangent.values.data[0].x+=.25;
        rejectShading();mesh->vertex_tangent.values.data[0]=saved;
        auto bitangent=mesh->vertex_bitangent.values.data[0];
        mesh->vertex_bitangent.values.data[0]={-bitangent.x,-bitangent.y,-bitangent.z};
        rejectShading();mesh->vertex_bitangent.values.data[0]=bitangent;
    }
    std::cout<<shadingControls<<" altered shading controls rejected and restored\n";
    verifySceneFaceMaterials(source,converted.get(),primitiveMaterials,records);
    verifySceneAnimation(source,converted.get(),false,mergeSkins);
    if(source->mNumAnimations) {
        unsigned animationControls=2;
        auto rejectAnimation=[&]() {
            bool rejected=false;try{verifySceneAnimation(source,converted.get(),false,mergeSkins);}catch(const std::runtime_error&){rejected=true;}
            require(rejected,"changed animation control accepted");
        };
        auto* stack=converted->anim_stacks.data[0];auto end=stack->time_end;stack->time_end+=.25;
        rejectAnimation();stack->time_end=end;
        require(converted->anim_curves.count && converted->anim_curves.data[0]->keyframes.count,"animation control curve missing");
        auto& key=converted->anim_curves.data[0]->keyframes.data[0];auto value=key.value;key.value+=15;
        rejectAnimation();key.value=value;
        for(auto* curve:converted->anim_curves)if(curve->keyframes.count>1 && curve->keyframes.data[0].interpolation==UFBX_INTERPOLATION_CONSTANT_PREV && std::abs(curve->keyframes.data[0].value-curve->keyframes.data[1].value)>1e-6) {
            auto& stepKey=curve->keyframes.data[0];stepKey.interpolation=UFBX_INTERPOLATION_LINEAR;
            rejectAnimation();stepKey.interpolation=UFBX_INTERPOLATION_CONSTANT_PREV;++animationControls;break;
        }
        verifySceneAnimation(source,converted.get(),false,mergeSkins);
        std::cout<<animationControls<<" altered animation controls rejected and restored\n";
        verifySceneAnimation(source,converted.get(),true,mergeSkins);
    }
    std::cout<<"PASS: "<<records.size()<<" source materials, "<<primitiveMaterials.size()<<" imported primitives, "<<faces<<" ordered faces; node attachments, material assignments, positions/winding, corner weights/binds, 13 synthetic bone poses and exact provenance readback\n";
    std::cout<<controls<<" altered imported-scene controls rejected and restored\n";
}
