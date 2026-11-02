// Fixture-specific independent UNORM16 import check for plate-flex.gasset.
// Assimp supplies decoded attributes; double precision arithmetic isolates the
// weight change from float accumulation noise. This is not engine acceptance.
static void verifyQuantizedWeights(const char* referencePath, const char* path,
                                  double maxMm, double maxDegrees, bool rawDecode=false) {
    require(std::isfinite(maxMm) && maxMm>=0 && std::isfinite(maxDegrees) &&
        maxDegrees>=0 && maxDegrees<=180,"invalid quantisation probe limits");
    std::ifstream input(path,std::ios::binary);
    std::vector<unsigned char> bytes((std::istreambuf_iterator<char>(input)),{});
    require(bytes.size()>=28,"quantised GLB missing");
    auto word=[&](size_t i){return uint32_t(bytes[i])|(uint32_t(bytes[i+1])<<8)|
        (uint32_t(bytes[i+2])<<16)|(uint32_t(bytes[i+3])<<24);};
    require(word(0)==0x46546c67 && word(4)==2 && word(8)==bytes.size() &&
        word(16)==0x4e4f534a && word(12)<=bytes.size()-28,"invalid quantised GLB header");
    const size_t binary=28+word(12);
    require(word(binary-4)==0x004e4942 && word(binary-8)==bytes.size()-binary,"invalid binary chunk");
    rapidjson::Document doc; doc.Parse(reinterpret_cast<const char*>(bytes.data()+20),word(12));
    require(!doc.HasParseError() && doc.HasMember("meshes") && doc.HasMember("accessors") &&
        doc.HasMember("bufferViews"),"invalid quantised document");
    require(doc.HasMember("skins") && doc["skins"].Size()==1 && doc["skins"][0]["joints"].Size()==2,
        "expected two-joint skin");
    const auto& palette=doc["skins"][0]["joints"];
    require(std::string(doc["nodes"][palette[0].GetUint()]["name"].GetString())=="base" &&
        std::string(doc["nodes"][palette[1].GetUint()]["name"].GetString())=="tip","unexpected raw joint order");
    size_t rawVertices=0;
    std::vector<std::array<double,2>> rawWeights;
    for(const auto& mesh:doc["meshes"].GetArray()) for(const auto& primitive:mesh["primitives"].GetArray()) {
        const auto& attributes=primitive["attributes"];
        if(!attributes.HasMember("WEIGHTS_0"))continue;
        const auto& accessor=doc["accessors"][attributes["WEIGHTS_0"].GetUint()];
        require(accessor["componentType"].GetUint()==5123 && accessor.HasMember("normalized") &&
            accessor["normalized"].GetBool() && std::string(accessor["type"].GetString())=="VEC4",
            "expected actual normalized unsigned-short weights");
        const auto& view=doc["bufferViews"][accessor["bufferView"].GetUint()];
        require(!view.HasMember("byteStride") && !accessor.HasMember("sparse"),"unexpected weight layout");
        size_t offset=view.HasMember("byteOffset")?view["byteOffset"].GetUint():0;
        size_t count=accessor["count"].GetUint();
        require(!accessor.HasMember("byteOffset") || accessor["byteOffset"].GetUint()==0,"unexpected accessor offset");
        require(offset<=bytes.size()-binary && count<=(bytes.size()-binary-offset)/8 &&
            view["byteLength"].GetUint64()==count*8,"invalid weight byte range");
        require(rawWeights.empty(),"expected one skinned primitive");
        const auto& joints=doc["accessors"][attributes["JOINTS_0"].GetUint()];
        require(joints["componentType"].GetUint()==5123 && joints["count"].GetUint()==count &&
            std::string(joints["type"].GetString())=="VEC4","unexpected joint encoding");
        const auto& jointView=doc["bufferViews"][joints["bufferView"].GetUint()];
        size_t jointOffset=jointView.HasMember("byteOffset")?jointView["byteOffset"].GetUint():0;
        require(!jointView.HasMember("byteStride") && !joints.HasMember("sparse") &&
            (!joints.HasMember("byteOffset") || joints["byteOffset"].GetUint()==0) &&
            jointOffset<=bytes.size()-binary && count<=(bytes.size()-binary-jointOffset)/8 &&
            jointView["byteLength"].GetUint64()==count*8,"invalid joint byte range");
        rawWeights.resize(count);
        for(size_t i=0;i<count;++i) {
            unsigned sum=0;
            for(unsigned k=0;k<4;++k) {
                auto at=binary+offset+i*8+k*2;unsigned weight=bytes[at]+256u*bytes[at+1];sum+=weight;
                auto jt=binary+jointOffset+i*8+k*2;unsigned joint=bytes[jt]+256u*bytes[jt+1];
                require(joint<2,"unexpected joint palette index");
                if(weight) {require(rawWeights[i][joint]==0,"duplicate positive influence");rawWeights[i][joint]=weight/65535.0;}
            }
            require(sum==65535,"integer weight sum is not 65535");
        }
        rawVertices+=count;
    }
    require(rawVertices>100,"quantised fixture has no skin weights");
    Assimp::Importer importers[2];
    const aiScene* scenes[2]={importers[0].ReadFile(referencePath,0),importers[1].ReadFile(path,0)};
    const aiMesh* meshes[2]; const aiNode* tips[2]; const aiNode* bases[2];
    std::vector<double> weights[2][2]; aiMatrix4x4 offsets[2][2];
    const aiNodeAnim* tracks[2];
    for(unsigned s=0;s<2;++s) {
        const auto* scene=scenes[s]; require(scene && scene->mNumAnimations==1,"expected one flex clip");
        auto* plate=scene->mRootNode->FindNode("plate");
        bases[s]=scene->mRootNode->FindNode("base");tips[s]=scene->mRootNode->FindNode("tip");
        require(plate && bases[s] && tips[s] && tips[s]->mParent==bases[s] && plate->mNumMeshes==1,"flex hierarchy changed");
        auto* mesh=scene->mMeshes[plate->mMeshes[0]]; meshes[s]=mesh;
        require(mesh->mNumBones==2 && mesh->HasNormals() && mesh->HasTangentsAndBitangents() && mesh->HasTextureCoords(0),"flex attributes missing");
        for(auto& w:weights[s])w.resize(mesh->mNumVertices);
        bool seen[2]={};
        for(unsigned b=0;b<2;++b) {
            auto* bone=mesh->mBones[b];std::string name=bone->mName.C_Str();
            require(name=="base" || name=="tip","unexpected flex bone");unsigned j=name=="base"?0:1;
            require(!seen[j],"duplicate flex bone");seen[j]=true;offsets[s][j]=bone->mOffsetMatrix;
            if(s==1 && rawDecode) {
                require(mesh->mNumVertices==rawWeights.size(),"raw/imported vertex counts differ");
                for(unsigned i=0;i<mesh->mNumVertices;++i)weights[s][j][i]=rawWeights[i][j];
                continue;
            }
            for(unsigned k=0;k<bone->mNumWeights;++k) {
                auto w=bone->mWeights[k];
                if(!(w.mVertexId<mesh->mNumVertices && w.mWeight>0 && weights[s][j][w.mVertexId]==0))
                    std::cerr<<"Invalid influence: scene "<<s<<", bone "<<j<<", vertex "<<w.mVertexId<<", weight "<<w.mWeight<<"\n";
                require(w.mVertexId<mesh->mNumVertices && w.mWeight>0 &&
                    weights[s][j][w.mVertexId]==0,"invalid imported influence");weights[s][j][w.mVertexId]=w.mWeight;
            }
        }
        auto* clip=scene->mAnimations[0];
        require(std::string(clip->mName.C_Str())=="bend" && clip->mNumChannels==1 &&
            clip->mTicksPerSecond>0 && std::abs(clip->mDuration/clip->mTicksPerSecond-1)<1e-6,"flex animation changed");
        tracks[s]=clip->mChannels[0];
        require(std::string(tracks[s]->mNodeName.C_Str())=="tip" && tracks[s]->mNumRotationKeys==2,"flex rotation keys changed");
    }
    auto* a=meshes[0];auto* b=meshes[1];
    require(a->mNumVertices==b->mNumVertices && b->mNumVertices==rawVertices && a->mNumFaces==b->mNumFaces,"quantisation changed topology");
    require(bases[0]->mTransformation==bases[1]->mTransformation && tips[0]->mTransformation==tips[1]->mTransformation &&
        offsets[0][0]==offsets[1][0] && offsets[0][1]==offsets[1][1],"quantisation changed rig transforms");
    for(unsigned k=0;k<2;++k) require(tracks[0]->mRotationKeys[k]==tracks[1]->mRotationKeys[k],"quantisation changed clip keys");
    require(tracks[0]->mNumPositionKeys==tracks[1]->mNumPositionKeys &&
        tracks[0]->mNumScalingKeys==tracks[1]->mNumScalingKeys,"quantisation changed TRS channels");
    for(unsigned k=0;k<tracks[0]->mNumPositionKeys;++k)
        require(tracks[0]->mPositionKeys[k]==tracks[1]->mPositionKeys[k],"quantisation changed translation keys");
    for(unsigned k=0;k<tracks[0]->mNumScalingKeys;++k)
        require(tracks[0]->mScalingKeys[k]==tracks[1]->mScalingKeys[k],"quantisation changed scale keys");
    bool changed=false;
    for(unsigned i=0;i<a->mNumVertices;++i) {
        require(a->mVertices[i]==b->mVertices[i] && a->mNormals[i]==b->mNormals[i] && a->mTangents[i]==b->mTangents[i] &&
            a->mBitangents[i]==b->mBitangents[i] && a->mTextureCoords[0][i]==b->mTextureCoords[0][i],"quantisation changed vertex attributes");
        for(unsigned j=0;j<2;++j) {
            double original=weights[0][j][i],quantized=weights[1][j][i];
            require(std::abs(original-quantized)<=1.0/65535+1e-7,"imported weight exceeds quantisation step");
            require(original!=0 || quantized==0,"zero influence became nonzero");
            require(std::abs(quantized*65535-std::round(quantized*65535))<.003,"imported weight not on UNORM16 lattice");
            changed|=original!=quantized;
        }
        require(std::abs(weights[1][0][i]+weights[1][1][i]-1)<1e-7,"decoded weight sum changed");
    }
    require(changed,"control did not exercise changed weights");
    for(unsigned f=0;f<a->mNumFaces;++f) {
        require(a->mFaces[f].mNumIndices==3 && b->mFaces[f].mNumIndices==3,"expected triangles");
        for(unsigned k=0;k<3;++k)require(a->mFaces[f].mIndices[k]==b->mFaces[f].mIndices[k],"triangle order changed");
    }
    using M=aiMatrix4x4t<double>;using V=aiVector3t<double>;using N=aiMatrix3x3t<double>;
    auto convert=[](const aiMatrix4x4& src){M dst;for(unsigned r=0;r<4;++r)for(unsigned c=0;c<4;++c)dst[r][c]=src[r][c];return dst;};
    auto vec=[](const aiVector3D& p){return V(p.x,p.y,p.z);};
    double maximumMm=0,maximumDegrees=0;size_t comparisons=0;
    for(unsigned sample=0;sample<=64;++sample) {
        double time=sample/64.0;M matrices[2][2];
        for(unsigned s=0;s<2;++s) {
            aiVector3D scale,translation;aiQuaternion rotation;
            tips[s]->mTransformation.Decompose(scale,rotation,translation);
            aiQuaternion::Interpolate(rotation,tracks[s]->mRotationKeys[0].mValue,tracks[s]->mRotationKeys[1].mValue,float(time));rotation.Normalize();
            matrices[s][0]=convert(worldMatrix(bases[s])*offsets[s][0]);
            matrices[s][1]=convert(worldMatrix(bases[s])*aiMatrix4x4(scale,rotation,translation)*offsets[s][1]);
        }
        for(unsigned i=0;i<a->mNumVertices;++i) {
            V positions[2],normals[2],tangents[2];double signs[2];
            for(unsigned s=0;s<2;++s) {
                M blend;for(unsigned r=0;r<4;++r)for(unsigned c=0;c<4;++c)
                    blend[r][c]=matrices[s][0][r][c]*weights[s][0][i]+matrices[s][1][r][c]*weights[s][1][i];
                N linear(blend),normal(linear);require(std::abs(linear.Determinant())>1e-12,"singular sampled skin");
                normal.Inverse().Transpose();positions[s]=blend*vec(a->mVertices[i]);
                normals[s]=(normal*vec(a->mNormals[i])).Normalize();
                V tangent=linear*vec(a->mTangents[i]);tangents[s]=(tangent-normals[s]*(normals[s]*tangent)).Normalize();
                signs[s]=linear.Determinant();
            }
            require(signs[0]*signs[1]>0,"tangent handedness changed");
            double mm=(positions[0]-positions[1]).Length()*1000;
            auto angle=[](const V& x,const V& y){return std::atan2((x^y).Length(),x*y)*180/3.14159265358979323846;};
            double degrees=std::max(angle(normals[0],normals[1]),angle(tangents[0],tangents[1]));
            require(std::isfinite(mm) && std::isfinite(degrees),"nonfinite quantisation comparison");
            maximumMm=std::max(maximumMm,mm);maximumDegrees=std::max(maximumDegrees,degrees);++comparisons;
        }
    }
    std::cout<<(rawDecode?"Raw-decoded quantised weights (NOT Assimp weight import support): ":"Quantised flex import: ")
        <<rawVertices<<" vertices, "<<comparisons<<" vertex/pose comparisons; maximum position "
        <<maximumMm<<" mm, normal/tangent "<<maximumDegrees<<" degrees; limits "<<maxMm<<" mm, "<<maxDegrees<<" degrees\n";
    require(maximumMm<=maxMm && maximumDegrees<=maxDegrees,"imported quantisation exceeds sampled limits");
}
