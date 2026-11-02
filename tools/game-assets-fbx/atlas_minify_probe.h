// Independent byte-level oracle for the 25%-coverage minification fixture.
static void verifyAtlasMinification(const char* sourcePath,const char* packedPath) {
    Assimp::Importer sourceImporter,packedImporter;
    auto* original=sourceImporter.ReadFile(sourcePath,0);auto* packedScene=packedImporter.ReadFile(packedPath,0);
    require(original && packedScene && original->mNumMeshes==1 && packedScene->mNumMeshes==1,"minified scenes changed");
    auto* source=original->mMeshes[0];auto* mesh=packedScene->mMeshes[0];
    require(source->mNumVertices==24 && mesh->mNumVertices==24 && source->mNumFaces==12 && mesh->mNumFaces==12,"minified cube topology changed");
    require(source->HasTextureCoords(0) && mesh->HasTextureCoords(0) && source->HasTangentsAndBitangents() && mesh->HasTangentsAndBitangents(),"minified attributes missing");
    std::vector<std::vector<unsigned>> charts;
    for(unsigned i=0;i<24;++i) {
        require((source->mVertices[i]-mesh->mVertices[i]).Length()<1e-7 && (source->mNormals[i]-mesh->mNormals[i]).Length()<1e-7 && (source->mTangents[i]-mesh->mTangents[i]).Length()<1e-6,"minified geometry or basis changed");
        size_t g=0;while(g<charts.size() && (source->mNormals[i]-source->mNormals[charts[g][0]]).Length()>1e-6)++g;
        if(g==charts.size())charts.push_back({});charts[g].push_back(i);
    }
    for(unsigned f=0;f<12;++f)for(unsigned k=0;k<3;++k)require(source->mFaces[f].mIndices[k]==mesh->mFaces[f].mIndices[k],"minified winding changed");
    require(charts.size()==6,"minified chart count changed");
    std::vector<unsigned char> coverage(64*64,0);size_t chartPixels=0,paddingPixels=0;
    for(const auto& chart:charts) {
        double low[2]={1e30,1e30},high[2]={-1e30,-1e30},oldLow[2]={1e30,1e30},oldHigh[2]={-1e30,-1e30};
        for(auto vertex:chart)for(unsigned k=0;k<2;++k) {
            double v=k?1-mesh->mTextureCoords[0][vertex].y:mesh->mTextureCoords[0][vertex].x;
            double old=k?1-source->mTextureCoords[0][vertex].y:source->mTextureCoords[0][vertex].x;
            low[k]=std::min(low[k],v);high[k]=std::max(high[k],v);oldLow[k]=std::min(oldLow[k],old);oldHigh[k]=std::max(oldHigh[k],old);
        }
        for(unsigned k=0;k<2;++k)require(std::abs((high[k]-low[k])*64/(oldHigh[k]-oldLow[k])-4)<1e-5,"minified chart density changed");
        int left=int(std::floor(low[0]*64)),right=int(std::floor(high[0]*64)),top=int(std::floor(low[1]*64)),bottom=int(std::floor(high[1]*64));
        require(left>=2 && top>=2 && right+2<64 && bottom+2<64,"minified padding outside image");
        for(int y=top-2;y<=bottom+2;++y)for(int x=left-2;x<=right+2;++x){size_t i=y*64+x;
            require(!coverage[i],"minified charts overlap");bool padding=x<left || x>right || y<top || y>bottom;
            coverage[i]=padding?2:1;++chartPixels;if(padding)++paddingPixels;
        }
    }
    auto* sm=original->mMaterials[source->mMaterialIndex];auto* pm=packedScene->mMaterials[mesh->mMaterialIndex];
    unsigned controls=0;size_t comparisons=0;
    for(auto type:{aiTextureType_BASE_COLOR,aiTextureType_EMISSIVE,aiTextureType_METALNESS,aiTextureType_LIGHTMAP,aiTextureType_NORMALS}) {
        auto old=atlasImage(original,sm,type);auto image=atlasImage(packedScene,pm,type);
        require(old.width==16 && old.height==1 && image.width==64 && image.height==64,"minified image dimensions changed");
        bool colour=type==aiTextureType_BASE_COLOR || type==aiTextureType_EMISSIVE;
        bool packed=type==aiTextureType_METALNESS,normal=type==aiTextureType_NORMALS;
        for(unsigned x=0;x<16;++x){unsigned char v=x%4==1?255:0;
            std::array<unsigned char,4> expected=normal?std::array<unsigned char,4>{128,128,255,255}:packed?std::array<unsigned char,4>{255,v,v,255}:std::array<unsigned char,4>{v,v,v,v};
            for(unsigned c=0;c<4;++c)require(old.pixels[4*x+c]==expected[c],"source minification pattern changed");
        }
        int mag=0,min=0;aiString path;aiTextureMapMode modes[3]={aiTextureMapMode_Wrap,aiTextureMapMode_Wrap,aiTextureMapMode_Wrap};
        pm->GetTexture(type,0,&path,nullptr,nullptr,nullptr,nullptr,modes);
        require(pm->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MAG(type,0),mag)==AI_SUCCESS && pm->Get(AI_MATKEY_GLTF_MAPPINGFILTER_MIN(type,0),min)==AI_SUCCESS && mag==9729 && min==9729 && modes[0]==aiTextureMapMode_Clamp && modes[1]==aiTextureMapMode_Clamp,"minified sampler changed");
        auto check=[&](){for(size_t i=0;i<coverage.size();++i){
            std::array<unsigned char,4> expected=coverage[i]?(colour?std::array<unsigned char,4>{137,137,137,64}:packed?std::array<unsigned char,4>{255,64,64,255}:normal?std::array<unsigned char,4>{128,128,255,255}:std::array<unsigned char,4>{64,64,64,64}):(packed?std::array<unsigned char,4>{255,0,0,255}:std::array<unsigned char,4>{0,0,0,0});
            for(unsigned c=0;c<4;++c)require(image.pixels[4*i+c]==expected[c],"minified atlas coverage/colour/channel pixel mismatch");
        }};
        check();comparisons+=coverage.size();
        auto reject=[&](size_t byte,unsigned char value){auto saved=image.pixels[byte];image.pixels[byte]=value;bool failed=false;try{check();}catch(const std::runtime_error&){failed=true;}image.pixels[byte]=saved;require(failed,"minification control escaped");++controls;};
        size_t interior=std::find(coverage.begin(),coverage.end(),1)-coverage.begin();
        size_t gutter=std::find(coverage.begin(),coverage.end(),2)-coverage.begin();
        if(type==aiTextureType_BASE_COLOR){reject(4*interior,64);reject(4*interior+3,137);reject(4*gutter,0);}
        if(packed)reject(4*interior+1,137);
        check();
    }
    require(chartPixels>1000 && paddingPixels>300 && controls==4,"minification coverage incomplete");
    std::cout<<"Minified atlas import passed: six charts, five PNGs, "<<comparisons<<" exact RGBA pixels; "<<chartPixels<<" covered pixels and "<<paddingPixels<<" padding pixels per map; four colour/alpha/packing/padding controls rejected\n";
}
