// Canonical triangle primitive contracts and an audit work estimate. This is not
// an upper bound on allocator overhead, decoded images, or total process memory.
static void verifySourcePrimitives(const rapidjson::Document& doc) {
    if(!doc.HasMember("meshes"))return;
    require(doc["meshes"].IsArray(),"source meshes must be an array");
    std::vector<uint64_t> uses(doc["meshes"].Size(),0);
    std::vector<bool> skinned(doc["meshes"].Size(),false);
    if(doc.HasMember("nodes"))for(const auto& node:doc["nodes"].GetArray())if(node.HasMember("mesh")) {
        auto index=node["mesh"].GetUint();++uses[index];if(node.HasMember("skin"))skinned[index]=true;
    } // Rig-reference preflight ran first.
    auto accessor=[&](const rapidjson::Value& index)->const rapidjson::Value& {
        require(index.IsUint() && doc.HasMember("accessors") && doc["accessors"].IsArray() && index.GetUint()<doc["accessors"].Size(),"source primitive accessor reference invalid");
        return doc["accessors"][index.GetUint()];
    };
    auto type=[](const rapidjson::Value& a){return std::string(a["type"].GetString(),a["type"].GetStringLength());};
    auto normalized=[](const rapidjson::Value& a) {
        require(!a.HasMember("normalized") || a["normalized"].IsBool(),"source attribute normalization invalid");
        return a.HasMember("normalized") && a["normalized"].GetBool();
    };
    uint64_t total=0;size_t meshIndex=0;
    for(const auto& mesh:doc["meshes"].GetArray()) {
        require(mesh.IsObject() && mesh.HasMember("primitives") && mesh["primitives"].IsArray() && !mesh["primitives"].Empty(),"source primitive array invalid");
        uint64_t meshCost=0;
        for(const auto& primitive:mesh["primitives"].GetArray()) {
            require(primitive.IsObject() && primitive.HasMember("attributes") && primitive["attributes"].IsObject() && primitive["attributes"].HasMember("POSITION"),"source position attribute missing");
            require(sourceUnsigned(primitive,"mode",4)==4 && !primitive.HasMember("targets"),"source primitive mode or morph targets unsupported by audit");
            const auto& attributes=primitive["attributes"];const auto& positions=accessor(attributes["POSITION"]);
            if(skinned[meshIndex])require(attributes.HasMember("JOINTS_0") && attributes.HasMember("WEIGHTS_0"),"source skinned primitive attributes missing");
            uint64_t vertices=sourceUnsigned(positions,"count",0),components=0;
            std::set<std::string> names;
            for(auto it=attributes.MemberBegin();it!=attributes.MemberEnd();++it) {
                std::string name(it->name.GetString(),it->name.GetStringLength());require(names.insert(name).second,"duplicate source primitive attribute");
                const auto& a=accessor(it->value);auto shape=type(a);auto component=sourceUnsigned(a,"componentType",0);bool norm=normalized(a);
                require(sourceUnsigned(a,"count",0)==vertices,"source attribute counts differ");
                unsigned width=shape=="SCALAR"?1:shape=="VEC2"?2:shape=="VEC3"?3:shape=="VEC4"?4:0;
                require(width>0,"source primitive attribute shape invalid");components+=width;
                if(name=="POSITION" || name=="NORMAL" || name=="TANGENT")
                    require(component==5126 && !norm && shape==(name=="TANGENT"?"VEC4":"VEC3"),"source geometric attribute shape unsupported by audit");
                else if(name.rfind("JOINTS_",0)==0)
                    require(shape=="VEC4" && (component==5121 || component==5123) && !norm,"source joint attribute shape invalid");
                else if(name.rfind("WEIGHTS_",0)==0 || name.rfind("TEXCOORD_",0)==0 || name.rfind("COLOR_",0)==0) {
                    bool validShape=name.rfind("WEIGHTS_",0)==0?shape=="VEC4":name.rfind("TEXCOORD_",0)==0?shape=="VEC2":(shape=="VEC3" || shape=="VEC4");
                    require(validShape && ((component==5126 && !norm) || ((component==5121 || component==5123) && norm)),"source numeric attribute shape invalid");
                } else require(!name.empty() && name[0]=='_',"source primitive attribute unsupported by audit");
            }
            for(const auto& name:names) {
                if(name.rfind("JOINTS_",0)==0)require(names.count("WEIGHTS_"+name.substr(7))!=0,"source joint and weight attributes unpaired");
                if(name.rfind("WEIGHTS_",0)==0)require(names.count("JOINTS_"+name.substr(8))!=0,"source joint and weight attributes unpaired");
            }
            uint64_t corners=vertices;
            if(primitive.HasMember("indices")) {
                const auto& indices=accessor(primitive["indices"]);auto component=sourceUnsigned(indices,"componentType",0);
                require(type(indices)=="SCALAR" && (component==5121 || component==5123 || component==5125) && !normalized(indices),"source index accessor shape invalid");
                corners=sourceUnsigned(indices,"count",0);
            }
            require(corners>0 && corners%3==0,"source triangle corner count invalid");
            uint64_t charge=std::max(vertices,corners)*components*8+corners*4;
            require(charge<=128*1024*1024-meshCost,"source primitive expansion exceeds audit budget");meshCost+=charge;
        }
        uint64_t copies=std::max(uint64_t(1),uses[meshIndex++]);
        require(meshCost<=((128*1024*1024-total)/copies),"source instance expansion exceeds audit budget");total+=meshCost*copies;
    }
    std::cout<<"Primitive expansion estimate: "<<total<<" audit bytes (not total process memory)\n";
}
