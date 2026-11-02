static void verifyAuthoredMaps(const char *path) {
    Assimp::Importer importer; auto *scene=importer.ReadFile(path,0);
    require(scene && scene->mNumMeshes==1 && scene->mNumTextures==5,"authored maps scene changed");
    auto *material=scene->mMaterials[scene->mMeshes[0]->mMaterialIndex];
    const std::vector<unsigned char> colour={255,32,16,64,16,255,32,128,32,16,255,192,128,128,128,255};
    const std::vector<unsigned char> numeric={128,64,255,255,64,128,255,255,192,192,255,255,255,128,255,255};
    for(auto type:{aiTextureType_BASE_COLOR,aiTextureType_METALNESS,aiTextureType_NORMALS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE}) {
        auto image=atlasImage(scene,material,type);
        require(image.width==2 && image.height==2,"authored map dimensions changed");
        auto expected=(type==aiTextureType_BASE_COLOR || type==aiTextureType_EMISSIVE)?colour:numeric;
        for(unsigned p=0;p<4;++p) {
            if(type==aiTextureType_METALNESS) {expected[p*4]=255;expected[p*4+1]=numeric[p*4];expected[p*4+2]=numeric[p*4];}
            if(type==aiTextureType_NORMALS) expected[p*4+1]=255-numeric[p*4+1];
        }
        require(image.pixels==expected,"authored PNG pixels/packing/convention changed");
    }
    std::ifstream file(path,std::ios::binary);
    std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(file)),{});
    require(bytes.size()>=20,"truncated maps GLB");
    uint32_t length=uint32_t(bytes[12])|(uint32_t(bytes[13])<<8)|(uint32_t(bytes[14])<<16)|(uint32_t(bytes[15])<<24);
    require(length<=bytes.size()-20,"invalid maps JSON length");
    rapidjson::Document doc; doc.Parse(reinterpret_cast<const char*>(bytes.data()+20),length);
    require(!doc.HasParseError() && doc.HasMember("materials") && doc["materials"].Size()==1,"maps JSON missing");
    const auto &m=doc["materials"][0];
    require(m.HasMember("extras") && m["extras"].HasMember("gcad_texture_source_inputs"),"texture provenance missing");
    const auto &records=m["extras"]["gcad_texture_source_inputs"];
    require(records.IsArray() && records.Size()==6,"texture provenance count changed");
    const char *slots[]={"base_colour","roughness","metallic","normal","occlusion","emissive"};
    for(auto slot:slots) {
        unsigned count=0;
        for(const auto &record:records.GetArray()) {
            if(std::string(record["slot"].GetString())!=slot) continue;
            ++count; bool coloured=std::string(slot)=="base_colour" || std::string(slot)=="emissive";
            require(std::string(record["project_path"].GetString())==(coloured?"examples/game-assets/maps/rgba.png":"examples/game-assets/maps/numeric.png"),"portable texture source path changed");
            require(std::string(record["sha256"].GetString())==(coloured?"d628010611cb3b8472717722715929c9d06134d025e7fa6de41619da1fc91aeb":"1340d9c390728118da8837424a08ed8c31838de9b5e54332db77096816434df2"),"texture source digest changed");
            require(std::string(record["license"].GetString())=="GPL-3.0-or-later" && std::string(record["origin"].GetString())=="gcad-test-pattern","texture declarations changed");
        }
        require(count==1,"duplicate or missing texture provenance slot");
    }
    const auto &sampler=doc["samplers"][0];
    require(sampler["wrapS"].GetUint()==33071 && sampler["wrapT"].GetUint()==33648 && sampler["magFilter"].GetUint()==9728 && sampler["minFilter"].GetUint()==9729,"authored sampler changed");
    require(std::abs(m["normalTexture"]["scale"].GetDouble()-.7)<1e-9 && std::abs(m["occlusionTexture"]["strength"].GetDouble()-.4)<1e-9,"map controls changed");
    std::cout<<"Authored maps import passed: all 80 embedded RGBA bytes, six source provenance records, packing, normal-Y and sampler controls\n";
}
