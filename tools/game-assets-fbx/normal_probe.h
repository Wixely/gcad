// Analytic detail-transfer check, independent of the Rust baker/rasterizer.
// atlas_probe.h supplies PNG decoding from the pinned stb dependency.
static void verifyNormalDetail(const char* path) {
    Assimp::Importer importer;
    auto* scene=importer.ReadFile(path,0);
    require(scene && scene->mNumMeshes==1,"normal detail scene missing");
    auto* mesh=scene->mMeshes[0];
    require(mesh->mNumVertices==8 && mesh->mNumFaces==4 && mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0),"normal detail geometry/attributes changed");
    const float positions[8][3]={{0,0,0},{.1f,0,0},{0,0,-.1f},{.1f,0,-.1f},{.2f,0,0},{.3f,0,0},{.2f,0,-.1f},{.3f,0,-.1f}};
    const float us[8]={.0625f,.4375f,.0625f,.4375f,.9375f,.5625f,.9375f,.5625f};
    for(unsigned i=0;i<8;++i) {
        require((mesh->mVertices[i]-aiVector3D(positions[i][0],positions[i][1],positions[i][2])).Length()<1e-7,"low geometry acquired high surface displacement");
        require((mesh->mNormals[i]-aiVector3D(0,1,0)).Length()<1e-6,"low normals changed");
        require(std::abs(mesh->mTextureCoords[0][i].x-us[i])<1e-6 && std::abs(mesh->mTextureCoords[0][i].y-(i%4<2?.875f:.125f))<1e-6,"normal detail UV or mirror changed");
        require((mesh->mTangents[i]-aiVector3D(i<4?1.f:-1.f,0,0)).Length()<1e-6,"normal detail tangent direction changed");
        require((mesh->mBitangents[i]-aiVector3D(0,0,-1)).Length()<1e-6,"normal detail bitangent handedness changed");
    }
    auto* material=scene->mMaterials[mesh->mMaterialIndex];
    auto image=atlasImage(scene,material,aiTextureType_NORMALS);
    require(image.width==64 && image.height==32,"normal map dimensions changed");
    int minify=0,wrapU=0,wrapV=0;
    require(material->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MIN(aiTextureType_NORMALS,0),minify)==AI_SUCCESS && minify==9729,"normal detail mipmaps unexpectedly enabled");
    require(material->Get(AI_MATKEY_MAPPINGMODE_U(aiTextureType_NORMALS,0),wrapU)==AI_SUCCESS && wrapU==aiTextureMapMode_Clamp,"normal detail U addressing changed");
    require(material->Get(AI_MATKEY_MAPPINGMODE_V(aiTextureType_NORMALS,0),wrapV)==AI_SUCCESS && wrapV==aiTextureMapMode_Clamp,"normal detail V addressing changed");
    aiVector3D expected(-.2f,1.f,.3f);expected.Normalize();
    unsigned covered=0,padded=0,empty=0; double maxAngle=0,minFlippedAngle=180;
    for(int y=0;y<32;++y)for(int x=0;x<64;++x) {
        int chart=-1,distance=1000;
        for(int c=0;c<2;++c) {
            int left=c?36:4,right=c?59:27;
            int d=std::max({left-x,0,x-right})+std::max({4-y,0,y-27});
            if(d<distance){distance=d;chart=c;}
        }
        auto* rgba=&image.pixels[4*(y*64+x)];
        require(rgba[3]==255,"normal map alpha changed");
        if(distance>2) {
            require(rgba[0]==128 && rgba[1]==128 && rgba[2]==255,"outside normal texel is not neutral");++empty;continue;
        }
        if(distance==0)++covered;else ++padded;
        aiVector3D tangentNormal(rgba[0]/255.f*2-1,rgba[1]/255.f*2-1,rgba[2]/255.f*2-1);
        unsigned vertex=chart?4:0;
        auto reconstruct=[&](aiVector3D n) {
            auto result=mesh->mTangents[vertex]*n.x+mesh->mBitangents[vertex]*n.y+mesh->mNormals[vertex]*n.z;
            result.Normalize();return result;
        };
        auto normal=reconstruct(tangentNormal);
        double angle=std::acos(std::clamp(double(normal*expected),-1.0,1.0))*180/3.141592653589793;
        require(angle<0.4,"baked normal differs from analytic high surface slope");maxAngle=std::max(maxAngle,angle);
        tangentNormal.y=-tangentNormal.y;
        double flipped=std::acos(std::clamp(double(reconstruct(tangentNormal)*expected),-1.0,1.0))*180/3.141592653589793;
        minFlippedAngle=std::min(minFlippedAngle,flipped);
    }
    require(covered==1152 && padded==392 && empty==504,"normal detail coverage/padding changed");
    require(minFlippedAngle>20,"normal convention negative control is ineffective");
    std::cout<<"Normal detail import passed: "<<covered<<" projected and "<<padded<<" padding pixels across ordinary/mirrored charts; maximum analytic normal error "<<maxAngle<<" degrees; flipped-green control at least "<<minFlippedAngle<<" degrees\n";
}
