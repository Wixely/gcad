// Core PNG/JPEG metallic-roughness texture bindings for the canonical audit.
static void verifySourceTextures(const rapidjson::Document& doc) {
    auto reference=[&](const rapidjson::Value& index,const char* array,const char* diagnostic) {
        require(index.IsUint() && doc.HasMember(array) && doc[array].IsArray() && index.GetUint()<doc[array].Size(),diagnostic);
    };
    if(doc.HasMember("samplers")) {
        require(doc["samplers"].IsArray(),"source samplers must be an array");
        for(const auto& sampler:doc["samplers"].GetArray()) {
            require(sampler.IsObject(),"source sampler must be an object");
            if(sampler.HasMember("magFilter")){auto v=sourceUnsigned(sampler,"magFilter",0);require(v==9728 || v==9729,"source magnification filter invalid");}
            if(sampler.HasMember("minFilter")){auto v=sourceUnsigned(sampler,"minFilter",0);require(v==9728 || v==9729 || (v>=9984 && v<=9987),"source minification filter invalid");}
            for(auto key:{"wrapS","wrapT"})if(sampler.HasMember(key)){auto v=sourceUnsigned(sampler,key,0);require(v==33071 || v==33648 || v==10497,"source texture wrap invalid");}
        }
    }
    if(doc.HasMember("textures")) {
        require(doc["textures"].IsArray(),"source textures must be an array");
        for(const auto& texture:doc["textures"].GetArray()) {
            require(texture.IsObject() && texture.HasMember("source"),"source texture image missing");
            reference(texture["source"],"images","source texture image reference invalid");
            if(texture.HasMember("sampler"))reference(texture["sampler"],"samplers","source texture sampler reference invalid");
            require(!texture.HasMember("extensions"),"source texture extensions unsupported by audit");
        }
    }
    if(!doc.HasMember("materials")) {
        if(doc.HasMember("meshes"))for(const auto& mesh:doc["meshes"].GetArray())for(const auto& primitive:mesh["primitives"].GetArray())
            require(!primitive.HasMember("material"),"source primitive material reference invalid");
        return;
    }
    require(doc["materials"].IsArray(),"source materials must be an array");
    std::vector<std::set<uint32_t>> uvSets;
    for(const auto& material:doc["materials"].GetArray()) {
        require(material.IsObject(),"source material must be an object");std::set<uint32_t> sets;
        auto info=[&](const rapidjson::Value& object,const char* key) {
            if(!object.HasMember(key))return;
            const auto& value=object[key];require(value.IsObject() && value.HasMember("index"),"source texture info invalid");
            reference(value["index"],"textures","source material texture reference invalid");
            require(!value.HasMember("extensions"),"source texture-info extensions unsupported by audit");
            uint64_t uv=sourceUnsigned(value,"texCoord",0);require(uv<=UINT32_MAX,"source texture coordinate index invalid");sets.insert(uint32_t(uv));
        };
        if(material.HasMember("pbrMetallicRoughness")) {
            const auto& pbr=material["pbrMetallicRoughness"];require(pbr.IsObject(),"source PBR material must be an object");
            info(pbr,"baseColorTexture");info(pbr,"metallicRoughnessTexture");
        }
        info(material,"normalTexture");info(material,"occlusionTexture");info(material,"emissiveTexture");
        uvSets.push_back(std::move(sets));
    }
    if(doc.HasMember("meshes"))for(const auto& mesh:doc["meshes"].GetArray())for(const auto& primitive:mesh["primitives"].GetArray()) {
        if(!primitive.HasMember("material"))continue;
        reference(primitive["material"],"materials","source primitive material reference invalid");
        for(uint32_t set:uvSets[primitive["material"].GetUint()]) {
            auto name="TEXCOORD_"+std::to_string(set);
            require(primitive["attributes"].HasMember(name.c_str()),"source material UV binding missing on primitive");
        }
    }
}
