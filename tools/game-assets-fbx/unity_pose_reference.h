// Source-derived positions, independent of the FBX writer and Unity importer.
static void writeUnityPoseReference(const char* input,const std::filesystem::path& path) {
    require(!std::filesystem::exists(path),"preserving existing pose reference");
    auto imported=importScene(input,true,true);
    auto prepared=prepareScene(imported);const auto* source=prepared.scene.get();
    rapidjson::StringBuffer json;rapidjson::Writer<rapidjson::StringBuffer> writer(json);
    writer.StartObject();writer.Key("schema");writer.Uint(1);
    writer.Key("coordinates");writer.String("right-handed-y-up-metres");
    writer.Key("samples");writer.StartArray();
    for(unsigned c=0;c<source->mNumAnimations;++c) {
        const auto* clip=source->mAnimations[c];
        require(clip->mTicksPerSecond>0 && clip->mDuration>0,"invalid reference clip timing");
        std::map<std::string,const aiNodeAnim*> tracks;
        for(unsigned t=0;t<clip->mNumChannels;++t)tracks.emplace(clip->mChannels[t]->mNodeName.C_Str(),clip->mChannels[t]);
        for(double fraction:{0.0,.125,.333,.5,.731,.875,1.0}) {
            const double ticks=clip->mDuration*fraction;
            std::map<std::string,aiMatrix4x4> worlds;
            std::function<void(const aiNode*,const aiMatrix4x4&)> visit=[&](const aiNode* node,const aiMatrix4x4& parent) {
                auto local=node->mTransformation;auto found=tracks.find(node->mName.C_Str());
                if(found!=tracks.end()) {
                    const auto* channel=found->second;aiVector3D scale,translation;aiQuaternion rotation;
                    local.Decompose(scale,rotation,translation);
                    if(channel->mNumPositionKeys)translation=sampleSceneVector(channel->mPositionKeys,channel->mNumPositionKeys,ticks);
                    if(channel->mNumScalingKeys)scale=sampleSceneVector(channel->mScalingKeys,channel->mNumScalingKeys,ticks);
                    if(channel->mNumRotationKeys)rotation=sampleSceneRotation(channel->mRotationKeys,channel->mNumRotationKeys,ticks);
                    local=aiMatrix4x4(scale,rotation,translation);
                }
                const auto world=parent*local;worlds.emplace(node->mName.C_Str(),world);
                for(unsigned n=0;n<node->mNumChildren;++n)visit(node->mChildren[n],world);
            };
            visit(source->mRootNode,aiMatrix4x4());
            writer.StartObject();writer.Key("clip");writer.String(clip->mName.C_Str());
            writer.Key("time");writer.Double(ticks/clip->mTicksPerSecond);
            writer.Key("points");writer.StartArray();
            std::function<void(const aiNode*)> positions=[&](const aiNode* node) {
                for(unsigned p=0;p<node->mNumMeshes;++p) {
                    const auto* mesh=source->mMeshes[node->mMeshes[p]];const auto weights=sourceSceneWeights(mesh);
                    for(unsigned v=0;v<mesh->mNumVertices;++v) {
                        aiVector3D point;
                        if(weights[v].empty())point=worlds.at(node->mName.C_Str())*mesh->mVertices[v];
                        else for(const auto& influence:weights[v])
                            point+=(worlds.at(influence.first)*influence.second.first->mOffsetMatrix*mesh->mVertices[v])*ai_real(influence.second.second);
                        require(std::isfinite(point.x)&&std::isfinite(point.y)&&std::isfinite(point.z),"nonfinite source pose");
                        writer.StartObject();writer.Key("x");writer.Double(point.x);
                        writer.Key("y");writer.Double(point.y);writer.Key("z");writer.Double(point.z);writer.EndObject();
                    }
                }
                for(unsigned n=0;n<node->mNumChildren;++n)positions(node->mChildren[n]);
            };
            positions(source->mRootNode);writer.EndArray();writer.EndObject();
        }
    }
    writer.EndArray();writer.EndObject();
    std::ofstream output(path,std::ios::binary);output.write(json.GetString(),std::streamsize(json.GetSize()));
    require(bool(output),"cannot write pose reference");
}
