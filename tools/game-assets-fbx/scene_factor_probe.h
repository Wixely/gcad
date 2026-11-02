// Core numeric declarations, checked before the float-based native importer.
static void verifySourceFactors(const rapidjson::Document& doc) {
    const double limit=std::numeric_limits<float>::max();
    auto number=[&](const rapidjson::Value& value,double low,double high,const char* diagnostic) {
        require(value.IsNumber(),diagnostic);double n=value.GetDouble();
        require(std::isfinite(n) && n>=low && n<=high,diagnostic);return n;
    };
    auto scalar=[&](const rapidjson::Value& object,const char* key,double low,double high,const char* diagnostic) {
        if(object.HasMember(key))number(object[key],low,high,diagnostic);
    };
    auto vector=[&](const rapidjson::Value& object,const char* key,unsigned size,double low,double high,const char* diagnostic) {
        if(!object.HasMember(key))return;
        const auto& value=object[key];require(value.IsArray() && value.Size()==size,diagnostic);
        for(const auto& item:value.GetArray())number(item,low,high,diagnostic);
    };
    if(doc.HasMember("materials"))for(const auto& material:doc["materials"].GetArray()) {
        if(material.HasMember("pbrMetallicRoughness")) {
            const auto& pbr=material["pbrMetallicRoughness"];
            vector(pbr,"baseColorFactor",4,0,1,"source base color factor invalid");
            scalar(pbr,"metallicFactor",0,1,"source metallic factor invalid");
            scalar(pbr,"roughnessFactor",0,1,"source roughness factor invalid");
        }
        vector(material,"emissiveFactor",3,0,1,"source emissive factor invalid");
        scalar(material,"alphaCutoff",0,limit,"source alpha cutoff invalid");
        if(material.HasMember("alphaMode")) {
            const auto& mode=material["alphaMode"];
            require(mode.IsString(),"source alpha mode invalid");
            std::string s(mode.GetString(),mode.GetStringLength());
            require(s=="OPAQUE" || s=="MASK" || s=="BLEND","source alpha mode unsupported by audit");
        }
        if(material.HasMember("doubleSided"))require(material["doubleSided"].IsBool(),"source double-sided flag invalid");
        if(material.HasMember("normalTexture"))scalar(material["normalTexture"],"scale",-limit,limit,"source normal scale invalid");
        if(material.HasMember("occlusionTexture"))scalar(material["occlusionTexture"],"strength",0,1,"source occlusion strength invalid");
    }
    if(!doc.HasMember("nodes"))return;
    for(const auto& node:doc["nodes"].GetArray()) {
        require(!node.HasMember("matrix") || (!node.HasMember("translation") && !node.HasMember("rotation") && !node.HasMember("scale")),"source matrix and TRS conflict");
        vector(node,"matrix",16,-limit,limit,"source node matrix invalid");
        vector(node,"translation",3,-limit,limit,"source node translation invalid");
        vector(node,"scale",3,-limit,limit,"source node scale invalid");
        vector(node,"rotation",4,-1,1,"source node rotation invalid");
        if(node.HasMember("rotation")) {
            double norm=0;for(const auto& item:node["rotation"].GetArray()){double n=item.GetDouble();norm+=n*n;}
            require(std::abs(norm-1)<=1e-4,"source node quaternion not unit length");
        }
        if(node.HasMember("matrix")) {
            const auto& m=node["matrix"];
            require(m[3].GetDouble()==0 && m[7].GetDouble()==0 && m[11].GetDouble()==0 && m[15].GetDouble()==1,"source node matrix not affine");
        }
    }
    if(doc.HasMember("animations"))for(const auto& clip:doc["animations"].GetArray())for(const auto& channel:clip["channels"].GetArray())
        require(!doc["nodes"][channel["target"]["node"].GetUint()].HasMember("matrix"),"source animated node has matrix");
}
