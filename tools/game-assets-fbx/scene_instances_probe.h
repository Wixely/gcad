// The pinned import/writer route associates bones with mesh objects. Isolate
// JSON mesh definitions per node while retaining immutable buffer accessors.
static unsigned isolateSceneSkinnedInstances(rapidjson::Document& doc) {
    require(doc.HasMember("nodes") && doc["nodes"].IsArray() && doc.HasMember("meshes") && doc["meshes"].IsArray(),"instance scene structure missing");
    std::set<unsigned> skinned,seen;
    for(const auto& node:doc["nodes"].GetArray())if(node.IsObject() && node.HasMember("skin")) {
        require(node.HasMember("mesh") && node["mesh"].IsUint() && node["mesh"].GetUint()<doc["meshes"].Size(),"skinned instance mesh invalid");
        skinned.insert(node["mesh"].GetUint());
    }
    rapidjson::StringBuffer initial;rapidjson::Writer<rapidjson::StringBuffer> writer(initial);doc.Accept(writer);
    size_t budget=initial.GetSize();unsigned copies=0;
    for(auto& node:doc["nodes"].GetArray())if(node.IsObject() && node.HasMember("mesh")) {
        require(node["mesh"].IsUint() && node["mesh"].GetUint()<doc["meshes"].Size(),"instance mesh reference invalid");
        auto original=node["mesh"].GetUint();if(!skinned.count(original))continue;
        if(seen.insert(original).second)continue;
        rapidjson::StringBuffer json;rapidjson::Writer<rapidjson::StringBuffer> measure(json);doc["meshes"][original].Accept(measure);
        require(budget+json.GetSize()+32<=16*1024*1024,"isolated instance JSON exceeds 16 MiB budget");
        budget+=json.GetSize()+32;
        rapidjson::Value copy(doc["meshes"][original],doc.GetAllocator());
        node["mesh"].SetUint(doc["meshes"].Size());doc["meshes"].PushBack(copy,doc.GetAllocator());++copies;
    }
    return copies;
}

static void verifyImportedSkinIdentities(const rapidjson::Document& doc,const aiScene* scene,const std::vector<char>& glb) {
    if(!doc.HasMember("nodes"))return;
    for(unsigned i=0;i<doc["nodes"].Size();++i) {
        const auto& node=doc["nodes"][i];if(!node.HasMember("skin"))continue;
        auto* imported=scene->mRootNode->FindNode(aiString("gcad_audit_node_"+std::to_string(i)));
        if(!imported)continue; // Nodes outside the imported scene may be unused.
        require(doc.HasMember("skins") && doc["skins"].IsArray() && node["skin"].IsUint() && node["skin"].GetUint()<doc["skins"].Size(),"source instance skin invalid");
        const auto& skin=doc["skins"][node["skin"].GetUint()];
        require(skin.IsObject() && skin.HasMember("joints") && skin["joints"].IsArray(),"source skin joints missing");
        std::map<std::string,unsigned> names;
        for(const auto& joint:skin["joints"].GetArray()) {
            require(joint.IsUint() && joint.GetUint()<doc["nodes"].Size(),"source joint reference invalid");
            require(names.emplace("gcad_audit_node_"+std::to_string(joint.GetUint()),unsigned(names.size())).second,"duplicate source skin joint");
        }
        for(unsigned m=0;m<imported->mNumMeshes;++m) {
            auto* mesh=scene->mMeshes[imported->mMeshes[m]];require(mesh->mNumBones>0,"source skin was not imported");
            for(unsigned b=0;b<mesh->mNumBones;++b) {
                auto* bone=mesh->mBones[b];auto joint=names.find(bone->mName.C_Str());
                require(joint!=names.end(),"imported instance bone differs from source skin");
                float expected[16]={1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1};
                if(skin.HasMember("inverseBindMatrices")) {
                    require(skin["inverseBindMatrices"].IsUint() && doc.HasMember("accessors") && doc["accessors"].IsArray() && skin["inverseBindMatrices"].GetUint()<doc["accessors"].Size(),"source bind accessor invalid");
                    const auto& accessor=doc["accessors"][skin["inverseBindMatrices"].GetUint()];
                    require(accessor.IsObject() && !accessor.HasMember("sparse") && accessor.HasMember("componentType") && accessor["componentType"].IsUint() && accessor["componentType"].GetUint()==5126 && accessor.HasMember("type") && accessor["type"].IsString() && std::string(accessor["type"].GetString())=="MAT4" && accessor.HasMember("count") && accessor["count"].IsUint() && joint->second<accessor["count"].GetUint(),"unsupported source bind accessor");
                    require(accessor.HasMember("bufferView") && accessor["bufferView"].IsUint() && doc.HasMember("bufferViews") && doc["bufferViews"].IsArray() && accessor["bufferView"].GetUint()<doc["bufferViews"].Size(),"source bind view invalid");
                    const auto& view=doc["bufferViews"][accessor["bufferView"].GetUint()];
                    auto number=[](const rapidjson::Value& object,const char* key,uint64_t fallback) {
                        if(!object.HasMember(key))return fallback;
                        require(object[key].IsUint(),"invalid bind buffer offset");return uint64_t(object[key].GetUint());
                    };
                    require(view.IsObject() && number(view,"buffer",~uint64_t(0))==0,"bind matrices must use embedded buffer");
                    const uint64_t stride=number(view,"byteStride",64),offset=number(accessor,"byteOffset",0)+uint64_t(joint->second)*stride;
                    require(stride>=64 && offset+64<=number(view,"byteLength",0),"bind matrix outside view");
                    uint32_t jsonLength=0;std::memcpy(&jsonLength,glb.data()+12,4);
                    const uint64_t chunk=20+uint64_t(jsonLength);require(chunk+8<=glb.size(),"bind BIN chunk missing");
                    uint32_t length=0,type=0;std::memcpy(&length,glb.data()+chunk,4);std::memcpy(&type,glb.data()+chunk+4,4);
                    const uint64_t start=number(view,"byteOffset",0)+offset;
                    require(type==0x004e4942 && start+64<=length && chunk+8+length<=glb.size(),"bind matrix outside BIN chunk");
                    std::memcpy(expected,glb.data()+chunk+8+start,64);
                }
                for(unsigned row=0;row<4;++row)for(unsigned col=0;col<4;++col)require(std::isfinite(expected[col*4+row]) && std::abs(bone->mOffsetMatrix[row][col]-expected[col*4+row])<1e-6,"imported inverse bind differs from source skin");
            }
        }
    }
}
