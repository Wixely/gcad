// References and shapes for the audit's STEP/LINEAR TRS and matrix-skin route.
static void verifySourceRigContracts(const rapidjson::Document& doc) {
    auto reference=[&](const rapidjson::Value& value,const char* array,const char* diagnostic)->const rapidjson::Value& {
        require(value.IsUint() && doc.HasMember(array) && doc[array].IsArray() && value.GetUint()<doc[array].Size(),diagnostic);
        return doc[array][value.GetUint()];
    };
    auto shape=[&](const rapidjson::Value& accessor,const char* type,const char* diagnostic) {
        require(accessor.IsObject() && accessor.HasMember("type") && accessor["type"].IsString() &&
            std::string(accessor["type"].GetString(),accessor["type"].GetStringLength())==type &&
            sourceUnsigned(accessor,"componentType",0)==5126 &&
            (!accessor.HasMember("normalized") || (accessor["normalized"].IsBool() && !accessor["normalized"].GetBool())),diagnostic);
    };
    if(doc.HasMember("skins")) {
        require(doc["skins"].IsArray(),"source skins must be an array");
        for(const auto& skin:doc["skins"].GetArray()) {
            require(skin.IsObject() && skin.HasMember("joints") && skin["joints"].IsArray() && !skin["joints"].Empty(),"source skin joints invalid");
            std::set<unsigned> joints;
            for(const auto& joint:skin["joints"].GetArray()) {
                reference(joint,"nodes","source skin joint reference invalid");
                require(joints.insert(joint.GetUint()).second,"duplicate source skin joint");
            }
            if(skin.HasMember("skeleton"))reference(skin["skeleton"],"nodes","source skin skeleton reference invalid");
            if(skin.HasMember("inverseBindMatrices")) {
                const auto& accessor=reference(skin["inverseBindMatrices"],"accessors","source skin bind reference invalid");
                shape(accessor,"MAT4","source skin bind shape invalid");
                require(sourceUnsigned(accessor,"count",0)>=joints.size(),"source skin bind count too small");
            }
        }
    }
    if(doc.HasMember("nodes"))for(const auto& node:doc["nodes"].GetArray()) {
        if(node.HasMember("mesh"))reference(node["mesh"],"meshes","source node mesh reference invalid");
        if(node.HasMember("skin")) {
            reference(node["skin"],"skins","source node skin reference invalid");
            require(node.HasMember("mesh"),"source skinned node has no mesh");
        }
    }
    if(doc.HasMember("animations"))for(const auto& clip:doc["animations"].GetArray()) {
        const auto& samplers=clip["samplers"]; // Types/interpolation checked by caller.
        for(const auto& sampler:samplers.GetArray()) {
            require(sampler.HasMember("input") && sampler.HasMember("output"),"source animation accessor missing");
            shape(reference(sampler["input"],"accessors","source animation input reference invalid"),"SCALAR","source animation time shape invalid");
            reference(sampler["output"],"accessors","source animation output reference invalid");
        }
        require(clip.HasMember("channels") && clip["channels"].IsArray(),"source animation channels invalid");
        std::set<std::pair<unsigned,std::string>> targets;
        for(const auto& channel:clip["channels"].GetArray()) {
            require(channel.IsObject() && channel.HasMember("sampler") && channel["sampler"].IsUint() && channel["sampler"].GetUint()<samplers.Size(),"source animation sampler reference invalid");
            require(channel.HasMember("target") && channel["target"].IsObject(),"source animation target invalid");
            const auto& target=channel["target"];
            require(target.HasMember("node") && target.HasMember("path") && target["path"].IsString(),"source animation target invalid");
            reference(target["node"],"nodes","source animation node reference invalid");
            std::string path(target["path"].GetString(),target["path"].GetStringLength());
            require(path=="translation" || path=="rotation" || path=="scale","source animation path unsupported by audit");
            require(targets.emplace(target["node"].GetUint(),path).second,"duplicate source animation target");
            const auto& sampler=samplers[channel["sampler"].GetUint()];
            const auto& input=doc["accessors"][sampler["input"].GetUint()];
            const auto& output=doc["accessors"][sampler["output"].GetUint()];
            shape(output,path=="rotation"?"VEC4":"VEC3","source animation output shape invalid");
            require(sourceUnsigned(input,"count",0)==sourceUnsigned(output,"count",0),"source animation key counts differ");
        }
    }
}
