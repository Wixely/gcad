static void verifySceneMaterialContracts(const aiScene* source,const ufbx_scene* converted,bool external=false) {
    std::set<unsigned> used;for(unsigned m=0;m<source->mNumMeshes;++m)used.insert(source->mMeshes[m]->mMaterialIndex);
    size_t maps=0,controls=0;
    for(unsigned index:used) {
        auto* input=source->mMaterials[index];aiString name;input->Get(AI_MATKEY_NAME,name);
        const ufbx_material* output=nullptr;
        for(auto* material:converted->materials)if(std::string(material->name.data,material->name.length)==name.C_Str()) {
            require(!output,"duplicate material identity");output=material;
        }
        require(output,"source material missing from FBX");
        auto verify=[&]() {
            verifyMaterialExtensionV2(source,input,output,false,external);
            if(external) {
                const aiMaterialProperty* provenance=nullptr;
                for(unsigned p=0;p<input->mNumProperties;++p)
                    if(input->mProperties[p]->mKey==aiString("gcad.texture_source_inputs.json"))provenance=input->mProperties[p];
                require(provenance,"prepared material provenance missing");
                verifyMaterialProvenance(output,std::string(provenance->mData,provenance->mDataLength));
            }
            ai_real roughness=1,metallic=0;input->Get(AI_MATKEY_ROUGHNESS_FACTOR,roughness);input->Get(AI_MATKEY_METALLIC_FACTOR,metallic);
            for(const auto& item:std::vector<std::pair<const char*,double>>{{"gcad|Roughness",roughness},{"gcad|Metallic",metallic}}) {
                auto* prop=ufbx_find_prop(&output->props,item.first);
                require(prop && prop->value_real==item.second,"scene PBR factor changed");
            }
            require(ufbx_find_int(&output->props,"gcad|RoughnessChannel",-1)==1 && ufbx_find_int(&output->props,"gcad|MetallicChannel",-1)==2 && ufbx_find_int(&output->props,"gcad|OcclusionChannel",-1)==0,"scene packed map channel changed");
        };
        verify();unsigned present=0;
        for(auto type:{aiTextureType_BASE_COLOR,aiTextureType_DIFFUSE_ROUGHNESS,aiTextureType_METALNESS,aiTextureType_NORMALS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE})present+=input->GetTextureCount(type);
        maps+=present;
        if(present==6)controls+=rejectMaterialPropertyChanges(source,input,output,external);
        auto reject=[&]() {bool rejected=false;try{verify();}catch(const std::runtime_error&){rejected=true;}require(rejected,"scene material corruption control accepted");++controls;};
        for(auto key:{"gcad|Roughness","gcad|Metallic"}) {
            auto* prop=ufbx_find_prop(&output->props,key);auto value=prop->value_real;prop->value_real+=.125;reject();prop->value_real=value;
        }
        for(auto key:{"gcad|RoughnessChannel","gcad|MetallicChannel","gcad|OcclusionChannel"}) {
            auto* prop=ufbx_find_prop(&output->props,key);require(prop,"scene channel property missing");auto value=prop->value_int;prop->value_int+=1;reject();prop->value_int=value;
        }
        if(present)for(const auto& connection:output->textures)if(std::string(connection.material_prop.data,connection.material_prop.length)=="gcad|BaseColour") {
            auto* texture=connection.texture;require(texture,"material texture control missing");
            if(external) {
                auto path=texture->relative_filename;
                texture->relative_filename={"textures/wrong.png",18};reject();texture->relative_filename=path;
                auto content=texture->content;texture->content={"x",1};reject();texture->content=content;
            } else {
                require(texture->content.size,"material payload control missing");
                auto size=texture->content.size;--texture->content.size;reject();texture->content.size=size;
            }
            break;
        }
        if(external) {
            auto* provenance=ufbx_find_prop(&output->props,"gcad|TextureSourceInputsJson");
            require(provenance,"prepared provenance control missing");
            auto saved=provenance->value_str;provenance->value_str={"invalid",7};reject();provenance->value_str=saved;
        }
        verify();
    }
    std::cout<<"Scene material contract: "<<used.size()<<" materials, "<<maps
        <<(external?" custom map connections with exact external paths and provenance; ":" custom map connections with exact embedded bytes; ")
        <<controls<<" altered material controls rejected and restored\n";
}
