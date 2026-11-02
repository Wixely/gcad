// Value checks after dense embedded ranges and usage contracts have passed.
static void verifySourceValues(const rapidjson::Document& doc,const char* path,const aiScene* imported=nullptr) {
    if(!doc.HasMember("accessors"))return;
    std::ifstream file(std::filesystem::u8path(path),std::ios::binary|std::ios::ate);auto end=file.tellg();
    require(bool(file) && end>=28 && end<=std::streamoff(64*1024*1024),"source value file size invalid");
    std::vector<unsigned char> data(size_t(end),0);file.seekg(0);file.read(reinterpret_cast<char*>(data.data()),end);require(bool(file),"source value read failed");
    auto word=[&](size_t offset){require(offset<=data.size() && data.size()-offset>=4,"source value header truncated");return uint32_t(data[offset])|(uint32_t(data[offset+1])<<8)|(uint32_t(data[offset+2])<<16)|(uint32_t(data[offset+3])<<24);};
    size_t bin=20+uint64_t(word(12));require(bin<=data.size() && data.size()-bin>=8 && word(bin+4)==0x004e4942,"source value BIN missing");bin+=8;
    struct Layout {uint64_t start,stride,count,width,rows,columns,column;unsigned component;bool normalized;};
    std::vector<Layout> layouts;
    for(const auto& a:doc["accessors"].GetArray()) {
        const auto& view=doc["bufferViews"][a["bufferView"].GetUint()];unsigned component=a["componentType"].GetUint();
        uint64_t width=(component==5120 || component==5121)?1:(component==5122 || component==5123)?2:4;
        std::string type(a["type"].GetString());uint64_t rows=1,cols=1;
        if(type=="VEC2")rows=2;if(type=="VEC3")rows=3;if(type=="VEC4")rows=4;
        if(type=="MAT2")rows=cols=2;if(type=="MAT3")rows=cols=3;if(type=="MAT4")rows=cols=4;
        uint64_t column=rows*width;if(cols>1)column=(column+3)&~uint64_t(3);
        layouts.push_back({bin+sourceUnsigned(view,"byteOffset",0)+sourceUnsigned(a,"byteOffset",0),
            sourceUnsigned(view,"byteStride",column*cols),a["count"].GetUint64(),width,rows,cols,column,component,
            a.HasMember("normalized") && a["normalized"].IsBool() && a["normalized"].GetBool()});
    }
    auto read=[&](unsigned id,uint64_t index,unsigned scalar) {
        require(id<layouts.size(),"source value accessor invalid");const auto& l=layouts[id];
        require(index<l.count && scalar<l.rows*l.columns,"source value element invalid");
        uint64_t at=l.start+index*l.stride+(scalar/l.rows)*l.column+(scalar%l.rows)*l.width;
        require(at<=data.size() && l.width<=data.size()-at,"source value outside file");
        uint32_t raw=0;for(unsigned i=0;i<l.width;++i)raw|=uint32_t(data[size_t(at)+i])<<(8*i);
        double value=raw;
        if(l.component==5126){float f;std::memcpy(&f,&raw,4);value=f;}
        else if(l.component==5120)value=raw>=128?double(raw)-256:raw;
        else if(l.component==5122)value=raw>=32768?double(raw)-65536:raw;
        if(l.normalized && l.component==5121)value/=255;
        if(l.normalized && l.component==5123)value/=65535;
        return value;
    };
    for(unsigned id=0;id<layouts.size();++id)if(layouts[id].component==5126)
        for(uint64_t i=0;i<layouts[id].count;++i)for(unsigned c=0;c<layouts[id].rows*layouts[id].columns;++c)
            require(std::isfinite(read(id,i,c)),"nonfinite source accessor value");
    if(doc.HasMember("meshes"))for(const auto& mesh:doc["meshes"].GetArray())for(const auto& p:mesh["primitives"].GetArray())if(p.HasMember("indices")) {
        auto id=p["indices"].GetUint();uint64_t vertices=layouts[p["attributes"]["POSITION"].GetUint()].count;
        double restart=layouts[id].component==5121?255:layouts[id].component==5123?65535:4294967295.0;
        for(uint64_t i=0;i<layouts[id].count;++i){double index=read(id,i,0);require(index<vertices && index!=restart,"source index value outside primitive");}
    }
    if(doc.HasMember("nodes"))for(const auto& node:doc["nodes"].GetArray())if(node.HasMember("skin")) {
        size_t jointCount=doc["skins"][node["skin"].GetUint()]["joints"].Size();
        for(const auto& p:doc["meshes"][node["mesh"].GetUint()]["primitives"].GetArray()) {
            const auto& attributes=p["attributes"];std::vector<std::pair<unsigned,unsigned>> sets;
            for(auto it=attributes.MemberBegin();it!=attributes.MemberEnd();++it) {
                std::string name(it->name.GetString());if(name.rfind("JOINTS_",0)!=0)continue;
                sets.emplace_back(it->value.GetUint(),attributes[("WEIGHTS_"+name.substr(7)).c_str()].GetUint());
            }
            uint64_t count=layouts[attributes["POSITION"].GetUint()].count;
            for(uint64_t i=0;i<count;++i) {
                double sum=0;
                for(const auto& set:sets)for(unsigned c=0;c<4;++c) {
                    require(read(set.first,i,c)<jointCount,"source joint value outside skin palette");
                    double weight=read(set.second,i,c);require(weight>=0 && weight<=1,"source weight value outside unit interval");sum+=weight;
                }
                require(std::isfinite(sum) && std::abs(sum-1)<=1e-4,"source vertex weights not normalized");
            }
        }
    }
    if(doc.HasMember("animations"))for(const auto& clip:doc["animations"].GetArray())for(const auto& sampler:clip["samplers"].GetArray()) {
        auto id=sampler["input"].GetUint();double previous=-1;
        for(uint64_t i=0;i<layouts[id].count;++i){double time=read(id,i,0);require(time>=0 && time>previous,"source animation times not increasing");previous=time;}
    }
    if(imported && doc.HasMember("nodes")) {
        size_t compared=0;
        for(const auto& rawNode:doc["nodes"].GetArray())if(rawNode.HasMember("skin")) {
            auto* node=imported->mRootNode->FindNode(aiString(rawNode["name"].GetString()));
            const auto& primitives=doc["meshes"][rawNode["mesh"].GetUint()]["primitives"];
            const auto& joints=doc["skins"][rawNode["skin"].GetUint()]["joints"];
            require(node && node->mNumMeshes==primitives.Size(),"imported source primitive association changed");
            for(unsigned p=0;p<primitives.Size();++p) {
                const auto& primitive=primitives[p];const auto& attributes=primitive["attributes"];
                auto* mesh=imported->mMeshes[node->mMeshes[p]];auto weights=sourceSceneWeights(mesh);
                uint64_t cornerCount=primitive.HasMember("indices")?layouts[primitive["indices"].GetUint()].count:layouts[attributes["POSITION"].GetUint()].count;
                require(uint64_t(mesh->mNumFaces)*3==cornerCount,"imported source face count changed");
                for(unsigned f=0;f<mesh->mNumFaces;++f)for(unsigned c=0;c<3;++c) {
                    require(mesh->mFaces[f].mNumIndices==3,"imported source topology changed");
                    uint64_t corner=uint64_t(f)*3+c;
                    uint64_t rawVertex=primitive.HasMember("indices")?uint64_t(read(primitive["indices"].GetUint(),corner,0)):corner;
                    auto vertex=mesh->mFaces[f].mIndices[c];require(vertex<mesh->mNumVertices,"imported source vertex invalid");
                    const auto& position=mesh->mVertices[vertex];
                    require(position.x==read(attributes["POSITION"].GetUint(),rawVertex,0) && position.y==read(attributes["POSITION"].GetUint(),rawVertex,1) && position.z==read(attributes["POSITION"].GetUint(),rawVertex,2),"imported corner remapping differs from source");
                    std::map<std::string,double> expected;
                    for(auto it=attributes.MemberBegin();it!=attributes.MemberEnd();++it) {
                        std::string name(it->name.GetString());if(name.rfind("JOINTS_",0)!=0)continue;
                        auto weightId=attributes[("WEIGHTS_"+name.substr(7)).c_str()].GetUint();
                        for(unsigned k=0;k<4;++k) {
                            double weight=read(weightId,rawVertex,k);if(weight==0)continue;
                            auto joint=unsigned(read(it->value.GetUint(),rawVertex,k));
                            expected[doc["nodes"][joints[joint].GetUint()]["name"].GetString()]+=weight;
                        }
                    }
                    require(weights[vertex].size()==expected.size(),"imported influence set differs from source");
                    for(const auto& entry:expected) {
                        auto actual=weights[vertex].find(entry.first);
                        require(actual!=weights[vertex].end() && std::abs(actual->second.second-entry.second)<=1e-6,"imported weight differs from source");
                    }
                    ++compared;
                }
            }
        }
        std::cout<<"Raw source weight comparison: "<<compared<<" remapped corners\n";
    }
}
