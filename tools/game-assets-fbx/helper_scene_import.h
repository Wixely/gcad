// Shared validated import preparation. Included after the source-contract checks.
// No output files are written; the importer owns the tagged source scene.
#include "helper_textures.h"
struct ImportedScene {
    rapidjson::Document doc;
    std::vector<char> tagged;
    std::unique_ptr<Assimp::Importer> importer;
    std::vector<std::string> records;
    std::vector<unsigned> primitiveMaterials;
    gcad::AuthoredNames names;
};
static ImportedScene importScene(const char* input, bool mergeSkins, bool isolateInstances) {
    auto doc=readProvenanceDocument(input);
    verifySourceSceneGraph(doc);
    verifySourceBuffers(doc,input);
    if(doc.HasMember("animations")) {
        require(doc["animations"].IsArray(),"invalid source animations");
        for(const auto& clip:doc["animations"].GetArray()) {
            require(clip.IsObject() && clip.HasMember("samplers") && clip["samplers"].IsArray(),"invalid animation samplers");
            for(const auto& sampler:clip["samplers"].GetArray())require(sampler.IsObject() &&
                (!sampler.HasMember("interpolation") || (sampler["interpolation"].IsString() && (std::string(sampler["interpolation"].GetString())=="LINEAR" || std::string(sampler["interpolation"].GetString())=="STEP"))),"scene animation audit requires STEP or LINEAR samplers");
        }
    }
    verifySourceRigContracts(doc);
    verifySourcePrimitives(doc);
    verifySourceTextures(doc);
    verifySourceFactors(doc);
    verifySourceValues(doc,input);
    verifySourceImages(doc,input);
    if(isolateInstances)std::cout<<"Isolated skinned mesh references: "<<isolateSceneSkinnedInstances(doc)<<"\n";
    require(doc.HasMember("materials") && doc["materials"].IsArray(),"source materials missing");
    std::vector<std::string> records;
    for(const auto& material:doc["materials"].GetArray()) {
        records.push_back(sourceMaterialProvenance(material));
    }
    require(doc.HasMember("meshes") && doc["meshes"].IsArray(),"source meshes missing");
    std::vector<unsigned> primitiveMaterials;
    for(const auto& mesh:doc["meshes"].GetArray()) {
        require(mesh.IsObject() && mesh.HasMember("primitives") && mesh["primitives"].IsArray(),"source primitives missing");
        for(const auto& primitive:mesh["primitives"].GetArray()) {
            require(primitive.IsObject() && primitive.HasMember("material") && primitive["material"].IsUint() &&
                primitive["material"].GetUint()<records.size(),"source material reference invalid");
            primitiveMaterials.push_back(primitive["material"].GetUint());
        }
    }
    // glTF objects are loaded lazily: source indices are NOT imported indices.
    // Tag an in-memory copy before import; keep source files untouched.
    // Retain identities through import validation; restore on the export copy.
    auto authoredNames=gcad::tagImportNames(doc);
    std::map<std::string,unsigned> materialIds,meshMaterials;
    for(unsigned i=0;i<records.size();++i) {
        auto name="gcad_audit_material_"+std::to_string(i);materialIds[name]=i;
    }
    unsigned flat=0;
    for(unsigned i=0;i<doc["meshes"].Size();++i) {
        auto name="gcad_source_mesh_"+std::to_string(i);auto& mesh=doc["meshes"][i];
        const auto count=mesh["primitives"].Size();
        for(unsigned p=0;p<count;++p)meshMaterials[name+(count>1?"-"+std::to_string(p):"")]=primitiveMaterials[flat++];
    }
    require(doc.HasMember("nodes") && doc["nodes"].IsArray(),"source nodes missing");
    rapidjson::StringBuffer json;rapidjson::Writer<rapidjson::StringBuffer> writer(json);doc.Accept(writer);
    std::ifstream file(std::filesystem::u8path(input),std::ios::binary|std::ios::ate);auto length=file.tellg();file.seekg(0);
    std::vector<char> original(static_cast<size_t>(length));file.read(original.data(),length);require(bool(file),"cannot reread GLB");
    uint32_t oldJson=0;std::memcpy(&oldJson,original.data()+12,4);
    const uint32_t padded=uint32_t((json.GetSize()+3)&~size_t(3));
    std::vector<char> tagged(20+padded+original.size()-20-oldJson,' ');
    std::copy_n(original.data(),20,tagged.data());
    uint32_t total=uint32_t(tagged.size());std::memcpy(tagged.data()+8,&total,4);std::memcpy(tagged.data()+12,&padded,4);
    std::copy_n(json.GetString(),json.GetSize(),tagged.data()+20);
    std::copy(original.begin()+20+oldJson,original.end(),tagged.begin()+20+padded);
    auto importer=std::make_unique<Assimp::Importer>();
    auto* source=importer->ReadFileFromMemory(tagged.data(),tagged.size(),0,"glb");
    require(source && source->mNumMeshes>0,"scene import failed");
    verifyImportedInterpolation(doc,source);
    verifyImportedSkinIdentities(doc,source,tagged);
    verifySourceValues(doc,input,source);
    if(mergeSkins) {
        auto* mutableSource=const_cast<aiScene*>(source);
        if(!mutableSource->mMetaData)mutableSource->mMetaData=new aiMetadata;
        for(auto key:{"UnitScaleFactor","OriginalUnitScaleFactor"})
            if(!mutableSource->mMetaData->Set(key,100.0))mutableSource->mMetaData->Add(key,100.0);
    }
    std::map<unsigned,unsigned> importedMaterialIds;
    for(unsigned i=0;i<source->mNumMaterials;++i) {
        auto* material=source->mMaterials[i];aiString name;material->Get(AI_MATKEY_NAME,name);
        auto found=materialIds.find(name.C_Str());if(found==materialIds.end())continue;
        auto id=found->second;importedMaterialIds[i]=id;int version=3;
        require(material->AddProperty(&version,1,"gcad.extension.version",0,0)==AI_SUCCESS &&
            material->AddBinaryProperty(records[id].data(),unsigned(records[id].size()),"gcad.texture_source_inputs.json",0,0,aiPTI_Buffer)==AI_SUCCESS,"cannot attach scene provenance");
    }
    primitiveMaterials.clear();
    for(unsigned i=0;i<source->mNumMeshes;++i) {
        auto* mesh=source->mMeshes[i];auto expected=meshMaterials.find(mesh->mName.C_Str());
        auto actual=importedMaterialIds.find(mesh->mMaterialIndex);
        require(expected!=meshMaterials.end() && actual!=importedMaterialIds.end() && expected->second==actual->second,"imported primitive material association changed");
        primitiveMaterials.push_back(actual->second);
        auto auditName="gcad_audit_mesh_"+std::to_string(i);
        authoredNames.meshes[auditName]=authoredNames.meshes.at(mesh->mName.C_Str());
        mesh->mName=aiString(auditName);
    }
    return {std::move(doc),std::move(tagged),std::move(importer),std::move(records),
        std::move(primitiveMaterials),std::move(authoredNames)};
}
static gcad::PreparedScene prepareScene(const ImportedScene& imported) {
    auto prepared=gcad::prepareTextures(*imported.importer->GetScene());
    // Assimp uses the sole glTF scene node as its root. The FBX writer treats
    // that root as an implicit container and does not emit it as a model.
    // Keep every authored node (including its transform/mesh) below an owned,
    // identity container, just as Assimp does for scenes with multiple roots.
    auto* root=prepared.scene->mRootNode;
    if(root && imported.names.nodes.count(root->mName.C_Str())) {
        std::string name="gcad_export_root";
        auto collision=[&]() {
            for(const auto& entry:imported.names.nodes)
                if(entry.first==name || entry.second==name)return true;
            return false;
        };
        unsigned suffix=0;
        while(collision())name="gcad_export_root_"+std::to_string(++suffix);
        auto wrapper=std::make_unique<aiNode>(name);
        wrapper->mChildren=new aiNode*[1]{root};
        wrapper->mNumChildren=1;
        root->mParent=wrapper.get();
        prepared.scene->mRootNode=wrapper.release();
    }
    gcad::restoreNames(*prepared.scene,imported.names);
    return prepared;
}
