// Independent interpolation of imported STEP/LINEAR glTF channels against ufbx.
#include "scene_basis_probe.h"
static void verifyImportedInterpolation(const rapidjson::Document& doc,const aiScene* scene) {
    if(!doc.HasMember("animations")){require(scene->mNumAnimations==0,"unexpected imported animations");return;}
    for(const auto& clip:doc["animations"].GetArray()) {
        require(clip.HasMember("name") && clip["name"].IsString() && clip.HasMember("channels") && clip["channels"].IsArray(),"animation source identity missing");
        const aiAnimation* imported=nullptr;
        for(unsigned i=0;i<scene->mNumAnimations;++i)if(scene->mAnimations[i]->mName==aiString(clip["name"].GetString())) {
            require(!imported,"duplicate imported clip");imported=scene->mAnimations[i];
        }
        require(imported,"source clip not imported");
        for(const auto& channel:clip["channels"].GetArray()) {
            require(channel.IsObject() && channel.HasMember("sampler") && channel["sampler"].IsUint() && channel["sampler"].GetUint()<clip["samplers"].Size() && channel.HasMember("target") && channel["target"].IsObject(),"invalid animation source channel");
            const auto& target=channel["target"];
            require(target.HasMember("node") && target["node"].IsUint() && target.HasMember("path") && target["path"].IsString(),"invalid animation source target");
            auto name="gcad_audit_node_"+std::to_string(target["node"].GetUint());const aiNodeAnim* track=nullptr;
            for(unsigned i=0;i<imported->mNumChannels;++i)if(imported->mChannels[i]->mNodeName==aiString(name))track=imported->mChannels[i];
            require(track,"animation target not imported");
            const auto& sampler=clip["samplers"][channel["sampler"].GetUint()];
            const bool step=sampler.HasMember("interpolation") && std::string(sampler["interpolation"].GetString())=="STEP";
            auto check=[&](const auto* keys,unsigned count) {
                require(count>0,"source animation keys missing");
                for(unsigned i=0;i<count;++i)require(keys[i].mInterpolation==(step?aiAnimInterpolation_Step:aiAnimInterpolation_Linear),"imported interpolation differs from GLB");
            };
            const std::string path=target["path"].GetString();
            if(path=="translation")check(track->mPositionKeys,track->mNumPositionKeys);
            else if(path=="rotation")check(track->mRotationKeys,track->mNumRotationKeys);
            else if(path=="scale")check(track->mScalingKeys,track->mNumScalingKeys);
            else require(false,"unsupported animation target path");
        }
    }
}
static aiVector3D sampleSceneVector(const aiVectorKey* keys,unsigned count,double ticks) {
    require(count>0,"animation vector keys missing");
    if(ticks<=keys[0].mTime)return keys[0].mValue;
    for(unsigned i=1;i<count;++i)if(ticks<=keys[i].mTime) {
        require(keys[i].mTime>keys[i-1].mTime,"animation key times unordered");
        if(keys[i-1].mInterpolation==aiAnimInterpolation_Step)return ticks<keys[i].mTime?keys[i-1].mValue:keys[i].mValue;
        auto t=ai_real((ticks-keys[i-1].mTime)/(keys[i].mTime-keys[i-1].mTime));
        return keys[i-1].mValue*(1-t)+keys[i].mValue*t;
    }
    return keys[count-1].mValue;
}
static aiQuaternion sampleSceneRotation(const aiQuatKey* keys,unsigned count,double ticks) {
    require(count>0,"animation rotation keys missing");
    if(ticks<=keys[0].mTime)return keys[0].mValue;
    for(unsigned i=1;i<count;++i)if(ticks<=keys[i].mTime) {
        require(keys[i].mTime>keys[i-1].mTime,"animation key times unordered");
        if(keys[i-1].mInterpolation==aiAnimInterpolation_Step)return ticks<keys[i].mTime?keys[i-1].mValue:keys[i].mValue;
        aiQuaternion result;aiQuaternion::Interpolate(result,keys[i-1].mValue,keys[i].mValue,
            ai_real((ticks-keys[i-1].mTime)/(keys[i].mTime-keys[i-1].mTime)));return result.Normalize();
    }
    return keys[count-1].mValue;
}
static size_t verifySceneAnimation(const aiScene* source,const ufbx_scene* converted,bool evaluateSkin=true,bool physical=false) {
    require(converted->anim_stacks.count==source->mNumAnimations,"animation clip count changed");
    size_t comparisons=0,corners=0,normalSamples=0,normalDifferences=0,reflectedNormals=0,positiveNormalDifferences=0;
    double maximum=0,maxSkin=0,maxNormalAngle=0,maxPositiveNormalAngle=0;
    size_t zeroTimePositiveNormals=0,zeroTimePositiveDifferences=0;double maxZeroTimePositiveAngle=0;
    size_t basisSamples=0;double maxBasisError=0;bool basisControls=false;
    const double unit=physical?converted->settings.unit_meters:1;
    require(std::isfinite(unit) && unit>0,"invalid animation length unit");
    for(unsigned c=0;c<source->mNumAnimations;++c) {
        auto* clip=source->mAnimations[c];require(clip->mTicksPerSecond>0 && clip->mDuration>0,"invalid source clip timing");
        const ufbx_anim_stack* stack=nullptr;
        for(auto* candidate:converted->anim_stacks)if(std::string(candidate->name.data,candidate->name.length)==clip->mName.C_Str()) {
            require(!stack,"duplicate clip name");stack=candidate;
        }
        const double duration=clip->mDuration/clip->mTicksPerSecond;
        require(stack && std::abs(stack->time_begin)<1e-6 && std::abs(stack->time_end-duration)<1e-6,"animation name or timing changed");
        std::map<std::string,const aiNodeAnim*> tracks;
        for(unsigned t=0;t<clip->mNumChannels;++t)require(tracks.emplace(clip->mChannels[t]->mNodeName.C_Str(),clip->mChannels[t]).second,"duplicate node animation channel");
        std::set<double> times;
        for(unsigned sample=0;sample<=32;++sample)times.insert(duration*sample/32);
        // Off-grid times avoid merely rechecking a power-of-two writer bake grid.
        for(unsigned sample=1;sample<=37;++sample)times.insert(duration*std::fmod(sample*.6180339887498949,1.0));
        std::set<double> keys;
        for(const auto& item:tracks) {
            auto* track=item.second;
            auto collect=[&](auto* values,unsigned count){for(unsigned i=0;i<count;++i)keys.insert(values[i].mTime/clip->mTicksPerSecond);};
            collect(track->mPositionKeys,track->mNumPositionKeys);collect(track->mRotationKeys,track->mNumRotationKeys);collect(track->mScalingKeys,track->mNumScalingKeys);
        }
        double previous=0;
        for(double key:keys) {
            require(std::isfinite(key) && key>=0 && key<=duration+1e-6,"key outside clip interval");
            times.insert(key);
            const double epsilon=std::max(1e-7,duration*1e-7);
            if(key>epsilon)times.insert(key-epsilon);
            if(key+epsilon<duration)times.insert(key+epsilon);
            if(key>previous)times.insert((key+previous)/2);
            previous=key;
        }
        if(!evaluateSkin) {
            // Independently inspect baked FBX knot intervals as well. These may
            // cluster near a source discontinuity missed by a uniform time grid.
            auto exportedTime=[&](double time) {
                // FBX quantizes key times to 1/46186158000 seconds. Do not
                // compare opposite sides of STEP inside that rounding window.
                // Exact source times and +/- epsilon remain separate samples.
                for(double key:keys)if(std::abs(time-key)<=2.0/46186158000.0){time=key;break;}
                if(time>=0 && time<=duration)times.insert(time);
            };
            for(auto* curve:converted->anim_curves) {
                for(size_t k=0;k<curve->keyframes.count;++k) {
                    const double time=curve->keyframes.data[k].time;
                    exportedTime(time);
                    if(k) {
                        const double mid=(curve->keyframes.data[k-1].time+time)*.5;
                        exportedTime(mid);
                    }
                }
            }
        }
        unsigned sample=0;
        for(double seconds:times) {
            ++sample;double ticks=seconds*clip->mTicksPerSecond;
            ufbx_evaluate_opts opts{};opts.evaluate_skinning=evaluateSkin;ufbx_error error{};
            Scene posed(ufbx_evaluate_scene(converted,stack->anim,seconds,&opts,&error),ufbx_free_scene);
            require(bool(posed),"animation scene evaluation failed");
            std::map<std::string,aiMatrix4x4> worlds;
            std::function<void(const aiNode*,const aiMatrix4x4&)> visit=[&](const aiNode* node,const aiMatrix4x4& parent) {
                auto local=node->mTransformation;auto track=tracks.find(node->mName.C_Str());
                if(track!=tracks.end()) {
                    auto* channel=track->second;aiVector3D scale,translation;aiQuaternion rotation;
                    local.Decompose(scale,rotation,translation);
                    if(channel->mNumPositionKeys)translation=sampleSceneVector(channel->mPositionKeys,channel->mNumPositionKeys,ticks);
                    if(channel->mNumScalingKeys)scale=sampleSceneVector(channel->mScalingKeys,channel->mNumScalingKeys,ticks);
                    if(channel->mNumRotationKeys)rotation=sampleSceneRotation(channel->mRotationKeys,channel->mNumRotationKeys,ticks);
                    local=aiMatrix4x4(scale,rotation,translation);
                }
                auto world=parent*local;
                require(worlds.emplace(node->mName.C_Str(),world).second,"duplicate source node name");
                // The importer/exporter may replace only the synthetic root node.
                if(node!=source->mRootNode || node->mNumMeshes || track!=tracks.end()) {
                    auto* actual=ufbx_find_node(posed.get(),node->mName.C_Str());require(actual,"animated node missing");
                    const double expected[]={world.a1,world.b1,world.c1,world.a2,world.b2,world.c2,world.a3,world.b3,world.c3,world.a4,world.b4,world.c4};
                    for(unsigned k=0;k<12;++k) {
                        double delta=std::abs(actual->node_to_world.v[k]*(k>=9?unit:1)-expected[k]);
                        require(std::isfinite(delta),"nonfinite animated transform");maximum=std::max(maximum,delta);
                        if(delta>1e-5)std::cout<<"Transform mismatch: clip "<<clip->mName.C_Str()<<", sample "<<sample<<", node "<<node->mName.C_Str()<<", component "<<k<<", expected "<<expected[k]<<", actual "<<actual->node_to_world.v[k]*(k>=9?unit:1)<<"\n";
                        require(delta<=1e-5,"sampled authored node transform changed");
                    }
                    ++comparisons;
                }
                for(unsigned i=0;i<node->mNumChildren;++i)visit(node->mChildren[i],world);
            };
            visit(source->mRootNode,aiMatrix4x4());
            std::function<void(const aiNode*)> skinVisit=[&](const aiNode* node) {
                if(node->mNumMeshes) {
                    auto* output=ufbx_find_node(posed.get(),node->mName.C_Str());require(output && output->mesh,"posed mesh node missing");
                    auto* mesh=output->mesh;size_t face=0;
                    for(unsigned p=0;p<node->mNumMeshes;++p) {
                        auto* primitive=source->mMeshes[node->mMeshes[p]];auto weights=sourceSceneWeights(primitive);
                        for(unsigned f=0;f<primitive->mNumFaces;++f,++face)for(unsigned k=0;k<primitive->mFaces[f].mNumIndices;++k) {
                            auto index=primitive->mFaces[f].mIndices[k];auto position=primitive->mVertices[index];
                            aiVector3D expected;
                            if(weights[index].empty())expected=worlds.at(node->mName.C_Str())*position;
                            else for(const auto& influence:weights[index])expected+=(worlds.at(influence.first)*influence.second.first->mOffsetMatrix*position)*ai_real(influence.second.second);
                            size_t corner=mesh->faces.data[face].index_begin+k;
                            require(mesh->skinned_position.exists && corner<mesh->skinned_position.indices.count,"evaluated skin corner missing");
                            auto actual=ufbx_get_vertex_vec3(&mesh->skinned_position,corner);
                            if(mesh->skinned_is_local)actual=ufbx_transform_position(&output->geometry_to_world,actual);
                            actual.x*=unit;actual.y*=unit;actual.z*=unit;
                            double delta=std::max({std::abs(actual.x-expected.x),std::abs(actual.y-expected.y),std::abs(actual.z-expected.z)});
                            require(std::isfinite(delta),"nonfinite evaluated skin");maxSkin=std::max(maxSkin,delta);
                            if(delta>1e-6)std::cout<<"Skin mismatch: clip "<<clip->mName.C_Str()<<", sample "<<sample<<", primitive "<<p<<", face "<<f<<", corner "<<k<<", error "<<delta<<"\n";
                            require(delta<=1e-6,"authored clip skin playback changed");++corners;
                            // Diagnostic only: gcad's inverse-transpose blended basis is
                            // intentionally compared with ufbx's recomputed surface normals.
                            // These are different conventions, not interchangeable oracles.
                            if(primitive->HasNormals() && mesh->skinned_normal.exists) {
                                aiMatrix3x3 linear(worlds.at(node->mName.C_Str()));
                                if(!weights[index].empty()) {
                                    for(unsigned r=0;r<3;++r)for(unsigned c=0;c<3;++c)linear[r][c]=0;
                                    for(const auto& influence:weights[index]) {
                                        aiMatrix3x3 joint(worlds.at(influence.first)*influence.second.first->mOffsetMatrix);
                                        for(unsigned r=0;r<3;++r)for(unsigned c=0;c<3;++c)
                                            linear[r][c]+=joint[r][c]*ai_real(influence.second.second);
                                    }
                                }
                                require(std::isfinite(linear.Determinant()) && std::abs(linear.Determinant())>1e-10,"singular shading diagnostic transform");
                                auto normalMatrix=linear;normalMatrix.Inverse().Transpose();
                                auto expectedNormal=normalMatrix*primitive->mNormals[index];
                                if(primitive->HasTangentsAndBitangents() && mesh->vertex_tangent.exists && mesh->vertex_bitangent.exists) {
                                    maxBasisError=std::max(maxBasisError,verifySceneBasis(linear,primitive->mNormals[index],
                                        primitive->mTangents[index],primitive->mBitangents[index],output,corner,basisControls));
                                    ++basisSamples;
                                }
                                auto readNormal=ufbx_get_vertex_vec3(&mesh->skinned_normal,corner);
                                if(mesh->skinned_is_local) {
                                    auto inverse=ufbx_matrix_invert(&output->geometry_to_world);
                                    readNormal={inverse.m00*readNormal.x+inverse.m10*readNormal.y+inverse.m20*readNormal.z,
                                        inverse.m01*readNormal.x+inverse.m11*readNormal.y+inverse.m21*readNormal.z,
                                        inverse.m02*readNormal.x+inverse.m12*readNormal.y+inverse.m22*readNormal.z};
                                }
                                double lengthExpected=std::sqrt(double(expectedNormal.x)*expectedNormal.x+double(expectedNormal.y)*expectedNormal.y+double(expectedNormal.z)*expectedNormal.z);
                                double lengthRead=std::sqrt(readNormal.x*readNormal.x+readNormal.y*readNormal.y+readNormal.z*readNormal.z);
                                require(std::isfinite(lengthExpected) && lengthExpected>0 && std::isfinite(lengthRead) && lengthRead>0,"invalid shading diagnostic normal");
                                double dot=(expectedNormal.x*readNormal.x+expectedNormal.y*readNormal.y+expectedNormal.z*readNormal.z)/(lengthExpected*lengthRead);
                                double angle=std::acos(std::max(-1.0,std::min(1.0,dot)))*180.0/3.14159265358979323846;
                                ++normalSamples;if(angle>.1)++normalDifferences;maxNormalAngle=std::max(maxNormalAngle,angle);
                                if(linear.Determinant()<0)++reflectedNormals;
                                else {
                                    if(angle>.1)++positiveNormalDifferences;maxPositiveNormalAngle=std::max(maxPositiveNormalAngle,angle);
                                    if(seconds==0) {
                                        ++zeroTimePositiveNormals;if(angle>.1)++zeroTimePositiveDifferences;
                                        maxZeroTimePositiveAngle=std::max(maxZeroTimePositiveAngle,angle);
                                    }
                                }
                            }
                        }
                    }
                }
                for(unsigned i=0;i<node->mNumChildren;++i)skinVisit(node->mChildren[i]);
            };
            if(evaluateSkin)skinVisit(source->mRootNode);
        }
    }
    std::cout<<"Animation audit: "<<source->mNumAnimations<<" clips, "<<comparisons<<" sampled node transforms, maximum component error "<<maximum<<"\n";
    std::cout<<"Authored skin playback: "<<corners<<" sampled corners, maximum component error "<<maxSkin<<(physical?" metres":" source units")<<"\n";
    if(evaluateSkin) {
        std::cout<<"Explicit adapter basis: "<<basisSamples<<" sampled corners, maximum component error "<<maxBasisError<<"; "<<(basisControls?4:0)<<" direction/handedness/singular controls rejected\n";
        std::cout<<"Normal convention diagnostic (not parity): "<<normalSamples<<" corners, "<<normalDifferences<<" above 0.1 degrees, maximum "<<maxNormalAngle<<" degrees; blended inverse-transpose versus recomputed surface; deformed tangents unavailable\n";
        std::cout<<"Normal determinant split: "<<reflectedNormals<<" reflected corners; positive determinant "<<positiveNormalDifferences<<" above 0.1 degrees, maximum "<<maxPositiveNormalAngle<<" degrees\n";
        std::cout<<"Zero-time positive normal diagnostic: "<<zeroTimePositiveNormals<<" corners, "<<zeroTimePositiveDifferences<<" above 0.1 degrees, maximum "<<maxZeroTimePositiveAngle<<" degrees\n";
    }
    return comparisons;
}
