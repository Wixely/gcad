// Independent ufbx checks of the explicit, experimental gcad material contract.
#include <assimp/GltfMaterial.h>
#include <vector>
#include <utility>

static void varyMaterialExtensionFixture(aiMaterial* material, int variant) {
    float normal=1.3f,ao=.8f,cutoff=.25f,opacity=.6f;
    int sides=0;
    aiString mode(variant==2?"OPAQUE":"BLEND");
    aiColor4D base(.2f,.3f,.4f,opacity);aiColor3D emission(.8f,.7f,.6f);
    material->AddProperty(&normal,1,AI_MATKEY_GLTF_TEXTURE_SCALE(aiTextureType_NORMALS,0));
    material->AddProperty(&ao,1,AI_MATKEY_GLTF_TEXTURE_STRENGTH(aiTextureType_LIGHTMAP,0));
    material->AddProperty(&cutoff,1,AI_MATKEY_GLTF_ALPHACUTOFF);
    material->AddProperty(&opacity,1,AI_MATKEY_OPACITY);
    material->AddProperty(&sides,1,AI_MATKEY_TWOSIDED);
    material->AddProperty(&mode,AI_MATKEY_GLTF_ALPHAMODE);
    material->AddProperty(&base,1,AI_MATKEY_BASE_COLOR);
    material->AddProperty(&emission,1,AI_MATKEY_COLOR_EMISSIVE);
    const aiTextureType slots[]={aiTextureType_BASE_COLOR,aiTextureType_DIFFUSE_ROUGHNESS,aiTextureType_METALNESS,aiTextureType_NORMALS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE};
    const int wraps[]={aiTextureMapMode_Wrap,aiTextureMapMode_Clamp,aiTextureMapMode_Mirror};
    const int filters[]={9728,9729,9984,9985,9986,9987};
    for(unsigned i=0;i<6;++i) {
        int u=wraps[i%3],v=wraps[(i+1)%3],mag=i%2?9728:9729,min=filters[i];
        material->AddProperty(&u,1,AI_MATKEY_MAPPINGMODE_U(slots[i],0));
        material->AddProperty(&v,1,AI_MATKEY_MAPPINGMODE_V(slots[i],0));
        material->AddProperty(&mag,1,AI_MATKEY_GLTF_MAPPINGFILTER_MAG(slots[i],0));
        material->AddProperty(&min,1,AI_MATKEY_GLTF_MAPPINGFILTER_MIN(slots[i],0));
    }
}

static void verifyMaterialExtensionV2(const aiScene* source, const aiMaterial* input, const ufbx_material* output,bool requireSixMaps=true,bool external=false) {
    auto scalar=[&](const char* name, ai_real expected) {
        auto* prop=ufbx_find_prop(&output->props,name);
        require(prop && std::isfinite(prop->value_real) && prop->value_real==expected,"extension scalar absent or changed");
    };
    ai_real normal=1,ao=1,cutoff=.5,alpha=1;
    input->Get(AI_MATKEY_GLTF_TEXTURE_SCALE(aiTextureType_NORMALS,0),normal);
    input->Get(AI_MATKEY_GLTF_TEXTURE_STRENGTH(aiTextureType_LIGHTMAP,0),ao);
    input->Get(AI_MATKEY_GLTF_ALPHACUTOFF,cutoff);
    input->Get(AI_MATKEY_OPACITY,alpha);
    scalar("gcad|NormalScale",normal);scalar("gcad|OcclusionStrength",ao);
    scalar("gcad|AlphaCutoff",cutoff);scalar("gcad|BaseAlpha",alpha);
    int sides=0;input->Get(AI_MATKEY_TWOSIDED,sides);
    require(ufbx_find_int(&output->props,"gcad|DoubleSided",-1)==sides,"double-sided state changed");
    aiString mode("OPAQUE");input->Get(AI_MATKEY_GLTF_ALPHAMODE,mode);
    auto* modeProp=ufbx_find_prop(&output->props,"gcad|AlphaMode");
    require(modeProp && std::string(modeProp->value_str.data,modeProp->value_str.length)==mode.C_Str(),"alpha mode changed");
    aiColor4D base(1,1,1,1);aiColor3D emission(0,0,0);
    input->Get(AI_MATKEY_BASE_COLOR,base);input->Get(AI_MATKEY_COLOR_EMISSIVE,emission);
    auto colour=[&](const char* name,double r,double g,double b) {
        auto* prop=ufbx_find_prop(&output->props,name);
        require(prop && prop->value_vec3.x==r && prop->value_vec3.y==g && prop->value_vec3.z==b,"colour factor changed");
    };
    colour("gcad|BaseColour",base.r,base.g,base.b);colour("gcad|Emission",emission.r,emission.g,emission.b);
    const aiTextureType slots[]={aiTextureType_BASE_COLOR,aiTextureType_DIFFUSE_ROUGHNESS,aiTextureType_METALNESS,aiTextureType_NORMALS,aiTextureType_LIGHTMAP,aiTextureType_EMISSIVE};
    const char* names[]={"BaseColour","Roughness","Metallic","Normal","Occlusion","Emission"};
    for(unsigned i=0;i<6;++i) {
        const std::string prefix=std::string("gcad|")+names[i];
        if(!requireSixMaps && input->GetTextureCount(slots[i])==0) {
            for(const auto& connection:output->textures)require(prefix!=connection.material_prop.data,"unexpected map connection");
            continue;
        }
        require(input->GetTextureCount(slots[i])==1,"multiple maps in one material slot");
        aiString path;require(input->GetTexture(slots[i],0,&path)==AI_SUCCESS,"expected six source maps");
        auto* expected=source->GetEmbeddedTexture(path.C_Str());
        unsigned count=0;
        for(const auto& connection:output->textures) {
            if(prefix!=connection.material_prop.data)continue;
            ++count;auto* t=connection.texture;
            if(external) {
                require(t && !t->content.size &&
                    std::string(t->relative_filename.data,t->relative_filename.length)==path.C_Str(),
                    "external map connection path changed or image embedded");
            } else {
                require(t && expected && expected->mHeight==0 && t->content.size==expected->mWidth &&
                    !std::memcmp(t->content.data,expected->pcData,expected->mWidth),"v2 map connection bytes changed");
            }
        }
        require(count==1,"v2 map connection absent or duplicated");
        int u=0,v=0,mag=-1,min=-1,uv=0;
        input->Get(AI_MATKEY_MAPPINGMODE_U(slots[i],0),u);input->Get(AI_MATKEY_MAPPINGMODE_V(slots[i],0),v);
        input->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MAG(slots[i],0),mag);input->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MIN(slots[i],0),min);
        input->Get(AI_MATKEY_UVWSRC(slots[i],0),uv);
        auto wrap=[](int value){switch(value){case aiTextureMapMode_Wrap:return 10497;case aiTextureMapMode_Clamp:return 33071;case aiTextureMapMode_Mirror:return 33648;default:throw std::runtime_error("unsupported source wrap");}};
        for(const auto& pair:std::vector<std::pair<std::string,int>>{{"WrapU",wrap(u)},{"WrapV",wrap(v)},{"MagFilter",mag},{"MinFilter",min},{"UvSet",uv}})
            require(ufbx_find_int(&output->props,(prefix+"|"+pair.first).c_str(),-2)==pair.second,"sampler or UV set changed");
    }
}

// Negative controls modify the imported in-memory property, then restore it.
// These test verifier sensitivity, not a second FBX writer or engine renderer.
static unsigned rejectMaterialPropertyChanges(const aiScene* source, const aiMaterial* input, const ufbx_material* output,bool external=false) {
    std::vector<std::pair<std::string,char>> fields={
        {"gcad|NormalScale",'r'},{"gcad|OcclusionStrength",'r'},
        {"gcad|AlphaCutoff",'r'},{"gcad|BaseAlpha",'r'},
        {"gcad|DoubleSided",'i'},{"gcad|AlphaMode",'s'},
        {"gcad|BaseColour",'v'},{"gcad|Emission",'v'},
    };
    for(auto name:{"BaseColour","Roughness","Metallic","Normal","Occlusion","Emission"})
        for(auto field:{"WrapU","WrapV","MagFilter","MinFilter","UvSet"})
            fields.push_back({std::string("gcad|")+name+"|"+field,'i'});
    for(const auto& field:fields) {
        auto* prop=ufbx_find_prop(&output->props,field.first.c_str());
        require(prop!=nullptr,"negative control property absent");
        auto original=*prop;
        switch(field.second) {
            case 'r':prop->value_real+=.125;break;
            case 'i':prop->value_int^=1;break;
            case 's':prop->value_str={"INVALID",7};break;
            case 'v':prop->value_vec3.x+=.125;break;
        }
        bool rejected=false;
        try {verifyMaterialExtensionV2(source,input,output,true,external);}catch(const std::runtime_error&){rejected=true;}
        *prop=original;
        require(rejected,"material verifier missed a changed property");
    }
    verifyMaterialExtensionV2(source,input,output,true,external);
    return unsigned(fields.size());
}
